use crate::p2_flow::normal::{CallId, InvokeKind};
use crate::p4_calls::normal::CallEvidence;
use crate::p5_types::normal::{Kind, TypedBody};
use crate::{MethodBudget, RequestError};

pub(super) fn verify(
    typed: &TypedBody<'_>,
    id: CallId,
    actual: &super::calls::ReceiverCall,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
    let site = typed
        .ssa
        .calls
        .get(index)
        .ok_or(RequestError::InternalInvariant(
            "receiver has foreign call identity",
        ))?;
    let bound = typed.ssa.call(id)?;
    let Some(CallEvidence::Receiver { facts, .. }) = typed.bindings.get(index) else {
        return Ok(false);
    };
    let expected = match site.kind {
        InvokeKind::Direct => crate::p4_calls::ReceiverKind::Direct,
        InvokeKind::Super => crate::p4_calls::ReceiverKind::Super,
        InvokeKind::Virtual => crate::p4_calls::ReceiverKind::Virtual,
        InvokeKind::Interface => crate::p4_calls::ReceiverKind::Interface,
        InvokeKind::Static => return Ok(false),
    };
    let Some(receiver) = typed
        .call_uses
        .get(index)
        .and_then(|uses| uses.receiver.as_ref())
    else {
        return Ok(false);
    };
    if site.kind == InvokeKind::Super
        && receiver.identity != crate::p5_types::calls::ReceiverIdentity::ImplicitThis
    {
        return Ok(false);
    }
    if actual.contract.kind != expected
        || actual.contract.reference != site.reference
        || actual.contract.owner != facts.owner
        || actual.contract.name != facts.name
        || actual.contract.prototype != facts.prototype
        || actual.invoke != site.pc
        || actual.result_site != site.result_pc
        || actual.receiver.value().0 != receiver.value.0
        || matches!(actual.receiver, super::calls::CallReceiver::Super(_))
            != (expected == crate::p4_calls::ReceiverKind::Super)
        || bound.inputs.first() != Some(&receiver.value)
        || site.registers.first() != Some(&actual.original_receiver)
        || typed.kinds
            [usize::try_from(receiver.value.0).map_err(|_| RequestError::PlatformCapacity)?]
            != Some(Kind::Nominal(receiver.original_type))
    {
        return Ok(false);
    }
    let Some(formals) = facts.formals.as_deref() else {
        return Ok(false);
    };
    let Some((_, values)) = bound.inputs.split_first() else {
        return Ok(false);
    };
    if actual.arguments.len() != values.len()
        || values.len() != formals.len()
        || values.len() != actual.contract.arity
    {
        return Ok(false);
    }
    let mut name = actual.method.encode_utf16();
    for &unit in &facts.name {
        budget.charge_work(1)?;
        if name.next() != Some(unit) {
            return Ok(false);
        }
    }
    if name.next().is_some() {
        return Ok(false);
    }
    for ((&view, value), formal) in actual.arguments.iter().zip(values).zip(formals) {
        budget.charge_work(1)?;
        if !super::dispatcher_verify::call_argument(typed, view, *value, *formal)? {
            return Ok(false);
        }
    }
    Ok(true)
}
