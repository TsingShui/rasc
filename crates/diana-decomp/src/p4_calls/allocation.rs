use super::contracts::plain;
use crate::input::MethodInput;
use crate::p0_knowledge::DeclarationCatalog;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{CodeUnitOffset, MethodBudget, MethodId, RequestError};

pub(crate) struct ConstructorContract {
    _sealed: (),
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub prototype: Vec<u16>,
    pub arity: usize,
    pub declared_throws: Vec<Vec<u16>>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ConstructorUse {
    Allocation,
    Super,
    This,
}
pub(crate) fn resolve(
    input: &MethodInput<'_, '_>,
    _catalog: &DeclarationCatalog,
    invocation: &super::throws::Invocation<'_>,
    expected_owner: &[u16],
    _use_kind: ConstructorUse,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ConstructorContract, RequestError> {
    // The original reference fixes the constructor; hierarchy, access and
    // throws facts are not required to bind it.
    reference_only(input, invocation, expected_owner, budget)
}

impl ConstructorContract {
    pub(super) fn retain(&self, budget: &mut MethodBudget<'_, '_>) -> Result<Self, RequestError> {
        Ok(Self {
            _sealed: (),
            reference: self.reference,
            owner: super::receivers::retain(&self.owner, budget)?,
            prototype: super::receivers::retain(&self.prototype, budget)?,
            arity: self.arity,
            declared_throws: super::throws::retain(Some(&self.declared_throws), budget)?,
        })
    }
}

/// A constructor call whose owner has no declaration entry. The original
/// reference still fixes the owner, name and prototype, so the call binds; no
/// access, hierarchy or throws fact is asserted.
fn reference_only(
    input: &MethodInput<'_, '_>,
    invocation: &super::throws::Invocation<'_>,
    expected_owner: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ConstructorContract, RequestError> {
    let reference = invocation.reference;
    let pc = invocation.pc;
    let method = input.method_reference(reference, budget)?;
    if !method.owner.matches_with(expected_owner, budget)?
        || !method
            .declaration
            .name
            .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
        || !method
            .declaration
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return unavailable(pc);
    }
    let Some(formals) = super::contracts::original_formals(input, reference, budget)? else {
        return unavailable(pc);
    };
    Ok(ConstructorContract {
        _sealed: (),
        reference,
        owner: copy_units(method.owner, budget)?,
        prototype: copy_proto(&method.declaration.prototype, budget)?,
        arity: formals.len(),
        declared_throws: Vec::new(),
    })
}

fn unavailable<T>(pc: CodeUnitOffset) -> Result<T, RequestError> {
    Err(RequestError::InitializationUnavailable { pc })
}
