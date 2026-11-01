use crate::p5_types::initialization::{ChainArgument, ChainKind};
use crate::p6_variables::normal::StorageBody;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};

pub(crate) struct PrologueArgument {
    pub origin: ChainArgument,
    pub view: super::branch::ValueUse,
}
pub(crate) struct Prologue {
    _sealed: (),
    pub contract: crate::p4_calls::allocation::ConstructorContract,
    pub kind: ChainKind,
    pub pc: CodeUnitOffset,
    pub receiver: RegisterId,
    pub arguments: Vec<PrologueArgument>,
}
pub(super) fn lower(
    storage: &StorageBody<'_>,
    calls: &mut super::calls::SourceCalls,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Prologue>, RequestError> {
    let Some(entry) = storage.constructor_entry.as_ref() else {
        return Ok(None);
    };
    let chain =
        storage
            .typed
            .initializations
            .chain
            .as_ref()
            .ok_or(RequestError::InternalInvariant(
                "constructor entry lost chain",
            ))?;
    let index = usize::try_from(chain.call.0).map_err(|_| RequestError::PlatformCapacity)?;
    let contract = calls
        .constructors
        .get_mut(index)
        .and_then(Option::take)
        .ok_or(RequestError::InternalInvariant(
            "constructor entry lost binding",
        ))?;
    let mut arguments = Vec::new();
    let site = &storage.typed.ssa.calls[index];
    let uses = storage.typed.call_uses[index].arguments(site.reference, site.pc)?;
    if uses.len() != entry.arguments.len() {
        return Err(RequestError::InternalInvariant(
            "constructor origin/use arity changed",
        ));
    }
    for (&origin, usage) in entry.arguments.iter().zip(uses) {
        budget.charge_work(1)?;
        crate::substrate::owned::push(
            &mut arguments,
            PrologueArgument {
                origin,
                view: super::values::lower(super::branch::LocalId(usage.value.0), usage.conversion),
            },
            budget,
        )?;
    }
    Ok(Some(Prologue {
        _sealed: (),
        contract,
        kind: entry.kind,
        pc: entry.pc,
        receiver: entry.receiver,
        arguments,
    }))
}
pub(super) fn verify(
    storage: &StorageBody<'_>,
    actual: Option<&Prologue>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let (entry, actual) = match (storage.constructor_entry.as_ref(), actual) {
        (None, None) => return Ok(true),
        (Some(entry), Some(actual)) => (entry, actual),
        _ => return Ok(false),
    };
    let chain =
        storage
            .typed
            .initializations
            .chain
            .as_ref()
            .ok_or(RequestError::InternalInvariant(
                "constructor certificate lost chain",
            ))?;
    let index = usize::try_from(chain.call.0).map_err(|_| RequestError::PlatformCapacity)?;
    let Some(crate::p4_calls::normal::CallEvidence::Constructor {
        facts: original, ..
    }) = storage.typed.bindings.get(index)
    else {
        return Ok(false);
    };
    if actual.kind != entry.kind
        || actual.pc != entry.pc
        || actual.receiver != entry.receiver
        || actual.contract.reference != entry.reference
        || actual.contract.owner != original.owner
        || actual.contract.prototype != original.prototype
        || actual.arguments.len() != entry.arguments.len()
        || actual.arguments.len() != actual.contract.arity
    {
        return Ok(false);
    }
    let Some(formals) = original.formals.as_deref() else {
        return Ok(false);
    };
    let op = storage.typed.ssa.call(chain.call)?;
    if formals.len() != actual.arguments.len()
        || op.inputs.len()
            != actual
                .arguments
                .len()
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?
    {
        return Ok(false);
    }
    for (((value, &expected), &source), &formal) in actual
        .arguments
        .iter()
        .zip(&entry.arguments)
        .zip(op.inputs.iter().skip(1))
        .zip(formals)
    {
        budget.charge_work(1)?;
        if value.origin != expected
            || !super::dispatcher_verify::call_argument(&storage.typed, value.view, source, formal)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
