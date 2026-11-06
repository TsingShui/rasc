//! Closed held subgraphs and original release outlets. Syntax selection never
//! reschedules P5 copies or treats post-release work as locked body operations.
use super::build::*;
use super::graph::Graph;
use super::locks::{LockPositions, root};
use super::protection;
use super::*;
use crate::p2::{OpId, OpKind, Terminal};
use crate::p4::{Family, Interpretation};
use crate::p5::{CopyLocation, Materialization, PlanId, StorageMethod, ThrowSite, TransferSite};

fn nodes(set: &[bool]) -> Result<Vec<NodeId>, RegionError> {
    let mut out = Vec::new();
    for (i, &inside) in set.iter().enumerate() {
        if inside {
            push(&mut out, NodeId(id(i)?))?;
        }
    }
    Ok(out)
}
fn source_exception(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    held: &[bool],
    terminal: NodeId,
) -> Option<(OpId, crate::p5::StorageRead)> {
    let part = &f.nodes[terminal.0 as usize];
    let uses = storage.facts().terminal_uses(part.block)?;
    let [source] = uses else {
        return None;
    };
    let read = source.read().ok()?;
    if !matches!(
        read,
        crate::p5::StorageRead::Value {
            view: Interpretation::Direct(Family::Reference),
            ..
        }
    ) {
        return None;
    }
    let value = root(storage, read)?;
    let local = storage.facts().binding(value)?.local()?;
    let capture = storage.facts().definitions().iter().find_map(|d| {
        matches!(d.materialization(),Materialization::ExceptionCapture(l)if l==local)
            .then_some(d.operation())
    })?;
    if !held[g.op_node[capture.0 as usize].0 as usize]
        || !f.handlers.iter().any(|h| h.capture == Some(capture))
    {
        return None;
    }
    Some((capture, read))
}
fn route(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    held: &[bool],
    enter: NodeId,
    release: OpId,
) -> Result<SyncExit, RegionError> {
    let at = g.op_node[release.0 as usize];
    let [link] = g.outgoing[at.0 as usize].as_slice() else {
        return Err(bug("release has no unique normal continuation"));
    };
    let target = f.links[link.0 as usize].to;
    let mut node = target;
    let mut routing = Vec::new();
    let mut seen = filled(f.nodes.len(), false)?;
    let kind = loop {
        if seen[node.0 as usize] || held[node.0 as usize] || node == enter {
            routing.clear();
            break SyncExitKind::Follow(target);
        }
        seen[node.0 as usize] = true;
        let context = f
            .loops
            .iter()
            .filter(|p| p.members.contains(&enter))
            .filter_map(|p| {
                if p.continue_target() == node {
                    Some((p, SyncLoopTransfer::Continue))
                } else if p.follow == Some(node) && !p.members.contains(&node) {
                    Some((p, SyncLoopTransfer::Break))
                } else {
                    None
                }
            })
            .min_by_key(|(p, _)| p.members.len());
        if let Some((p, kind)) = context {
            break SyncExitKind::Loop {
                loop_id: p.id,
                kind,
                target: node,
            };
        }
        let part = &f.nodes[node.0 as usize];
        if !part.operations.is_empty() {
            break SyncExitKind::Follow(node);
        }
        let terminal = part
            .terminal
            .then(|| storage.execution().blocks()[part.block.0 as usize].terminal())
            .flatten();
        match terminal {
            Some(Terminal::Return { width: None, .. }) => {
                if storage
                    .facts()
                    .terminal_uses(part.block)
                    .is_none_or(|u| !u.is_empty())
                {
                    break SyncExitKind::Follow(node);
                }
                push(&mut routing, node)?;
                break SyncExitKind::ReturnVoid { terminal: node };
            }
            Some(Terminal::Throw { .. }) => {
                if let Some((capture, exception)) = source_exception(storage, f, g, held, node) {
                    push(&mut routing, node)?;
                    break SyncExitKind::Rethrow {
                        terminal: node,
                        capture,
                        exception,
                    };
                }
                break SyncExitKind::Follow(node);
            }
            Some(Terminal::Goto { .. }) | None => {}
            _ => break SyncExitKind::Follow(node),
        }
        let [next] = g.outgoing[node.0 as usize].as_slice() else {
            break SyncExitKind::Follow(node);
        };
        push(&mut routing, node)?;
        node = f.links[next.0 as usize].to;
    };
    Ok(SyncExit {
        release,
        link: *link,
        target,
        kind,
        routing,
        before: Vec::new(),
        after: Vec::new(),
    })
}
fn append_at(
    storage: &StorageMethod<'_>,
    out: &mut Vec<PlanId>,
    location: impl Fn(CopyLocation) -> bool,
) -> Result<(), RegionError> {
    for plan in storage.facts().copy_plans() {
        if location(plan.location())
            && storage.facts().copy_emission(plan.id()) == Some(crate::p5::CopyEmission::Original)
        {
            push(out, plan.id())?;
        }
    }
    Ok(())
}
fn append_link(f: &RegionFacts, out: &mut Vec<PlanId>, link: LinkId) -> Result<(), RegionError> {
    for &copy in &f.link_plans[link.0 as usize].copies {
        push(out, copy)?;
    }
    Ok(())
}
fn obligations(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    exit: &mut SyncExit,
) -> Result<(), RegionError> {
    append_at(
        storage,
        &mut exit.before,
        |l| matches!(l,CopyLocation::Transfer{site:TransferSite::BeforeOperation(op),..}if op==exit.release),
    )?;
    append_at(storage, &mut exit.before, |l| {
        l == CopyLocation::BeforeThrow(ThrowSite::Operation(exit.release))
    })?;
    append_at(
        storage,
        &mut exit.after,
        |l| matches!(l,CopyLocation::Transfer{site:TransferSite::AfterCommit(op),..}if op==exit.release),
    )?;
    append_link(f, &mut exit.after, exit.link)?;
    for &node in &exit.routing {
        let part = &f.nodes[node.0 as usize];
        if part.terminal {
            append_at(
                storage,
                &mut exit.after,
                |l| matches!(l,CopyLocation::Transfer{site:TransferSite::BeforeTerminal(b),..}if b==part.block),
            )?;
            append_at(storage, &mut exit.after, |l| {
                l == CopyLocation::BeforeThrow(ThrowSite::Terminal(part.block))
            })?;
        }
        for &link in &g.outgoing[node.0 as usize] {
            append_link(f, &mut exit.after, link)?;
        }
    }
    Ok(())
}
fn resume(kind: SyncExitKind) -> Option<NodeId> {
    match kind {
        SyncExitKind::Follow(n)
        | SyncExitKind::Loop { target: n, .. }
        | SyncExitKind::ReturnVoid { terminal: n } => Some(n),
        SyncExitKind::Rethrow { .. } => None,
    }
}
fn select(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    exits: &mut [SyncExit],
) -> Result<(Option<NodeId>, Option<LockSeparation>), RegionError> {
    let mut common = None;
    let mut same = true;
    for exit in exits.iter() {
        if let Some(target) = resume(exit.kind) {
            if common.is_some_and(|old| old != target) {
                same = false;
            } else {
                common = Some(target);
            }
        }
    }
    if same && let Some(target) = common {
        for exit in exits.iter_mut() {
            if resume(exit.kind).is_some() {
                exit.kind = SyncExitKind::Follow(target);
                exit.routing.retain(|n| *n != target);
            }
        }
    }
    let mut follow = None;
    let mut separation = None;
    let mut fallthroughs = 0usize;
    for exit in exits.iter_mut() {
        obligations(storage, f, g, exit)?;
        if let SyncExitKind::Follow(target) = exit.kind {
            fallthroughs += 1;
            if follow.is_some_and(|old| old != target) {
                separation = Some(LockSeparation::IncompatibleContinuation {
                    release: exit.release,
                    target,
                });
            } else {
                follow = Some(target);
            }
        }
    }
    for exit in exits {
        if matches!(exit.kind, SyncExitKind::Follow(_)) && fallthroughs == 1 {
            continue;
        }
        for &plan in &exit.after {
            let p = &storage.facts().copy_plans()[plan.0 as usize];
            if !p.is_complete() || !p.steps().is_empty() {
                separation = Some(LockSeparation::PostReleaseCopy(plan));
                break;
            }
        }
    }
    Ok((follow, separation))
}
pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    positions: LockPositions<'_>,
) -> Result<(), RegionError> {
    let mut events = filled(f.nodes.len(), None)?;
    for (index, event) in f.locks.iter().enumerate() {
        events[g.op_node[event.operation.0 as usize].0 as usize] = Some(index);
    }
    for op in storage.execution().operations() {
        if !matches!(op.kind(), OpKind::Monitor { enter: true }) {
            continue;
        }
        let enter = op.id();
        let entry = g.op_node[enter.0 as usize];
        let event = &f.locks[events[entry.0 as usize].ok_or(bug("acquire has no lock event"))?];
        let early = match event.normal {
            LockState::Unknown(gap) => Some(LockSeparation::State(gap)),
            LockState::Unreachable => Some(LockSeparation::Unreachable),
            LockState::Known(_) => None,
        };
        let lock = event.receiver;
        if let Some(reason) = early.or_else(|| lock.err().map(LockSeparation::Read)) {
            push(
                &mut f.monitors,
                MonitorPlan {
                    enter,
                    mode: MonitorMode::Explicit(reason),
                },
            )?;
            continue;
        }
        let lock = lock.map_err(|_| bug("available acquire lost receiver"))?;
        let [enter_link] = g.outgoing[entry.0 as usize].as_slice() else {
            return Err(bug("acquire has no unique normal continuation"));
        };
        let mut members = filled(f.nodes.len(), false)?;
        let mut held = filled(f.nodes.len(), false)?;
        let mut releases = Vec::new();
        let mut pending = Vec::new();
        members[entry.0 as usize] = true;
        push(&mut pending, f.links[enter_link.0 as usize].to)?;
        let mut failure = None;
        while let Some(node) = pending.pop() {
            if node == entry {
                failure = Some(LockSeparation::ReenteredAcquire);
                break;
            }
            if members[node.0 as usize] {
                continue;
            }
            if let Err(reason) = positions.held(node, enter) {
                failure = Some(reason);
                break;
            }
            members[node.0 as usize] = true;
            held[node.0 as usize] = true;
            if let Some(release) = events[node.0 as usize]
                .map(|i| &f.locks[i])
                .filter(|e| e.matched == Some(enter) && e.release_cannot_throw)
                .map(|e| e.operation)
            {
                push(&mut releases, release)?;
                continue;
            }
            if g.outgoing[node.0 as usize].is_empty() && g.exceptions[node.0 as usize].is_empty() {
                failure = Some(LockSeparation::AbruptExit(node));
                break;
            }
            for &link in &g.outgoing[node.0 as usize] {
                push(&mut pending, f.links[link.0 as usize].to)?;
            }
            for &edge in &g.exceptions[node.0 as usize] {
                if matches!(
                    f.edges[edge.0 as usize].route,
                    EdgeRoute::Exception {
                        delivery: ExceptionDelivery::ImpossibleRelease(_),
                        ..
                    }
                ) {
                    continue;
                }
                if let Some(target) = Graph::exception_target(f, storage, edge) {
                    push(&mut pending, target)?;
                } else {
                    let EdgeRoute::Exception { site, .. } = f.edges[edge.0 as usize].route else {
                        return Err(bug("lock exception lost site"));
                    };
                    failure = Some(LockSeparation::EscapingException(site));
                    break;
                }
            }
            if failure.is_some() {
                break;
            }
        }
        if let Some(reason) = failure {
            push(
                &mut f.monitors,
                MonitorPlan {
                    enter,
                    mode: MonitorMode::Explicit(reason),
                },
            )?;
            continue;
        }
        let mut exits = Vec::new();
        for release in releases {
            push(&mut exits, route(storage, f, g, &held, entry, release)?)?;
        }
        let (follow, mut separation) = select(storage, f, g, &mut exits)?;
        for exit in &exits {
            for &node in &exit.routing {
                members[node.0 as usize] = true;
            }
        }
        if !protection::closed_entry(storage, f, g, &members, entry) {
            separation = Some(LockSeparation::ExternalEntry);
        }
        let all = nodes(&members)?;
        members[entry.0 as usize] = false;
        let body = nodes(&members)?;
        let scope = SyncId(id(f.synchronized.len())?);
        push(
            &mut f.synchronized,
            SyncPlan {
                id: scope,
                enter,
                enter_link: *enter_link,
                lock,
                held: nodes(&held)?,
                body,
                exits,
                members: all,
                follow,
                separation,
            },
        )?;
        push(
            &mut f.monitors,
            MonitorPlan {
                enter,
                mode: separation.map_or(MonitorMode::Natural(scope), MonitorMode::Explicit),
            },
        )?;
    }
    Ok(())
}
