use crate::input::MethodInput;
use crate::p2_flow::normal::{DeclarationType, ResultType};
use crate::p5_types::normal::Kind;
use crate::p6_variables::normal::StorageBody;
use crate::substrate::owned::{TextBuffer, copy_units, push};
use crate::{DeclarationCatalog, MethodBudget, RequestError};

struct NamedType {
    original: Kind,
    source: String,
}
#[derive(Default)]
pub(crate) struct TypeNames {
    named: Vec<NamedType>,
}
impl TypeNames {
    pub(crate) fn name<'a>(
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
    pub(crate) fn declaration<'a>(
        &'a self,
        declaration: DeclarationType,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<&'a str, RequestError> {
        match declaration {
            DeclarationType::Narrow(kind) => Ok(kind.spelling()),
            _ => self.name(declaration.kind(), budget),
        }
    }
    pub(crate) fn result<'a>(
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
                if !crate::p4_calls::normal::source_reference_type(input, catalog, base, budget)? {
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
        if let crate::p2_flow::normal::Op::Class(_, reference) = op.op
            && !names.include(input, catalog, Kind::Nominal(reference), budget)?
        {
            return Ok(None);
        }
        if let crate::p2_flow::normal::Op::InstanceOf(_, _, _, kind) = op.op
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
