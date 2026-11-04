use crate::input::MethodSource;
use crate::p2_flow::entry::ParameterFamily;
use crate::substrate::owned::push;
use crate::{MethodId, RequestError, RunContext};
pub(crate) struct EntryDeclarations {
    original: super::constructor_evidence::ConstructorDeclarations,
    owner: Vec<u16>,
    formals: Vec<ParameterFamily>,
    scope: Option<super::allocation::ConstructorUse>,
}
impl EntryDeclarations {
    pub(crate) fn scope(&self) -> Option<super::allocation::ConstructorUse> {
        self.scope
    }
    pub(crate) fn formals(&self) -> &[ParameterFamily] {
        &self.formals
    }

    pub(crate) fn bind(
        &self,
        reference: MethodId,
        pc: crate::CodeUnitOffset,
        kind: super::allocation::ConstructorUse,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        self.original
            .bind(reference, pc, &self.owner, kind, budget)?;
        Ok(())
    }
}
pub(crate) fn collect(
    _schedule: &crate::engine::SchedulePermit,
    input: &MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    reference: MethodId,
    pc: crate::CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<EntryDeclarations, RequestError> {
    collect_impl(input, catalog, reference, pc, budget)
}

fn collect_impl(
    input: &MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    reference: MethodId,
    pc: crate::CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<EntryDeclarations, RequestError> {
    let declaration = input.method_reference(reference, budget)?;
    let owner = crate::substrate::owned::copy_units(declaration.owner, budget)?;
    let scope = if input.owner().matches_with(&owner, budget)? {
        Some(super::allocation::ConstructorUse::This)
    } else {
        let context = input.program().class_context(input.class_id(), budget)?;
        match context.superclass? {
            Some(parent) if parent.matches_with(&owner, budget)? => {
                Some(super::allocation::ConstructorUse::Super)
            }
            _ => None,
        }
    };
    let original = super::constructor_evidence::collect(
        input,
        catalog,
        &super::throws::Invocation { reference, pc },
        budget,
    )?;
    Ok(EntryDeclarations {
        original,
        owner,
        scope,
        formals: formals(input, reference, budget)?,
    })
}
fn formals(
    input: &MethodSource<'_, '_>,
    reference: MethodId,
    budget: &mut RunContext<'_>,
) -> Result<Vec<ParameterFamily>, RequestError> {
    let original = input.method_reference(reference, budget)?;
    let mut result = Vec::new();
    original
        .declaration
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            let family = match name.reader()?.next(budget)? {
                Some(74 | 68) => ParameterFamily::Wide,
                Some(76 | 91) => ParameterFamily::Reference,
                _ => ParameterFamily::Word,
            };
            push(&mut result, family, budget)
        })?;
    Ok(result)
}
