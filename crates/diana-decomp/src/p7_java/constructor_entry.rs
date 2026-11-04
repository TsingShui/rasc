use crate::p4_types::initialization::{ChainArgument, ChainKind};
use crate::p5_variables::StorageBody;
use crate::{CodeUnitOffset, RegisterId, RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Invocation {
    This,
    Super,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Argument {
    Parameter(u32),
    Int(i32),
    Long(i64),
}
pub(crate) struct PrologueArgument {
    pub(super) origin: ChainArgument,
    pub source: Argument,
    pub view: super::branch::ValueUse,
}
pub(crate) struct Prologue {
    _sealed: (),
    pub contract: crate::p4_types::declarations::allocation::ConstructorContract,
    pub kind: Invocation,
    pub pc: CodeUnitOffset,
    pub receiver: RegisterId,
    pub arguments: Vec<PrologueArgument>,
}
pub(super) fn lower(
    storage: &StorageBody<'_>,
    calls: &mut super::calls::SourceCalls,
    budget: &mut RunContext<'_>,
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
    let uses = storage.typed.call_uses[index].arguments();
    if uses.len() != entry.arguments.len() {
        return Err(RequestError::InternalInvariant(
            "constructor origin/use arity changed",
        ));
    }
    for (&origin, usage) in entry.arguments.iter().zip(uses) {
        budget.checkpoint()?;
        crate::substrate::owned::push(
            &mut arguments,
            PrologueArgument {
                origin,
                source: match origin {
                    ChainArgument::Parameter(index) => Argument::Parameter(index),
                    ChainArgument::Literal(value) => Argument::Int(value),
                    ChainArgument::WideLiteral(value) => Argument::Long(value),
                },
                view: super::values::lower(super::branch::LocalId(usage.value.0), usage.conversion),
            },
            budget,
        )?;
    }
    Ok(Some(Prologue {
        _sealed: (),
        contract,
        kind: match entry.kind {
            ChainKind::This => Invocation::This,
            ChainKind::Super => Invocation::Super,
        },
        pc: entry.pc,
        receiver: entry.receiver,
        arguments,
    }))
}
pub(super) fn verify(
    storage: &StorageBody<'_>,
    actual: Option<&Prologue>,
    budget: &mut RunContext<'_>,
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
    let Some(crate::p4_types::bindings::CallEvidence::Constructor {
        facts: original, ..
    }) = storage.typed.bindings.get(index)
    else {
        return Ok(false);
    };
    if !matches!(
        (actual.kind, entry.kind),
        (Invocation::This, ChainKind::This) | (Invocation::Super, ChainKind::Super)
    ) || actual.pc != entry.pc
        || actual.receiver != entry.receiver
        || actual.contract.reference != entry.reference
        || actual.contract.owner != original.owner
        || actual.contract.prototype != original.prototype
        || actual.arguments.len() != entry.arguments.len()
        || actual.arguments.len() != actual.contract.arity
    {
        return Ok(false);
    }
    let formals = &storage.typed.ssa.calls
        [usize::try_from(chain.call.0).map_err(|_| RequestError::PlatformCapacity)?]
    .formals;
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
        budget.checkpoint()?;
        if value.origin != expected
            || !matches!((value.source,expected),(Argument::Parameter(a),ChainArgument::Parameter(b)) if a==b)
                && !matches!((value.source,expected),(Argument::Int(a),ChainArgument::Literal(b)) if a==b)
                && !matches!((value.source,expected),(Argument::Long(a),ChainArgument::WideLiteral(b)) if a==b)
            || !super::dispatcher_verify::call_argument(
                &storage.typed,
                value.view,
                source,
                formal,
                budget,
            )?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
