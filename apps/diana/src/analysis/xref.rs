use diana_decomp::input::{MethodBody, ProgramIndex, ReferenceFacts};
use diana_decomp::raw::LayoutKind;
use diana_decomp::{ClassDefId, MethodId};

use super::apk::DexRoot;
use super::dex::{self, ImageIndex};
use super::images::LogicalDex;
use super::query::{ImageInfo, QueryError};
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
    pub logical: LogicalDex,
    pub class_def: ClassDefId,
    /// Original owner descriptor for the canonical method locator.
    pub class_descriptor: Vec<u16>,
    pub method: MethodId,
    pub method_name: Vec<u16>,
    pub method_prototype: Vec<u16>,
    /// One for the queried target, two for a caller of a caller.
    pub depth: u32,
    pub code_unit: u32,
    pub use_: &'static str,
    pub matched: Matched,
}

#[derive(Clone, Debug)]
pub struct XrefPlan {
    /// Every target the scan looks for. One target keeps the single query; a
    /// transitive level asks for all callers of the previous level at once.
    pub targets: Vec<Target>,
    /// Exact class descriptor restricting the referencing definition.
    pub class: Option<Vec<u16>>,
    /// Which transitive level this scan answers.
    pub depth: u32,
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
        sink: &mut impl FnMut(XrefRow) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        super::runner::run(
            self.roots.len(),
            self.plan.jobs,
            |ordinal, emit| {
                let root = &self.roots[ordinal];

                dex::visit_logical(root.bytes(), |logical, index| {
                    scan_logical(&index, ImageIndex(ordinal), logical, &self.plan, emit)
                })
            },
            sink,
        )
    }
}

fn scan_logical(
    index: &ProgramIndex<'_>,
    image: ImageIndex,
    logical: LogicalDex,
    plan: &XrefPlan,
    emit: &mut dyn FnMut(XrefRow) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    let failure: std::cell::RefCell<Option<QueryError>> = std::cell::RefCell::new(None);
    let result = {
        let mut forward = |row: XrefRow| -> Result<(), diana_decomp::input::InputError> {
            if let Err(error) = emit(row) {
                *failure.borrow_mut() = Some(error);
                return Err(diana_decomp::input::InputError::InternalInvariant(
                    "xref sink stopped",
                ));
            }
            Ok(())
        };
        index.visit_classes(|class_def, class| {
            let class = class?;
            if let Some(target) = plan.class.as_deref()
                && !class.descriptor.matches(target)?
            {
                return Ok(());
            }
            let class_descriptor = class.descriptor.units()?;
            class.visit_methods(|method| {
                let declaration = method.declaration?;
                let name = declaration.name.units()?;
                let mut prototype = Vec::new();
                prototype.push(u16::from(b'('));
                declaration.prototype.visit_parameters(|parameter| {
                    prototype.extend(parameter.units()?);
                    Ok(())
                })?;
                prototype.push(u16::from(b')'));
                prototype.extend(declaration.prototype.return_type.units()?);
                let view = match method.body {
                    Ok(MethodBody::Code(view)) => view,
                    Ok(MethodBody::DeclarationOnly) => return Ok(()),
                    Err(error) => {
                        *failure.borrow_mut() = Some(error.into());
                        return Err(diana_decomp::input::InputError::InternalInvariant(
                            "xref method reader stopped",
                        ));
                    }
                };
                let mut sites = Vec::new();
                let collected = {
                    view.visit_layouts(|code_unit, layout| {
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
                    return Err(diana_decomp::input::InputError::InternalInvariant(
                        "xref producer stopped",
                    ));
                }
                for (code_unit, mnemonic, reference) in sites {
                    let facts = index.reference_facts(reference)?;
                    let found = match matched(facts, &plan.targets) {
                        Ok(found) => found,
                        Err(error) => {
                            *failure.borrow_mut() = Some(error);
                            return Err(diana_decomp::input::InputError::InternalInvariant(
                                "xref producer stopped",
                            ));
                        }
                    };
                    let Some(matched) = found else { continue };
                    let row = match (copy(&name), copy(&prototype)) {
                        (Ok(method_name), Ok(method_prototype)) => XrefRow {
                            image,
                            logical,
                            class_def,
                            class_descriptor: class_descriptor.clone(),
                            method: method.method_idx,
                            method_name,
                            method_prototype,
                            depth: plan.depth,
                            code_unit: code_unit.0,
                            use_: mnemonic,
                            matched,
                        },
                        (Err(error), _) | (_, Err(error)) => {
                            *failure.borrow_mut() = Some(error);
                            return Err(diana_decomp::input::InputError::InternalInvariant(
                                "xref producer stopped",
                            ));
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

fn copy(units: &[u16]) -> Result<Vec<u16>, QueryError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(units.len())
        .map_err(|_| QueryError::WorkerLost)?;

    owned.extend_from_slice(units);
    Ok(owned)
}

fn matched(
    facts: ReferenceFacts<'_, '_>,
    targets: &[Target],
) -> Result<Option<Matched>, QueryError> {
    // Only a method query carries more than one target; the string, type and
    // field kinds always carry exactly one, so the loop runs once.
    for target in targets {
        let found = match (&facts, target) {
            (ReferenceFacts::String(name), Target::String(units)) => {
                name.matches(units)?.then(|| Matched::String(units.clone()))
            }
            (ReferenceFacts::Type(name), Target::Type(units)) => {
                name.matches(units)?.then(|| Matched::Type(units.clone()))
            }
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
                if !owner.matches(owner_units)?
                    || !name.matches(name_units)?
                    || !prototype.matches(prototype_units)?
                {
                    continue;
                }
                Some(Matched::Method {
                    owner: owner.units()?,
                    name: name.units()?,
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
                if !owner.matches(owner_units)?
                    || !name.matches(name_units)?
                    || !field_type.matches(field_units)?
                {
                    continue;
                }
                Some(Matched::Field {
                    owner: owner.units()?,
                    name: name.units()?,
                    field_type: field_type.units()?,
                })
            }
            _ => None,
        };
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

/// Convenience for a `type` target expressed as a descriptor or dotted name.
pub fn type_target(text: &str) -> Target {
    Target::Type(super::dex::class_descriptor(text))
}
