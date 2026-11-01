use crate::p5_types::initialization::{ChainArgument, ChainKind};
use crate::p6_variables::normal::StorageBody;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};

pub(crate) struct Prologue {
    _sealed: (),
    pub contract: crate::p4_calls::allocation::ConstructorContract,
    pub kind: ChainKind,
    pub pc: CodeUnitOffset,
    pub receiver: RegisterId,
    pub arguments: Vec<ChainArgument>,
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
    for &argument in &entry.arguments {
        budget.charge_work(1)?;
        crate::substrate::owned::push(&mut arguments, argument, budget)?;
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
    let Some(crate::p4_calls::normal::CallEvidence::Constructor(original)) =
        storage.typed.bindings.get(index)
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
        || actual.arguments.len() != actual.contract.int_arity
    {
        return Ok(false);
    }
    for (&value, &expected) in actual.arguments.iter().zip(&entry.arguments) {
        budget.charge_work(1)?;
        if !matches!((value,expected),(ChainArgument::Parameter(a),ChainArgument::Parameter(b)) if a==b)
            && !matches!((value,expected),(ChainArgument::Literal(a),ChainArgument::Literal(b)) if a==b)
        {
            return Ok(false);
        }
    }
    Ok(true)
}
