use crate::p6_variables::exceptional::{Ingress, Snapshot};
use crate::p6_variables::normal::StorageBody;
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

pub(super) fn exception_root(
    typed: &crate::p5_types::normal::TypedBody<'_>,
    value: crate::p3_ssa::normal::ValueId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<crate::p2_flow::normal::BlockId>, RequestError> {
    use crate::p3_ssa::normal::{Definition, Sym};
    let mut todo = Vec::new();
    let mut seen = Vec::new();
    let mut root = None;
    push(&mut todo, value, budget)?;
    while let Some(value) = todo.pop() {
        budget.charge_work(1)?;
        let mut visited = false;
        for &old in &seen {
            budget.charge_work(1)?;
            if old == value {
                visited = true;
                break;
            }
        }
        if visited {
            continue;
        }
        push(&mut seen, value, budget)?;
        match typed.ssa.values
            [usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
        {
            Definition::ExceptionAt(id) => {
                let mut binding = None;
                for candidate in &typed.exceptions {
                    budget.charge_work(1)?;
                    if candidate.operation == id {
                        binding = Some(candidate);
                        break;
                    }
                }
                let Some(binding) = binding else {
                    return Ok(None);
                };
                if root.is_some_and(|old| old != binding.handler) {
                    return Ok(None);
                }
                root = Some(binding.handler);
            }
            Definition::OpResult(id) => {
                let op = typed.ssa.operation(id)?;
                if !matches!(op.op, crate::p2_flow::normal::Op::Move(..)) {
                    return Ok(None);
                }
                push(&mut todo, op.input(0)?, budget)?;
            }
            Definition::Phi => {
                let mut phi = None;
                for candidate in &typed.ssa.phis {
                    budget.charge_work(1)?;
                    if candidate.result == value {
                        phi = Some(candidate);
                        break;
                    }
                }
                let Some(phi) = phi else {
                    return Ok(None);
                };
                for &(_, source) in &phi.inputs {
                    budget.charge_work(1)?;
                    let Sym::Value(source) = source else {
                        return Ok(None);
                    };
                    push(&mut todo, source, budget)?;
                }
            }
            Definition::Parameter(_) => return Ok(None),
        }
    }
    Ok(root)
}

pub(super) fn snapshots(
    placements: &[Placement<'_>],
    block: crate::p2_flow::normal::BlockId,
    pc: crate::CodeUnitOffset,
    statements: &mut Vec<super::calls::Statement>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for placement in placements {
        budget.charge_work(1)?;
        let Placement::BeforeThrow(snapshot) = placement else {
            continue;
        };
        if snapshot.source_block == block && snapshot.before == pc {
            push(
                statements,
                super::calls::Statement::Snapshot(**snapshot),
                budget,
            )?;
        }
    }
    Ok(())
}

pub(super) fn placed_ingress<'a>(
    placements: &[Placement<'a>],
    handler: crate::p2_flow::normal::BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<&'a Ingress, RequestError> {
    for placement in placements {
        budget.charge_work(1)?;
        if let Placement::HandlerIngress(ingress) = placement
            && ingress.handler == handler
        {
            return Ok(ingress);
        }
    }
    Err(RequestError::InternalInvariant(
        "catch lost placement obligation",
    ))
}

pub(super) fn ingress<'a>(
    storage: &'a StorageBody<'_>,
    handler: crate::p2_flow::normal::BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<&'a Ingress, RequestError> {
    for ingress in &storage.exceptional.ingress {
        budget.charge_work(1)?;
        if ingress.handler == handler {
            return Ok(ingress);
        }
    }
    Err(RequestError::InternalInvariant(
        "handler lost checked ingress",
    ))
}

pub(super) enum Placement<'a> {
    BeforeThrow(&'a Snapshot),
    HandlerIngress(&'a Ingress),
}

pub(super) fn obligations<'a>(
    storage: &'a StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<Placement<'a>>, RequestError> {
    crate::p6_variables::exceptional::verify(
        &storage.typed,
        &storage.locals,
        &storage.exceptional,
        budget,
    )?;
    let mut result = Vec::new();
    for snapshot in &storage.exceptional.snapshots {
        budget.charge_work(1)?;
        push(&mut result, Placement::BeforeThrow(snapshot), budget)?;
    }
    for ingress in &storage.exceptional.ingress {
        budget.charge_work(1)?;
        push(&mut result, Placement::HandlerIngress(ingress), budget)?;
    }
    for placement in &result {
        budget.charge_work(1)?;
        match placement {
            Placement::BeforeThrow(snapshot) => {
                let edge = &storage.typed.ssa.exception_edges[usize::try_from(snapshot.edge.0)
                    .map_err(|_| RequestError::PlatformCapacity)?];
                let Some(chain) = storage.typed.ssa.protection.at(snapshot.before, budget)? else {
                    return Err(RequestError::InternalInvariant(
                        "snapshot left its protection domain",
                    ));
                };
                let clause = chain
                    .get(usize::try_from(edge.rank.0).map_err(|_| RequestError::PlatformCapacity)?)
                    .ok_or(RequestError::InternalInvariant("snapshot lost catch rank"))?;
                let handler = &storage.typed.ssa.blocks
                    [usize::try_from(edge.target.0).map_err(|_| RequestError::PlatformCapacity)?];
                if clause.target != handler.pc || clause.kind != edge.kind {
                    return Err(RequestError::InternalInvariant(
                        "snapshot changed catch ownership",
                    ));
                }
            }
            Placement::HandlerIngress(ingress) => {
                let bound = &storage.typed.ssa.bound[usize::try_from(ingress.handler.0)
                    .map_err(|_| RequestError::PlatformCapacity)?];
                if bound.ops.first() != Some(&ingress.bind_exception) {
                    return Err(RequestError::InternalInvariant(
                        "ingress precedes catch binding",
                    ));
                }
            }
        }
    }
    Ok(result)
}
