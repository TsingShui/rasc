use crate::input::MethodInput;
use crate::p2_flow::{DeclarationType, ResultType};
use crate::p5_types::Kind;
use crate::p6_variables::StorageBody;
use crate::substrate::owned::{TextBuffer, copy_units, push};
use crate::{DeclarationCatalog, MethodBudget, RequestError};

fn nested_name(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    descriptor: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<String>, RequestError> {
    let ownership = crate::input::ownership::collect(input.program(), budget)?;
    let mut components: Vec<String> = Vec::new();
    let mut current = Vec::new();
    for &unit in descriptor {
        push(&mut current, unit, budget)?;
    }
    for _ in 0..=ownership.nested().len() {
        budget.charge_work(1)?;
        let mut found = None;
        for entry in ownership.nested() {
            budget.charge_work(1)?;
            if input
                .program()
                .class_with(entry.class(), budget)?
                .descriptor
                .matches_with(&current, budget)?
            {
                found = Some(entry);
                break;
            }
        }
        let Some(entry) = found else {
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
                source.push(".", budget)?;
                source.push(component, budget)?;
            }
            return Ok(Some(source.finish()));
        };
        let Some(scope) = crate::p4_calls::nested_scope::collect(
            input.program(),
            entry.class(),
            catalog,
            budget,
        )?
        else {
            return Err(RequestError::SourceNameUnproved {
                class: input.class_id(),
            });
        };
        let Some(component) = super::calls::identifier_text(scope.name(), budget)? else {
            return Err(RequestError::SourceNameUnproved {
                class: input.class_id(),
            });
        };
        push(&mut components, component, budget)?;
        current.clear();
        for &unit in scope.outer() {
            push(&mut current, unit, budget)?;
        }
    }
    Err(RequestError::InternalInvariant(
        "checked ownership name chain did not terminate",
    ))
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
        if let Some(source) = nested_name(input, catalog, &descriptor, budget)? {
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
                if !crate::p4_calls::qualifier_available(
                    input,
                    catalog,
                    &[106, 97, 118, 97],
                    budget,
                )? {
                    return Ok(false);
                }
                source.push(builtin.spelling(), budget)?;
            } else {
                if !crate::p4_calls::source_reference_type(input, catalog, base, budget)? {
                    return Ok(false);
                }
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
    for local in &storage.locals {
        budget.charge_work(1)?;
        if !names.include(input, catalog, local.kind, budget)? {
            return Ok(None);
        }
    }
    for op in &storage.typed.ssa.operations {
        budget.charge_work(1)?;
        if let crate::p2_flow::Op::Class(_, reference) = op.op
            && !names.include(input, catalog, Kind::Nominal(reference), budget)?
        {
            return Ok(None);
        }
        if let crate::p2_flow::Op::InstanceOf(_, _, _, kind) = op.op
            && !names.include(input, catalog, kind.kind(), budget)?
        {
            return Ok(None);
        }
    }
    if let ResultType::Value(result) = storage.typed.ssa.result
        && !names.include(input, catalog, result.kind(), budget)?
    {
        return Ok(None);
    }
    Ok(Some(names))
}
