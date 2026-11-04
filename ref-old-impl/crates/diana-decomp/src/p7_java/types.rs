use crate::context::Context;
use crate::input::MethodSource;
use crate::p2_flow::{DeclarationType, ResultType};
use crate::p4_types::Kind;
use crate::p5_variables::StorageBody;
use crate::substrate::owned::{TextBuffer, copy_units, push};
use crate::{RequestError, RunContext};
use std::cell::OnceCell;

struct NestedIndex {
    /// Original descriptor, enclosing descriptor and simple name units.
    entries: Vec<(Vec<u16>, Vec<u16>, Vec<u16>)>,
    issues: Vec<(Vec<u16>, crate::input::InputError)>,
}

impl NestedIndex {
    fn build(
        input: &MethodSource<'_, '_>,
        budget: &mut RunContext<'_>,
    ) -> Result<Option<Self>, RequestError> {
        let ownership = input.program().ownership_cached(budget)?;
        let mut entries = Vec::new();
        for entry in ownership.nested() {
            budget.checkpoint()?;
            let descriptor = copy_units(
                input
                    .program()
                    .class_with(entry.class(), budget)?
                    .descriptor,
                budget,
            )?;
            let outer = copy_units(
                input
                    .program()
                    .class_with(entry.enclosing(), budget)?
                    .descriptor,
                budget,
            )?;
            let mut name = Vec::new();
            for &unit in entry.name() {
                push(&mut name, unit, budget)?;
            }
            push(&mut entries, (descriptor, outer, name), budget)?;
        }
        let mut issues = Vec::new();
        for id in 0..input.program().header.class_defs.count {
            budget.checkpoint()?;
            let class = crate::ClassDefId(id);
            if let Some(error) = ownership.issue(class)? {
                match input.program().class_with(class, budget) {
                    Ok(class) => {
                        let descriptor = copy_units(class.descriptor, budget)?;
                        push(&mut issues, (descriptor, error), budget)?;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            crate::FailureKind::Malformed | crate::FailureKind::Unsupported
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(Some(Self { entries, issues }))
    }
}

fn nested_name(
    input: &MethodSource<'_, '_>,
    descriptor: &[u16],
    cache: &OnceCell<Option<NestedIndex>>,
    budget: &mut RunContext<'_>,
) -> Result<Option<String>, RequestError> {
    if cache.get().is_none() {
        let built = NestedIndex::build(input, budget)?;
        let _ = cache.set(built);
    }
    let index = cache.get().ok_or(RequestError::InternalInvariant(
        "nested index cache was not initialized",
    ))?;
    let Some(index) = index else {
        return Ok(None);
    };
    for (original, error) in &index.issues {
        budget.checkpoint()?;
        if original == descriptor {
            return Err((*error).into());
        }
    }
    let mut components: Vec<&[u16]> = Vec::new();
    let mut current = descriptor;
    loop {
        let mut parent = None;
        for (original, outer, name) in &index.entries {
            budget.checkpoint()?;
            if original == current {
                parent = Some((outer.as_slice(), name.as_slice()));
                break;
            }
        }
        let Some((outer, name)) = parent else { break };
        push(&mut components, name, budget)?;
        current = outer;
    }
    if components.is_empty() {
        return Ok(None);
    }
    let Some(base) = super::calls::type_name(current, budget)? else {
        return Err(RequestError::SourceNameUnproved {
            class: input.class_id(),
        });
    };
    let mut source = TextBuffer::new();
    source.push(&base, budget)?;
    for component in components.iter().rev() {
        let Some(component) = crate::p7_java::calls::identifier_text(component, budget)? else {
            return Err(RequestError::SourceNameUnproved {
                class: input.class_id(),
            });
        };
        source.push(".", budget)?;
        source.push(&component, budget)?;
    }
    Ok(Some(source.finish()))
}

struct NamedType {
    original: Kind,
    source: String,
}
#[derive(Default)]
pub(crate) struct TypeNames {
    named: Vec<NamedType>,
}
#[derive(Clone, Copy)]
pub(crate) struct SourceType(SourceSpelling);
#[derive(Clone, Copy)]
enum SourceSpelling {
    Builtin(&'static str),
    Named(usize),
}
impl TypeNames {
    pub(super) fn source_type(
        &self,
        kind: Kind,
        budget: &mut RunContext<'_>,
    ) -> Result<SourceType, RequestError> {
        if let Some(name) = kind.builtin_spelling() {
            return Ok(SourceType(SourceSpelling::Builtin(name)));
        }
        for (index, name) in self.named.iter().enumerate() {
            budget.checkpoint()?;
            if name.original == kind {
                return Ok(SourceType(SourceSpelling::Named(index)));
            }
        }
        Err(RequestError::InternalInvariant(
            "source declaration lost type spelling",
        ))
    }
    pub(crate) fn source_name(&self, name: SourceType) -> Result<&str, RequestError> {
        match name.0 {
            SourceSpelling::Builtin(name) => Ok(name),
            SourceSpelling::Named(index) => self
                .named
                .get(index)
                .map(|name| name.source.as_str())
                .ok_or(RequestError::InternalInvariant(
                    "source type spelling is foreign",
                )),
        }
    }

    pub(super) fn name<'a>(
        &'a self,
        kind: Kind,
        budget: &mut RunContext<'_>,
    ) -> Result<&'a str, RequestError> {
        if let Some(name) = kind.builtin_spelling() {
            return Ok(name);
        }
        for entry in &self.named {
            budget.checkpoint()?;
            if entry.original == kind {
                return Ok(&entry.source);
            }
        }
        Err(RequestError::InternalInvariant(
            "verified method lost a named source type",
        ))
    }
    pub(super) fn declaration<'a>(
        &'a self,
        declaration: DeclarationType,
        budget: &mut RunContext<'_>,
    ) -> Result<&'a str, RequestError> {
        match declaration {
            DeclarationType::Narrow(kind) => Ok(kind.spelling()),
            _ => self.name(declaration.kind(), budget),
        }
    }
    pub(super) fn result<'a>(
        &'a self,
        result: ResultType,
        budget: &mut RunContext<'_>,
    ) -> Result<&'a str, RequestError> {
        match result {
            ResultType::Void => Ok("void"),
            ResultType::Value(kind) => self.declaration(kind, budget),
        }
    }
    fn include(
        &mut self,
        input: &MethodSource<'_, '_>,
        _catalog: &Context,
        kind: Kind,
        cache: &OnceCell<Option<NestedIndex>>,
        budget: &mut RunContext<'_>,
    ) -> Result<bool, RequestError> {
        if kind.builtin_spelling().is_some() {
            return Ok(true);
        }
        for entry in &self.named {
            budget.checkpoint()?;
            if entry.original == kind {
                return Ok(true);
            }
        }
        let name = match kind {
            Kind::Nominal(id) => input.type_descriptor(id, budget)?,
            Kind::ReferenceArray(key) | Kind::ProjectedNominal(key) => {
                input.projected_name(key, budget)?
            }
            _ => return Err(RequestError::InternalInvariant("named type lost identity")),
        };
        let descriptor = copy_units(name, budget)?;
        // Only nested classes can have a lexical source name; skip the ownership
        // scan for ordinary top-level descriptors.
        let nested = descriptor.contains(&u16::from(b'$'));
        let _nested_span = if nested {
            crate::pipeline::phase_span("p8_nested_name")
        } else {
            None
        };
        let source = if nested {
            nested_name(input, &descriptor, cache, budget)?
        } else {
            None
        };
        if let Some(source) = source {
            push(
                &mut self.named,
                NamedType {
                    original: kind,
                    source,
                },
                budget,
            )?;
            return Ok(true);
        }
        let mut dimensions = 0usize;
        while descriptor.get(dimensions) == Some(&91) {
            budget.checkpoint()?;
            dimensions += 1;
        }
        let base = &descriptor[dimensions..];
        let primitive = match base {
            [90] => Some("boolean"),
            [66] => Some("byte"),
            [83] => Some("short"),
            [67] => Some("char"),
            [73] => Some("int"),
            [70] => Some("float"),
            [74] => Some("long"),
            [68] => Some("double"),
            [86] => Some("void"),
            _ => None,
        };
        let mut source = TextBuffer::new();
        if let Some(base) = primitive {
            // Class literals name scalar types without introducing scalar reference values.
            source.push(base, budget)?;
        } else {
            let builtin = if base == crate::p4_types::BuiltinReference::Object.descriptor() {
                Some(crate::p4_types::BuiltinReference::Object)
            } else if base == crate::p4_types::BuiltinReference::String.descriptor() {
                Some(crate::p4_types::BuiltinReference::String)
            } else {
                None
            };
            if let Some(builtin) = builtin {
                source.push(builtin.spelling(), budget)?;
            } else {
                let Some(base) = super::calls::type_name(base, budget)? else {
                    return Ok(false);
                };
                source.push(&base, budget)?;
            }
        }
        for _ in 0..dimensions {
            source.push("[]", budget)?;
        }
        push(
            &mut self.named,
            NamedType {
                original: kind,
                source: source.finish(),
            },
            budget,
        )?;
        Ok(true)
    }
}
pub(crate) fn resolve(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    storage: &StorageBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Option<TypeNames>, RequestError> {
    let mut names = TypeNames::default();
    let ownership_cache: OnceCell<Option<NestedIndex>> = OnceCell::new();
    for local in &storage.locals {
        budget.checkpoint()?;
        if !names.include(input, catalog, local.kind, &ownership_cache, budget)? {
            return Ok(None);
        }
    }
    for op in &storage.typed.ssa.operations {
        budget.checkpoint()?;
        if let crate::p2_flow::Op::Class(_, reference) = op.op
            && !names.include(
                input,
                catalog,
                Kind::Nominal(reference),
                &ownership_cache,
                budget,
            )?
        {
            return Ok(None);
        }
        if let crate::p2_flow::Op::InstanceOf(_, _, _, kind) = op.op
            && !names.include(input, catalog, kind.kind(), &ownership_cache, budget)?
        {
            return Ok(None);
        }
    }
    if let ResultType::Value(result) = storage.typed.ssa.result
        && !names.include(input, catalog, result.kind(), &ownership_cache, budget)?
    {
        return Ok(None);
    }
    Ok(Some(names))
}
