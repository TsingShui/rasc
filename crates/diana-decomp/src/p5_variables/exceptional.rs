use super::normal::{Initializer, Local, local_type};
use super::{Copy, CopyPlan, LocalId};
use crate::p2_flow::protection::ExceptionEdgeId;
use crate::p2_flow::{BlockId, OpId};
use crate::p3_ssa::{Definition, Incoming, Sym, ValueId};
use crate::p4_types::TypedBody;
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RequestError, RunContext};

pub(crate) struct Shadow {
    pub handler: BlockId,
    pub phi: ValueId,
    pub local: LocalId,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Snapshot {
    pub edge: ExceptionEdgeId,
    pub phi: ValueId,
    pub source_block: BlockId,
    pub before: CodeUnitOffset,
    pub copy: Copy,
}
pub(crate) struct Ingress {
    pub handler: BlockId,
    pub bind_exception: OpId,
    pub copies: CopyPlan,
}
pub(crate) struct ExceptionalStorage {
    _sealed: (),
    first_shadow: usize,
    pub shadows: Vec<Shadow>,
    pub snapshots: Vec<Snapshot>,
    pub ingress: Vec<Ingress>,
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn invariant(message: &'static str) -> RequestError {
    RequestError::InternalInvariant(message)
}
fn types(
    locals: &[Local],
    budget: &mut RunContext<'_>,
) -> Result<Vec<super::LocalType>, RequestError> {
    let mut result = Vec::new();
    for local in locals {
        budget.checkpoint()?;
        push(&mut result, local_type(local.kind)?, budget)?;
    }
    Ok(result)
}
pub(crate) fn build(
    typed: &TypedBody<'_>,
    locals: &mut Vec<Local>,
    budget: &mut RunContext<'_>,
) -> Result<Option<ExceptionalStorage>, RequestError> {
    let mut result = ExceptionalStorage {
        _sealed: (),
        first_shadow: locals.len(),
        shadows: Vec::new(),
        snapshots: Vec::new(),
        ingress: Vec::new(),
    };
    for binding in &typed.exceptions {
        budget.checkpoint()?;
        let mut copies = Vec::new();
        for phi in &typed.ssa.phis {
            budget.checkpoint()?;
            if phi.block != binding.handler {
                continue;
            }
            let kind = typed.kinds[ix(phi.result.0)?].ok_or(invariant("handler Phi lost type"))?;
            let local =
                LocalId(u32::try_from(locals.len()).map_err(|_| RequestError::PlatformCapacity)?);
            push(
                locals,
                Local {
                    id: local,
                    kind,
                    initializer: Initializer::Default,
                },
                budget,
            )?;
            push(
                &mut result.shadows,
                Shadow {
                    handler: phi.block,
                    phi: phi.result,
                    local,
                },
                budget,
            )?;
            push(
                &mut copies,
                Copy {
                    target: LocalId(phi.result.0),
                    source: crate::p5_variables::CopySource::Local(local),
                },
                budget,
            )?;
            for &(incoming, value) in &phi.inputs {
                budget.checkpoint()?;
                let (Incoming::Exceptional(edge), Sym::Value(source)) = (incoming, value) else {
                    return Err(invariant("handler Phi has non-exceptional ingress"));
                };
                let original = typed
                    .ssa
                    .exception_edges
                    .get(ix(edge.0)?)
                    .ok_or(invariant("snapshot lost edge"))?;
                if typed.kinds.get(ix(source.0)?) != Some(&Some(kind))
                    || !available(typed, source, original.source, original.pc, budget)?
                {
                    return Ok(None);
                }
                push(
                    &mut result.snapshots,
                    Snapshot {
                        edge,
                        phi: phi.result,
                        source_block: original.source,
                        before: original.pc,
                        copy: Copy {
                            target: local,
                            source: crate::p5_variables::CopySource::Local(LocalId(source.0)),
                        },
                    },
                    budget,
                )?;
            }
        }
        let plan = super::plan(&copies, &types(locals, budget)?, budget)?;
        if !plan.temporaries.is_empty() {
            return Err(invariant("exclusive shadows acquired a copy cycle"));
        }
        push(
            &mut result.ingress,
            Ingress {
                handler: binding.handler,
                bind_exception: binding.operation,
                copies: plan,
            },
            budget,
        )?;
    }
    verify(typed, locals, &result, budget)?;
    Ok(Some(result))
}

// P3 proves entry availability. P6 additionally requires materialized storage at
// this exact site: a successful throwing result cannot satisfy its own snapshot.
fn available(
    typed: &TypedBody<'_>,
    source: ValueId,
    block: BlockId,
    before: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    if typed
        .initializations
        .deferred
        .get(ix(source.0)?)
        .is_some_and(Option::is_some)
    {
        return Ok(false);
    }
    match typed.ssa.values.get(ix(source.0)?) {
        Some(Definition::Parameter(_)) => Ok(true),
        Some(Definition::Phi) => {
            for phi in &typed.ssa.phis {
                budget.checkpoint()?;
                if phi.result == source {
                    return Ok(phi.block == block);
                }
            }
            Ok(false)
        }
        Some(Definition::OpResult(id) | Definition::ExceptionAt(id)) => {
            for &candidate in &typed.ssa.bound[ix(block.0)?].ops {
                budget.checkpoint()?;
                if candidate == *id {
                    return Ok(typed.ssa.operation(candidate)?.pc.0 < before.0);
                }
            }
            Ok(false)
        }
        None => Ok(false),
    }
}

pub(crate) fn verify(
    typed: &TypedBody<'_>,
    locals: &[Local],
    plan: &ExceptionalStorage,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let mut handlers = 0usize;
    for (index, block) in typed.ssa.bound.iter().enumerate() {
        budget.checkpoint()?;
        let Some(&id) = block.ops.first() else {
            continue;
        };
        let op = typed.ssa.operation(id)?;
        if !matches!(op.op, crate::p2_flow::Op::Exception(_)) {
            continue;
        }
        handlers = handlers
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
        let handler = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut binding = None;
        for candidate in &typed.exceptions {
            budget.checkpoint()?;
            if candidate.handler == handler && binding.replace(candidate).is_some() {
                return Err(invariant("duplicate typed handler binding"));
            }
        }
        let binding = binding.ok_or(invariant("handler lost typed binding"))?;
        let value = op
            .result
            .ok_or(invariant("catch binding lost definition"))?;
        let expected = binding.kind.map(|kind| match kind {
            crate::p4_types::declarations::exceptions::ExceptionType::Nominal(id) => {
                crate::p4_types::Kind::Nominal(id)
            }
            crate::p4_types::declarations::exceptions::ExceptionType::Throwable => {
                crate::p4_types::Kind::Throwable
            }
        });
        if expected.is_none()
            || typed.kinds.get(ix(value.0)?) != Some(&expected)
            || typed.ssa.values.get(ix(value.0)?) != Some(&Definition::ExceptionAt(id))
            || binding.operation != id
        {
            return Err(invariant("catch binding changed its typed definition"));
        }
    }
    if handlers != typed.exceptions.len() {
        return Err(invariant("typed handler coverage changed"));
    }
    if typed.exceptions.is_empty() {
        if !plan.shadows.is_empty() || !plan.snapshots.is_empty() || !plan.ingress.is_empty() {
            return Err(invariant("handler-free method acquired exception copies"));
        }
        return Ok(());
    }
    let local_types = types(locals, budget)?;
    if plan.first_shadow < typed.ssa.values.len()
        || plan.first_shadow.checked_add(plan.shadows.len()) != Some(locals.len())
    {
        return Err(invariant("shadow arena overlaps ordinary storage"));
    }
    for (index, shadow) in plan.shadows.iter().enumerate() {
        budget.checkpoint()?;
        if plan.first_shadow.checked_add(index) != Some(ix(shadow.local.0)?) {
            return Err(invariant("shadow aliases another storage slot"));
        }
    }
    let mut expected_shadows = 0usize;
    let mut expected_snapshots = 0usize;
    if plan.ingress.len() != typed.exceptions.len() {
        return Err(invariant("exception ingress coverage changed"));
    }
    for binding in &typed.exceptions {
        budget.checkpoint()?;
        let mut ingress = None;
        for candidate in &plan.ingress {
            budget.checkpoint()?;
            if candidate.handler == binding.handler && ingress.replace(candidate).is_some() {
                return Err(invariant("duplicate handler ingress"));
            }
        }
        let ingress = ingress.ok_or(invariant("missing handler ingress"))?;
        if ingress.bind_exception != binding.operation
            || !matches!(
                typed.ssa.operation(ingress.bind_exception)?.op,
                crate::p2_flow::Op::Exception(_)
            )
        {
            return Err(invariant("ingress lost catch parameter binding"));
        }
        let mut copies = Vec::new();
        for phi in &typed.ssa.phis {
            budget.checkpoint()?;
            if phi.block != binding.handler {
                continue;
            }
            expected_shadows = expected_shadows
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            let mut shadow = None;
            for candidate in &plan.shadows {
                budget.checkpoint()?;
                if candidate.handler == phi.block
                    && candidate.phi == phi.result
                    && shadow.replace(candidate).is_some()
                {
                    return Err(invariant("duplicate handler Phi shadow"));
                }
            }
            let shadow = shadow.ok_or(invariant("missing handler Phi shadow"))?;
            let local = locals
                .get(ix(shadow.local.0)?)
                .ok_or(invariant("foreign shadow local"))?;
            if ix(shadow.local.0)? < typed.ssa.values.len()
                || local.id != shadow.local
                || local.initializer != Initializer::Default
                || Some(local.kind) != typed.kinds[ix(phi.result.0)?]
            {
                return Err(invariant("shadow overlaps a definition or changes type"));
            }
            for other in &plan.shadows {
                budget.checkpoint()?;
                if other.local == shadow.local && !std::ptr::eq(other, shadow) {
                    return Err(invariant("handler shadows share a slot"));
                }
            }
            push(
                &mut copies,
                Copy {
                    target: LocalId(phi.result.0),
                    source: crate::p5_variables::CopySource::Local(shadow.local),
                },
                budget,
            )?;
            for (edge_index, edge) in typed.ssa.exception_edges.iter().enumerate() {
                budget.checkpoint()?;
                if edge.target != phi.block {
                    continue;
                }
                expected_snapshots = expected_snapshots
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                let edge_id = ExceptionEdgeId(
                    u32::try_from(edge_index).map_err(|_| RequestError::PlatformCapacity)?,
                );
                let mut source = None;
                for &(incoming, value) in &phi.inputs {
                    budget.checkpoint()?;
                    if incoming == Incoming::Exceptional(edge_id) && source.replace(value).is_some()
                    {
                        return Err(invariant("duplicate exceptional source"));
                    }
                }
                let Some(Sym::Value(source)) = source else {
                    return Err(invariant("undefined snapshot source"));
                };
                let mut snapshot = None;
                for candidate in &plan.snapshots {
                    budget.checkpoint()?;
                    if candidate.edge == edge_id
                        && candidate.phi == phi.result
                        && snapshot.replace(candidate).is_some()
                    {
                        return Err(invariant("duplicate throw snapshot"));
                    }
                }
                let snapshot = snapshot.ok_or(invariant("missing throw snapshot"))?;
                if snapshot.source_block != edge.source
                    || snapshot.before != edge.pc
                    || snapshot.copy
                        != (Copy {
                            target: shadow.local,
                            source: crate::p5_variables::CopySource::Local(LocalId(source.0)),
                        })
                    || !available(typed, source, edge.source, edge.pc, budget)?
                    || typed.kinds.get(ix(source.0)?) != typed.kinds.get(ix(phi.result.0)?)
                    || local_types.get(ix(source.0)?) != local_types.get(ix(shadow.local.0)?)
                {
                    return Err(invariant(
                        "snapshot changed pre-throw source, type or placement",
                    ));
                }
            }
        }
        super::verify(&copies, &local_types, &ingress.copies, budget)?;
    }
    if plan.shadows.len() != expected_shadows || plan.snapshots.len() != expected_snapshots {
        return Err(invariant("exception storage has foreign copies"));
    }
    Ok(())
}
