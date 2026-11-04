use crate::context::Context;
use crate::input::MethodSource;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{MethodId, RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ReceiverKind {
    Super,
    Direct,
    Virtual,
    Interface,
}
pub(crate) struct ReceiverDeclaration {
    target: ReceiverContract,
}

pub(crate) struct ReceiverContract {
    _sealed: (),
    pub reference: MethodId,
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub kind: ReceiverKind,
    pub arity: usize,
}
pub(crate) fn collect_receiver(
    input: &MethodSource<'_, '_>,
    _catalog: &Context,
    invocation: &super::throws::Invocation,
    kind: ReceiverKind,
    budget: &mut RunContext<'_>,
) -> Result<Option<ReceiverDeclaration>, RequestError> {
    let _span = crate::pipeline::phase_span("p4_receivers");
    let reference = invocation.reference;
    let method = input.method_reference(reference, budget)?;
    let formals = crate::p2_flow::call_facts::original_formals(input, reference, budget)?;
    let owner = copy_units(method.owner, budget)?;
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    Ok(Some(ReceiverDeclaration {
        target: ReceiverContract {
            _sealed: (),
            reference,
            owner,
            name: copy_units(method.declaration.name, budget)?,
            prototype: copy_proto(&method.declaration.prototype, budget)?,
            kind,
            arity: formals.len(),
        },
    }))
}

impl ReceiverDeclaration {
    pub(crate) fn bind_source(
        &self,
        input: &MethodSource<'_, '_>,
        catalog: &Context,
        parameter: &[u16],
        budget: &mut RunContext<'_>,
    ) -> Result<Option<ReceiverContract>, RequestError> {
        let target = &self.target;
        let caller = copy_units(input.owner(), budget)?;
        if target.kind == ReceiverKind::Super {
            if !crate::context::same_units(parameter, &caller, budget)? {
                return Ok(None);
            }
        } else if super::reference_types::source_subtype(
            input,
            catalog,
            parameter,
            &target.owner,
            budget,
        )? == Some(false)
        {
            return Ok(None);
        }
        Ok(Some(ReceiverContract {
            _sealed: (),
            reference: target.reference,
            owner: retain(&target.owner, budget)?,
            name: retain(&target.name, budget)?,
            prototype: retain(&target.prototype, budget)?,
            kind: target.kind,
            arity: target.arity,
        }))
    }
}
pub(super) fn retain(units: &[u16], budget: &mut RunContext<'_>) -> Result<Vec<u16>, RequestError> {
    let mut result = Vec::new();
    for &unit in units {
        budget.checkpoint()?;
        crate::substrate::owned::push(&mut result, unit, budget)?;
    }
    Ok(result)
}
