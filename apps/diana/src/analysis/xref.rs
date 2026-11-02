use std::sync::atomic::{AtomicBool, Ordering};

use diana_decomp::input::{MethodBody, ProgramIndex, ReferenceFacts};
use diana_decomp::raw::LayoutKind;
use diana_decomp::{ClassDefId, Limits, MethodId, RunContext};

use super::apk::DexRoot;
use super::dex::ImageIndex;
use super::query::{ImageInfo, QueryError, QueryLimits};
use super::request::RequestScope;

/// The exact original object a caller searches for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    /// Complete UTF-16 string equality, not a substring or a pattern.
    String(Vec<u16>),
    /// An original type descriptor.
    Type(Vec<u16>),
    /// A complete method reference including its prototype.
    Method {
        owner: Vec<u16>,
        name: Vec<u16>,
        prototype: Vec<u16>,
    },
    /// A complete field reference including its field type.
    Field {
        owner: Vec<u16>,
        name: Vec<u16>,
        field_type: Vec<u16>,
    },
}

/// The original referenced value actually found at one site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Matched {
    String(Vec<u16>),
    Type(Vec<u16>),
    Method {
        owner: Vec<u16>,
        name: Vec<u16>,
        prototype: Vec<u16>,
    },
    Field {
        owner: Vec<u16>,
        name: Vec<u16>,
        field_type: Vec<u16>,
    },
}

/// One instruction-level reference site. The context is the original method
/// identity plus its original code-unit offset; `use_` is the original DEX
/// mnemonic of the referencing instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XrefRow {
    pub image: ImageIndex,
    pub class_def: ClassDefId,
    /// Original owner descriptor for the canonical method locator.
    pub class_descriptor: Vec<u16>,
    pub method: MethodId,
    pub method_name: Vec<u16>,
    pub method_prototype: Vec<u16>,
    pub code_unit: u32,
    pub use_: &'static str,
    pub matched: Matched,
}

#[derive(Clone, Debug)]
pub struct XrefPlan {
    pub target: Target,
    /// Exact class descriptor restricting the referencing definition.
    pub class: Option<Vec<u16>>,
    pub limits: QueryLimits,
    pub jobs: usize,
}

/// Prepared reference scan over the root images of one leased input.
pub struct Xref<'a> {
    roots: Vec<DexRoot<'a>>,
    images: Vec<ImageInfo>,
    plan: XrefPlan,
}

impl<'a> Xref<'a> {
    pub fn new(scope: &'a RequestScope, plan: XrefPlan) -> Result<Self, QueryError> {
        if plan.jobs == 0 {
            return Err(QueryError::InvalidJobs);
        }
        let roots = scope.roots()?;
        let mut images = Vec::new();
        for (ordinal, root) in roots.iter().enumerate() {
            images.push(ImageInfo {
                index: ImageIndex(ordinal),
                label: root.label().to_owned(),
                entry: root.entry().map(|entry| entry.index()),
            });
        }
        Ok(Self {
            roots,
            images,
            plan,
        })
    }

    pub fn images(&self) -> &[ImageInfo] {
        &self.images
    }

    /// Streams every matching site in completion order. A site is never merged
    /// with another site of the same method, and no site is invented for a
    /// declaration or a hierarchy edge.
    pub fn run(
        &self,
        cancel: &AtomicBool,
        sink: &mut impl FnMut(XrefRow) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        super::runner::run(
            self.roots.len(),
            self.plan.jobs,
            cancel,
            self.plan.limits,
            |ordinal, cancel, emit| {
                let root = &self.roots[ordinal];
                let cancelled = || cancel.load(Ordering::Relaxed);
                let mut control = RunContext::new(Limits::default(), &cancelled);
                scan_root(root, ImageIndex(ordinal), &self.plan, &mut control, emit)
            },
            sink,
        )
    }
}

