use crate::input::MethodInput;
use crate::p2_flow::normal::{DeclarationType, ResultType};
use crate::p5_types::normal::Kind;
use crate::p6_variables::normal::StorageBody;
use crate::substrate::owned::{copy_units, push};
use crate::{DeclarationCatalog, MethodBudget, RequestError, TypeId};

struct NamedType {
    original: TypeId,
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
        let Kind::Nominal(id) = kind else {
            return Err(RequestError::InternalInvariant(
                "source type lost its identity",
            ));
        };
        for entry in &self.named {
            budget.charge_work(1)?;
            if entry.original == id {
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
        let Kind::Nominal(id) = kind else {
            return Ok(true);
        };
        for entry in &self.named {
            budget.charge_work(1)?;
            if entry.original == id {
                return Ok(true);
            }
        }
        let descriptor = copy_units(input.type_descriptor(id, budget)?, budget)?;
        if !crate::p4_calls::source_type(input, catalog, &descriptor, budget)? {
            return Ok(false);
        }
        let Some(source) = super::calls::type_name(&descriptor, budget)? else {
            return Ok(false);
        };
        push(
            &mut self.named,
            NamedType {
                original: id,
                source,
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
    if let ResultType::Value(result) = storage.typed.ssa.result
        && !names.include(input, catalog, result.kind(), budget)?
    {
        return Ok(None);
    }
    Ok(Some(names))
}
