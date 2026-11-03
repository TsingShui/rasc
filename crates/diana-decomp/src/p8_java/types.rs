use crate::input::MethodInput;
use crate::p0_knowledge::DeclarationCatalog;
use crate::p2_flow::{DeclarationType, ResultType};
use crate::p5_types::Kind;
use crate::p6_variables::StorageBody;
use crate::substrate::owned::{TextBuffer, copy_units, push};
use crate::{MethodBudget, RequestError};
use std::cell::OnceCell;

struct NestedIndex {
    /// descriptor -> (enclosing descriptor, simple name units)
    entries: std::collections::HashMap<Vec<u16>, (Vec<u16>, Vec<u16>)>,
}

impl NestedIndex {
    fn build(
        input: &MethodInput<'_, '_>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<Option<Self>, RequestError> {
        let ownership = input.program().ownership_cached(budget)?;
        let mut entries = std::collections::HashMap::new();
        let Some(ownership) = ownership else {
            return Ok(None);
        };
        for entry in ownership.nested() {
            budget.charge_work(1)?;
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
            entries.insert(descriptor, (outer, name));
        }
        Ok(Some(Self { entries }))
    }
}

fn nested_name(
    input: &MethodInput<'_, '_>,
    descriptor: &[u16],
    cache: &OnceCell<Option<NestedIndex>>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<String>, RequestError> {
    let index = cache.get_or_init(|| NestedIndex::build(input, budget).ok().flatten());
    let Some(index) = index else {
        return Ok(None);
    };
    let mut components: Vec<Vec<u16>> = Vec::new();
    let mut current: Vec<u16> = descriptor.to_vec();
    while let Some((outer, name)) = index.entries.get(&current) {
        budget.charge_work(1)?;
        components.push(name.clone());
        current = outer.clone();
    }
    if components.is_empty() {
        return Ok(None);
    }
    let Some(base) = super::calls::type_name(&current, budget)? else {
        return Err(RequestError::SourceNameUnproved {
            class: input.class_id(),
        });
    };
    let mut source = TextBuffer::new();
    source.push(&base, budget)?;
    for component in components.iter().rev() {
        let Some(component) = crate::p8_java::calls::identifier_text(component, budget)? else {
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
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<SourceType, RequestError> {
        if let Some(name) = kind.builtin_spelling() {
            return Ok(SourceType(SourceSpelling::Builtin(name)));
        }
        for (index, name) in self.named.iter().enumerate() {
            budget.charge_work(1)?;
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
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<&'a str, RequestError> {
        if let Some(name) = kind.builtin_spelling() {
            return Ok(name);
        }
        for entry in &self.named {
            budget.charge_work(1)?;
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
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<&'a str, RequestError> {
        match declaration {
            DeclarationType::Narrow(kind) => Ok(kind.spelling()),
            _ => self.name(declaration.kind(), budget),
        }
    }
    pub(super) fn result<'a>(
        &'a self,
        result: ResultType,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<&'a str, RequestError> {
        match result {
            ResultType::Void => Ok("void"),
            ResultType::Value(kind) => self.declaration(kind, budget),
        }
    }
    fn include(
        &mut self,
        input: &MethodInput<'_, '_>,
        catalog: &DeclarationCatalog,
        kind: Kind,
        cache: &OnceCell<Option<NestedIndex>>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, RequestError> {
        if kind.builtin_spelling().is_some() {
            return Ok(true);
        }
        for entry in &self.named {
            budget.charge_work(1)?;
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
            budget.charge_work(1)?;
            dimensions += 1;
        }
        // The JVM array-type format admits at most 255 dimensions.
        if dimensions > 255 {
            return Ok(false);
        }
        let base = &descriptor[dimensions..];
        let constant_name = if dimensions == 0 && descriptor.first() == Some(&76) {
            &descriptor[1..descriptor.len() - 1]
        } else {
            &descriptor
        };
        super::declarations::check_constant_utf8(constant_name, budget)?;
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
            let builtin = if base == crate::p5_types::BuiltinReference::Object.descriptor() {
                Some(crate::p5_types::BuiltinReference::Object)
            } else if base == crate::p5_types::BuiltinReference::String.descriptor() {
                Some(crate::p5_types::BuiltinReference::String)
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
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    storage: &StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<TypeNames>, RequestError> {
    let mut names = TypeNames::default();
    let ownership_cache: OnceCell<Option<NestedIndex>> = OnceCell::new();
    for local in &storage.locals {
        budget.charge_work(1)?;
        if !names.include(input, catalog, local.kind, &ownership_cache, budget)? {
            return Ok(None);
        }
    }
    for op in &storage.typed.ssa.operations {
        budget.charge_work(1)?;
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
