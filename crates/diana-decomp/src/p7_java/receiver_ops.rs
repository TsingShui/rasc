use crate::p2_flow::{CallId, InvokeKind};
use crate::p4_types::TypedBody;
use crate::p4_types::bindings::CallEvidence;
use crate::p5_variables::LocalId;
use crate::{RequestError, RunContext};

pub(super) fn verify(
    typed: &TypedBody<'_>,
    id: CallId,
    actual: &super::calls::ReceiverCall,
    budget: &mut RunContext<'_>,
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
        InvokeKind::Direct => crate::p4_types::declarations::ReceiverKind::Direct,
        InvokeKind::Super => crate::p4_types::declarations::ReceiverKind::Super,
        InvokeKind::Virtual => crate::p4_types::declarations::ReceiverKind::Virtual,
        InvokeKind::Interface => crate::p4_types::declarations::ReceiverKind::Interface,
        InvokeKind::Static => return Ok(false),
    };
    let Some(receiver) = typed
        .call_uses
        .get(index)
        .and_then(|uses| uses.receiver.as_ref())
    else {
        return Ok(false);
    };
    match (receiver, actual.receiver) {
        (
            crate::p4_types::calls::TypedReceiver::Super { value },
            super::calls::CallReceiver::Super(local),
        ) if site.kind == InvokeKind::Super && local.0 == value.0 => {}
        (
            crate::p4_types::calls::TypedReceiver::Value(usage),
            super::calls::CallReceiver::Value(view),
        ) if site.kind != InvokeKind::Super
            && view == super::values::lower(LocalId(usage.value.0), usage.conversion) =>
        {
            if !super::values::verify_reference_store(
                typed,
                view,
                usage.value,
                crate::p2_flow::DeclarationType::Object,
                budget,
            )? {
                return Ok(false);
            }
        }
        _ => return Ok(false),
    }
    if actual.contract.kind != expected
        || actual.contract.reference != site.reference
        || actual.contract.owner != facts.owner
        || actual.contract.name != facts.name
        || actual.contract.prototype != facts.prototype
        || actual.invoke != site.pc
        || actual.result_site != site.result_pc
        || actual.receiver.value().0 != receiver.value().0
        || matches!(actual.receiver, super::calls::CallReceiver::Super(_))
            != (expected == crate::p4_types::declarations::ReceiverKind::Super)
        || bound.inputs.first() != Some(&receiver.value())
        || site.registers.first() != Some(&actual.original_receiver)
    {
        return Ok(false);
    }
    let formals = &site.formals;
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
        budget.checkpoint()?;
        if name.next() != Some(unit) {
            return Ok(false);
        }
    }
    if name.next().is_some() {
        return Ok(false);
    }
    for ((&view, value), formal) in actual.arguments.iter().zip(values).zip(formals) {
        budget.checkpoint()?;
        if !super::dispatcher_verify::call_argument(typed, view, *value, *formal, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}
