use crate::p6_variables::StorageBody;
use crate::p6_variables::exceptional::{Ingress, Snapshot};
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

pub(super) fn exception_root(
    typed: &crate::p5_types::TypedBody<'_>,
    value: crate::p3_ssa::ValueId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<crate::p2_flow::BlockId>, RequestError> {
    use crate::p3_ssa::Definition;
    use crate::p5_types::reference_roots::Root;
    budget.charge_work(1)?;
    let Some(Root::Definition(root)) = typed.roots.one(value)? else {
        return Ok(None);
    };
    let Definition::ExceptionAt(id) =
        typed.ssa.values[usize::try_from(root.0).map_err(|_| RequestError::PlatformCapacity)?]
    else {
        return Ok(None);
    };
    for binding in &typed.exceptions {
        budget.charge_work(1)?;
        if binding.operation == id {
            return Ok(Some(binding.handler));
        }
    }
    Ok(None)
}

pub(super) fn snapshots(
    placements: &[Placement<'_>],
    block: crate::p2_flow::BlockId,
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
    handler: crate::p2_flow::BlockId,
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
    handler: crate::p2_flow::BlockId,
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
                let mut selection = snapshot.before;
                for op in &storage.typed.ssa.blocks
                    [usize::try_from(edge.source.0).map_err(|_| RequestError::PlatformCapacity)?]
                .ops
                {
                    budget.charge_work(1)?;
                    if op.pc == snapshot.before {
                        selection = crate::p2_flow::protection::selection_pc(op.kind, op.pc)?;
                        break;
                    }
                }
                let Some(chain) = storage.typed.ssa.protection.at(selection, budget)? else {
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
