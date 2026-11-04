use crate::context::Context;
use crate::input::MethodSource;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{CodeUnitOffset, MethodId, RequestError, RunContext};

pub(crate) struct ConstructorContract {
    _sealed: (),
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub prototype: Vec<u16>,
    pub arity: usize,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ConstructorUse {
    Allocation,
    Super,
    This,
}
pub(crate) fn resolve(
    input: &MethodSource<'_, '_>,
    _catalog: &Context,
    invocation: &super::throws::Invocation,
    expected_owner: &[u16],
    _use_kind: ConstructorUse,
    budget: &mut RunContext<'_>,
) -> Result<ConstructorContract, RequestError> {
    // The original reference fixes the constructor; hierarchy, access and
    // throws facts are not required to bind it.
    reference_only(input, invocation, expected_owner, budget)
}

impl ConstructorContract {
    pub(super) fn retain(&self, budget: &mut RunContext<'_>) -> Result<Self, RequestError> {
        Ok(Self {
            _sealed: (),
            reference: self.reference,
            owner: super::receivers::retain(&self.owner, budget)?,
            prototype: super::receivers::retain(&self.prototype, budget)?,
            arity: self.arity,
        })
    }
}

/// A constructor call whose owner has no declaration entry. The original
/// reference still fixes the owner, name and prototype, so the call binds; no
/// access, hierarchy or throws fact is asserted.
fn reference_only(
    input: &MethodSource<'_, '_>,
    invocation: &super::throws::Invocation,
    expected_owner: &[u16],
    budget: &mut RunContext<'_>,
) -> Result<ConstructorContract, RequestError> {
    let reference = invocation.reference;
    let pc = invocation.pc;
    let method = input.method_reference(reference, budget)?;
    if !method.owner.matches_with(expected_owner, budget)?
        || !method.declaration.name.matches_with(
            &const { crate::substrate::owned::ascii_units(*b"<init>") },
            budget,
        )?
        || !method
            .declaration
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return unavailable(pc);
    }
    let formals = crate::p2_flow::call_facts::original_formals(input, reference, budget)?;
    Ok(ConstructorContract {
        _sealed: (),
        reference,
        owner: copy_units(method.owner, budget)?,
        prototype: copy_proto(&method.declaration.prototype, budget)?,
        arity: formals.len(),
    })
}

fn unavailable<T>(pc: CodeUnitOffset) -> Result<T, RequestError> {
    Err(RequestError::InitializationUnavailable { pc })
}
