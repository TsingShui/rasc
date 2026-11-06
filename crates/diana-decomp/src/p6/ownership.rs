//! Selects each node's execution owner and decides how an exception reaches its
//! handler. A try whose routes are not uniformly caught falls back to an explicit
//! graph with a protection-boundary reason; a lock scope whose exits do not stay
//! inside their loop becomes explicit the same way. Nothing here changes an
//! original edge, handler priority or lock order.

use super::build::*;
use super::graph::Graph;
use super::protection;
use super::*;
use crate::p5::{OperationEmission, StorageMethod};

pub(super) fn deliveries(f: &RegionFacts) -> Result<Vec<ExceptionDelivery>, RegionError> {
    let mut out = filled(f.edges.len(), ExceptionDelivery::Explicit)?;
    for e in &f.edges {
        if let EdgeRoute::Exception {
            delivery: ExceptionDelivery::ImpossibleRelease(op),
            ..
        } = e.route
        {
            out[e.edge.0 as usize] = ExceptionDelivery::ImpossibleRelease(op);
        }
    }
    for r in &f.regions {
        if let RegionKind::Try(id) = r.kind {
            let p = &f.tries[id.0 as usize];
            for t in &f.throws {
                if !p.body.contains(&t.node) {
                    continue;
                }
                let actual = protection::handlers(&f.chains[t.chain.0 as usize]);
                let wanted = &f.chains[p.chain.0 as usize].clauses
                    [p.clause_range.start as usize..p.clause_range.end as usize];
                if !protection::suffix(actual, wanted) {
                    continue;
                }
                let start = actual.len() - wanted.len();
                for &edge in &t.edges[start..actual.len()] {
                    if !matches!(
                        out[edge.0 as usize],
                        ExceptionDelivery::ImpossibleRelease(_)
                    ) {
                        out[edge.0 as usize] = ExceptionDelivery::Catch(id);
                    }
                }
            }
        }
    }
    Ok(out)
}
pub(super) fn protect(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
) -> Result<(), RegionError> {
    loop {
        let selected = deliveries(f)?;
        let mut withdraw = filled(f.regions.len(), false)?;
        for t in &f.throws {
            let mut explicit = false;
            let mut natural = false;
            for (&edge, c) in t.edges.iter().zip(&f.chains[t.chain.0 as usize].clauses) {
                if c.target == CatchTarget::Uncaught {
                    continue;
                }
                match selected[edge.0 as usize] {
                    ExceptionDelivery::Explicit => explicit = true,
                    ExceptionDelivery::Catch(_) => natural = true,
                    _ => {}
                }
            }
            if !explicit || !natural {
                continue;
            }
            for &edge in &t.edges {
                for r in &f.regions {
                    if matches!((&r.kind,selected[edge.0 as usize]),(RegionKind::Try(a),ExceptionDelivery::Catch(b))if *a==b)
                    {
                        withdraw[r.id.0 as usize] = true;
                    }
                }
            }
        }
        if !withdraw.iter().any(|x| *x) {
            for e in &mut f.edges {
                if let EdgeRoute::Exception { delivery, .. } = &mut e.route {
                    *delivery = selected[e.edge.0 as usize];
                }
            }
            for t in &mut f.throws {
                t.protection = if t
                    .edges
                    .iter()
                    .any(|e| matches!(selected[e.0 as usize], ExceptionDelivery::Catch(_)))
                {
                    SiteProtection::Natural
                } else {
                    SiteProtection::Explicit
                };
            }
            return Ok(());
        }
        for (i, &drop) in withdraw.iter().enumerate() {
            if !drop {
                continue;
            }
            let r = &f.regions[i];
            let kind = super::compose::parts::graph_kind(
                storage,
                f,
                g,
                &r.nodes,
                r.entry,
                RecoveryReason::ProtectionBoundary,
            )?;
            let entry = r.entry;
            f.regions[i].kind = kind;
            if let Some(entry) = entry {
                push(
                    &mut f.issues,
                    PlanningIssue::Graph {
                        entry,
                        reason: RecoveryReason::ProtectionBoundary,
                    },
                )?;
            }
        }
    }
}
pub(super) fn scope_contexts(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
) -> Result<(), RegionError> {
    for index in 0..f.regions.len() {
        let RegionKind::Synchronized(scope) = f.regions[index].kind else {
            continue;
        };
        let mut failure = None;
        for exit in &f.synchronized[scope.0 as usize].exits {
            let SyncExitKind::Loop { loop_id, .. } = exit.kind else {
                continue;
            };
            let mut at = f.parents[index];
            let mut found = false;
            while let Some(parent) = at {
                if matches!(f.regions[parent.0 as usize].kind,RegionKind::Loop(id)if id==loop_id) {
                    found = true;
                    break;
                }
                at = f.parents[parent.0 as usize];
            }
            if !found {
                failure = Some(LockSeparation::LoopContext(loop_id));
                break;
            }
        }
        if let Some(reason) = failure {
            let r = &f.regions[index];
            let entry = r.entry;
            let kind = super::compose::parts::graph_kind(
                storage,
                f,
                g,
                &r.nodes,
                entry,
                RecoveryReason::LockBoundary,
            )?;
            f.regions[index].kind = kind;
            f.synchronized[scope.0 as usize].separation = Some(reason);
            if let Some(entry) = entry {
                push(
                    &mut f.issues,
                    PlanningIssue::Graph {
                        entry,
                        reason: RecoveryReason::LockBoundary,
                    },
                )?;
            }
        }
    }
    Ok(())
}
pub(super) fn own(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
) -> Result<(), RegionError> {
    for op in storage.execution().operations() {
        let node = g.op_node[op.id().0 as usize];
        let owner = match storage
            .facts()
            .operation_emission(op.id())
            .ok_or(bug("P6 operation has no P5 emission owner"))?
        {
            OperationEmission::Ordinary => OperationOwner::Ordinary {
                node,
                region: f.labels[node.0 as usize].owner,
            },
            OperationEmission::Construction { group, role } => {
                OperationOwner::Construction { group, role }
            }
        };
        push(&mut f.operation_owners, owner)?;
    }
    for b in storage.execution().blocks() {
        push(
            &mut f.terminal_owners,
            TerminalOwner::Ordinary(f.labels[g.terminal_node[b.id().0 as usize].0 as usize].owner),
        )?;
    }
    for r in &f.regions {
        match &r.kind {
            RegionKind::If { .. } => {
                let node = r.entry.ok_or(bug("if has no condition entry"))?;
                f.terminal_owners[f.nodes[node.0 as usize].block.0 as usize] =
                    TerminalOwner::IfCondition(r.id);
            }
            RegionKind::Switch { .. } => {
                let node = r.entry.ok_or(bug("switch has no selector entry"))?;
                f.terminal_owners[f.nodes[node.0 as usize].block.0 as usize] =
                    TerminalOwner::SwitchSelector(r.id);
            }
            RegionKind::Loop(id) => {
                if let Some(node) = f.loops[id.0 as usize].condition() {
                    f.terminal_owners[f.nodes[node.0 as usize].block.0 as usize] =
                        TerminalOwner::LoopCondition(*id);
                }
            }
            RegionKind::Try(id) => {
                let p = &f.tries[id.0 as usize];
                for &handler in &p.handlers {
                    if let Some(op) = f
                        .handlers
                        .iter()
                        .find(|h| h.node == handler)
                        .and_then(|h| h.capture)
                    {
                        f.operation_owners[op.0 as usize] = OperationOwner::CatchCapture(*id);
                    }
                }
            }
            RegionKind::Synchronized(id) => {
                let p = &f.synchronized[id.0 as usize];
                f.operation_owners[p.enter.0 as usize] = OperationOwner::Synchronized {
                    scope: *id,
                    role: MonitorRole::Acquire,
                };
                for exit in &p.exits {
                    f.operation_owners[exit.release.0 as usize] = OperationOwner::Synchronized {
                        scope: *id,
                        role: MonitorRole::Release,
                    };
                    for &node in &exit.routing {
                        let part = &f.nodes[node.0 as usize];
                        if part.terminal {
                            f.terminal_owners[part.block.0 as usize] =
                                TerminalOwner::SynchronizedExit(*id);
                        }
                    }
                    for &plan in exit.before.iter().chain(&exit.after) {
                        let owner = &mut f.copies[plan.0 as usize].owner;
                        match *owner {
                            CopyOwner::Original => *owner = CopyOwner::SynchronizedExit(*id),
                            CopyOwner::SynchronizedExit(at) if at == *id => {}
                            _ => return Err(bug("copy has two selected emission owners")),
                        }
                    }
                }
            }
            _ => {}
        }
    }
    for plan in &mut f.link_plans {
        plan.copies
            .retain(|id| f.copies[id.0 as usize].owner == CopyOwner::Original);
    }
    Ok(())
}
