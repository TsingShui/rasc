use super::branch::{LocalId, ValueUse};
use crate::p2_flow::{CallTarget, Op};
use crate::p3_ssa::BoundOp;
use crate::p4_types::initialization::InitializationId;
use crate::p5_variables::StorageBody;
use crate::{CodeUnitOffset, RegisterId, RequestError, RunContext};

pub(crate) struct Initialization {
    _sealed: (),
    pub contract: crate::p4_types::declarations::allocation::ConstructorContract,
    pub allocation: CodeUnitOffset,
    pub invoke: CodeUnitOffset,
    pub receiver: RegisterId,
    pub root: LocalId,
    pub aliases: Vec<LocalId>,
    pub arguments: Vec<ValueUse>,
    pub source_type: String,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn lower(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    contract: crate::p4_types::declarations::allocation::ConstructorContract,
    budget: &mut RunContext<'_>,
) -> Result<Option<Initialization>, RequestError> {
    let Op::Initialize(call) = op.op else {
        return Err(RequestError::InternalInvariant(
            "non-init entered constructor lowering",
        ));
    };
    let typed = &storage.typed;
    let index = ix(call.0)?;
    let id = typed
        .initializations
        .calls
        .get(index)
        .and_then(|id| *id)
        .ok_or(RequestError::InternalInvariant(
            "init lost normal-completion plan",
        ))?;
    let proof =
        storage
            .object_allocations
            .get(id.index()?)
            .ok_or(RequestError::InternalInvariant(
                "init lost deferred storage",
            ))?;
    let allocation = typed.ssa.operation(proof.allocation)?;
    let site = &typed.ssa.calls[index];
    let Some(source_type) = super::calls::type_name(&contract.owner, budget)? else {
        return Ok(None);
    };
    let mut aliases = Vec::new();
    for &alias in &proof.aliases {
        budget.checkpoint()?;
        crate::substrate::owned::push(&mut aliases, alias, budget)?;
    }
    let mut arguments = Vec::new();
    for argument in typed.call_uses[index].arguments() {
        budget.checkpoint()?;
        crate::substrate::owned::push(
            &mut arguments,
            super::values::lower(LocalId(argument.value.0), argument.conversion),
            budget,
        )?;
    }
    Ok(Some(Initialization {
        _sealed: (),
        contract,
        allocation: allocation.pc,
        invoke: op.pc,
        receiver: *site
            .registers
            .first()
            .ok_or(RequestError::InternalInvariant(
                "constructor lost raw receiver",
            ))?,
        root: proof.root,
        aliases,
        arguments,
        source_type,
    }))
}
pub(super) fn verify_deferred(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    id: InitializationId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
    let typed = &storage.typed;
    if id == InitializationId::This {
        let chain = typed
            .initializations
            .chain
            .as_ref()
            .ok_or(RequestError::InternalInvariant("deferred this lost entry"))?;
        let Some(result) = op.result else {
            return Ok(false);
        };
        return Ok(matches!(op.op, Op::Move(..))
            && typed.initializations.deferred.get(ix(result.0)?) == Some(&Some(id))
            && typed.initializations.deferred.get(ix(op.input(0)?.0)?) == Some(&Some(id))
            && op.pc.0 < typed.ssa.operation(chain.init)?.pc.0);
    }
    let proof =
        storage
            .object_allocations
            .get(id.index()?)
            .ok_or(RequestError::InternalInvariant(
                "deferred node lost storage proof",
            ))?;
    let Some(result) = op.result else {
        return Ok(false);
    };
    if typed.initializations.deferred.get(ix(result.0)?) != Some(&Some(id))
        || op.pc.0 >= typed.ssa.operation(proof.init)?.pc.0
    {
        return Ok(false);
    }
    Ok(match op.op {
        Op::NewInstance(..) => proof.allocation == op.id && proof.root.0 == result.0,
        Op::Move(..) => typed.initializations.deferred.get(ix(op.input(0)?.0)?) == Some(&Some(id)),
        _ => false,
    })
}
pub(super) fn verify(
    storage: &StorageBody<'_>,
    op: &BoundOp,
    actual: &Initialization,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Op::Initialize(call) = op.op else {
        return Ok(false);
    };
    let typed = &storage.typed;
    let index = ix(call.0)?;
    let site = typed
        .ssa
        .calls
        .get(index)
        .ok_or(RequestError::InternalInvariant(
            "constructor certificate lost site",
        ))?;
    let Some(crate::p4_types::bindings::CallEvidence::Constructor {
        facts: original, ..
    }) = typed.bindings.get(index)
    else {
        return Ok(false);
    };
    let id = typed
        .initializations
        .calls
        .get(index)
        .and_then(|id| *id)
        .ok_or(RequestError::InternalInvariant(
            "constructor certificate lost init plan",
        ))?;
    let proof =
        storage
            .object_allocations
            .get(id.index()?)
            .ok_or(RequestError::InternalInvariant(
                "constructor certificate lost storage",
            ))?;
    if site.target != CallTarget::Constructor
        || actual.contract.reference != site.reference
        || actual.contract.owner != original.owner
        || actual.contract.prototype != original.prototype
        || actual.allocation != typed.ssa.operation(proof.allocation)?.pc
        || actual.invoke != op.pc
        || actual.root != proof.root
        || actual.aliases.len() != proof.aliases.len()
        || site.registers.first() != Some(&actual.receiver)
        || op.result.is_some()
        || actual.arguments.len() != actual.contract.arity
        || actual.arguments.len().checked_add(1) != Some(op.inputs.len())
    {
        return Ok(false);
    }
    let mut source = actual.source_type.encode_utf16();
    if original.owner.len() < 3 {
        return Ok(false);
    }
    for &unit in &original.owner[1..original.owner.len() - 1] {
        budget.checkpoint()?;
        if source.next() != Some(if unit == 47 { 46 } else { unit }) {
            return Ok(false);
        }
    }
    if source.next().is_some() {
        return Ok(false);
    }
    for (&alias, &expected) in actual.aliases.iter().zip(&proof.aliases) {
        budget.checkpoint()?;
        if alias != expected || typed.initializations.deferred.get(ix(alias.0)?) != Some(&Some(id))
        {
            return Ok(false);
        }
    }
    let formals = &site.formals;
    if formals.len() != actual.arguments.len() {
        return Ok(false);
    }
    for ((&view, value), &formal) in actual
        .arguments
        .iter()
        .zip(op.inputs.iter().skip(1))
        .zip(formals)
    {
        budget.checkpoint()?;
        if !super::dispatcher_verify::call_argument(typed, view, *value, formal, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}