fn scan_root(
    root: &DexRoot<'_>,
    image: ImageIndex,
    plan: &XrefPlan,
    control: &mut RunContext<'_>,
    emit: &mut dyn FnMut(XrefRow) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    let index = ProgramIndex::open(root.bytes(), control)?;
    let failure: std::cell::RefCell<Option<QueryError>> = std::cell::RefCell::new(None);
    let result = {
        let mut forward = |row: XrefRow| -> Result<(), diana_decomp::input::InputError> {
            if failure.borrow().is_none()
                && let Err(error) = emit(row)
            {
                *failure.borrow_mut() = Some(error);
            }
            Ok(())
        };
        index.visit_classes(control, |class_def, class, control| {
            let class = class?;
            if let Some(target) = plan.class.as_deref()
                && !class.descriptor.matches(target, control)?
            {
                return Ok(());
            }
            let class_descriptor = class.descriptor.units(control)?;
            class.visit_methods(control, |method, control| {
                let declaration = method.declaration?;
                let name = declaration.name.units(control)?;
                let mut prototype = Vec::new();
                prototype.push(u16::from(b'('));
                declaration
                    .prototype
                    .visit_parameters(control, |parameter, control| {
                        prototype.extend(parameter.units(control)?);
                        Ok(())
                    })?;
                prototype.push(u16::from(b')'));
                prototype.extend(declaration.prototype.return_type.units(control)?);
                let Ok(MethodBody::Code(view)) = &method.body else {
                    return Ok(());
                };
                let mut sites = Vec::new();
                let collected = {
                    let budget = control.method_budget()?;
                    let mut fallback = budget.into_fallback()?;
                    view.visit_layouts(&mut fallback, |code_unit, layout, _budget| {
                        let LayoutKind::Instruction(spec) = layout.kind() else {
                            return Ok(());
                        };
                        for reference in layout.references().into_iter().flatten() {
                            sites
                                .try_reserve(1)
                                .map_err(diana_decomp::input::MethodError::Allocation)?;
                            sites.push((code_unit, spec.mnemonic, reference.target));
                        }
                        Ok(())
                    })
                };
                if let Err(error) = collected {
                    *failure.borrow_mut() = Some(error.into());
                    return Ok(());
                }
                for (code_unit, mnemonic, reference) in sites {
                    control.checkpoint()?;
                    let facts = index.reference_facts(reference, control)?;
                    let found = match matched(facts, &plan.target, control) {
                        Ok(found) => found,
                        Err(error) => {
                            *failure.borrow_mut() = Some(error);
                            return Ok(());
                        }
                    };
                    let Some(matched) = found else { continue };
                    let row = match (copy(&name, control), copy(&prototype, control)) {
                        (Ok(method_name), Ok(method_prototype)) => XrefRow {
                            image,
                            class_def,
                            class_descriptor: class_descriptor.clone(),
                            method: method.method_idx,
                            method_name,
                            method_prototype,
                            code_unit: code_unit.0,
                            use_: mnemonic,
                            matched,
                        },
                        (Err(error), _) | (_, Err(error)) => {
                            *failure.borrow_mut() = Some(error);
                            return Ok(());
                        }
                    };
                    forward(row)?;
                }
                Ok(())
            })
        })
    };
    match failure.into_inner() {
        Some(error) => Err(error),
        None => result.map_err(QueryError::from),
    }
}

fn copy(units: &[u16], control: &mut RunContext<'_>) -> Result<Vec<u16>, QueryError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(units.len())
        .map_err(|_| QueryError::WorkerLost)?;
    control.charge_allocation(u64::try_from(units.len()).map_err(|_| QueryError::WorkerLost)?)?;
    owned.extend_from_slice(units);
    Ok(owned)
}

fn matched(
    facts: ReferenceFacts<'_, '_>,
    target: &Target,
    control: &mut RunContext<'_>,
) -> Result<Option<Matched>, QueryError> {
    Ok(match (facts, target) {
        (ReferenceFacts::String(name), Target::String(units)) => name
            .matches(units, control)?
            .then(|| Matched::String(units.clone())),
        (ReferenceFacts::Type(name), Target::Type(units)) => name
            .matches(units, control)?
            .then(|| Matched::Type(units.clone())),
        (
            ReferenceFacts::Method {
                owner,
                name,
                prototype,
            },
            Target::Method {
                owner: owner_units,
                name: name_units,
                prototype: prototype_units,
            },
        ) => {
            if !owner.matches(owner_units, control)?
                || !name.matches(name_units, control)?
                || !prototype.matches(prototype_units, control)?
            {
                return Ok(None);
            }
            Some(Matched::Method {
                owner: owner.units(control)?,
                name: name.units(control)?,
                prototype: prototype_units.clone(),
            })
        }
        (
            ReferenceFacts::Field {
                owner,
                name,
                field_type,
            },
            Target::Field {
                owner: owner_units,
                name: name_units,
                field_type: field_units,
            },
        ) => {
            if !owner.matches(owner_units, control)?
                || !name.matches(name_units, control)?
                || !field_type.matches(field_units, control)?
            {
                return Ok(None);
            }
            Some(Matched::Field {
                owner: owner.units(control)?,
                name: name.units(control)?,
                field_type: field_type.units(control)?,
            })
        }
        _ => None,
    })
}

/// Convenience for a `type` target expressed as a descriptor or dotted name.
pub fn type_target(text: &str) -> Target {
    Target::Type(super::dex::class_descriptor(text))
}
