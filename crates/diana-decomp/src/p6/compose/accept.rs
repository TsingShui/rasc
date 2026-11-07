//! Decides whether one proposal may become a region. Acceptance is the P6
//! contract: entry closure, construction-group atomicity, no split owner,
//! lock-scope boundary, loop-plan laminarity and the successor-boundary
//! acyclicity of a partial arm. A rejected proposal keeps its reason and the
//! nodes keep their previous owner.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::protection;
use super::super::*;
use super::parts::*;
use crate::p5::StorageMethod;

pub(super) fn accept(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    owners: &[RegionId],
    c: &Candidate,
) -> Result<Result<(), RecoveryReason>, RegionError> {
    let set = mask(f.nodes.len(), &c.members)?;
    if f.synchronized.iter().any(|scope| {
        scope.separation.is_none()
            && scope.body.contains(&c.entry)
            && c.members.iter().any(|n| !scope.body.contains(n))
    }) {
        return Ok(Err(RecoveryReason::LockBoundary));
    }
    if !protection::closed_entry(storage, f, g, &set, c.entry) {
        return Ok(Err(RecoveryReason::MultipleEntry));
    }
    for group in storage.facts().construction_groups() {
        let inside = group
            .members()
            .iter()
            .filter(|op| set[g.op_node[op.0 as usize].0 as usize])
            .count();
        if inside != 0 && inside != group.members().len() {
            return Ok(Err(RecoveryReason::ConstructionSplit));
        }
    }
    let mut partition = filled(f.nodes.len(), usize::MAX)?;
    for (i, p) in c.parts.iter().enumerate() {
        for &n in &p.nodes {
            if !set[n.0 as usize] || partition[n.0 as usize] != usize::MAX {
                return Err(bug("region candidate has duplicate or foreign content"));
            }
            partition[n.0 as usize] = i;
        }
    }
    for &n in &c.members {
        let part = partition[n.0 as usize];
        if part == usize::MAX {
            return Err(bug("region candidate omitted execution content"));
        }
        let root = owners[n.0 as usize];
        if f.regions[root.0 as usize]
            .nodes
            .iter()
            .any(|n| partition[n.0 as usize] != part)
        {
            return Ok(Err(RecoveryReason::Overlap));
        }
    }
    Ok(Ok(()))
}
pub(super) fn match_branch_loops(
    f: &RegionFacts,
    c: &Candidate,
) -> Result<Result<(), RecoveryReason>, RegionError> {
    if !matches!(c.kind, RegionKind::If { .. } | RegionKind::Condition(_)) {
        return Ok(Ok(()));
    }
    let set = mask(f.nodes.len(), &c.members)?;
    for plan in &f.loops {
        let inside = plan
            .lexical_members
            .iter()
            .filter(|n| set[n.0 as usize])
            .count();
        let owns_loop = inside == plan.lexical_members.len();
        // All established plans constrain branches, before any loop is selected.
        if inside != 0 && !owns_loop && inside != c.members.len() {
            return Ok(Err(RecoveryReason::Overlap));
        }
        let owns_private_exit = c.follow == Some(plan.continue_target())
            && plan.private_exits.iter().any(|exit| {
                let link = f.links[exit.0 as usize];
                set[link.from.0 as usize] && set[link.to.0 as usize]
            });
        if !owns_loop
            && (set[plan.continue_target().0 as usize]
                || (!owns_private_exit
                    && plan
                        .latches
                        .iter()
                        .any(|l| set[f.links[l.0 as usize].from.0 as usize])))
        {
            return Ok(Err(RecoveryReason::Overlap));
        }
    }
    Ok(Ok(()))
}
pub(super) fn successor_arm_acyclic(
    f: &RegionFacts,
    owners: &[RegionId],
    arm: &[bool],
) -> Result<bool, RegionError> {
    let mut representative = Vec::new();
    for n in 0..f.nodes.len() {
        push(&mut representative, NodeId(id(n)?))?;
    }
    let mut contracted = filled(f.nodes.len(), false)?;
    let mut seen = filled(f.regions.len(), false)?;
    let mut stack = Vec::new();
    for (n, &inside) in arm.iter().enumerate() {
        if inside {
            let owner = owners[n];
            if !seen[owner.0 as usize] {
                seen[owner.0 as usize] = true;
                push(&mut stack, owner)?;
            }
        }
    }
    while let Some(id) = stack.pop() {
        let region = &f.regions[id.0 as usize];
        if matches!(region.kind, RegionKind::Loop(_))
            && region.nodes.iter().all(|n| arm[n.0 as usize])
        {
            let entry = region.entry.ok_or(bug("selected loop has no entry"))?;
            for &n in &region.nodes {
                representative[n.0 as usize] = entry;
                contracted[n.0 as usize] = true;
            }
        } else {
            for &child in &region.children {
                push(&mut stack, child)?;
            }
        }
    }
    let mut outgoing = lists(f.nodes.len())?;
    let mut incoming = filled(f.nodes.len(), 0usize)?;
    for link in &f.links {
        if !arm[link.from.0 as usize] || !arm[link.to.0 as usize] {
            continue;
        }
        let from = representative[link.from.0 as usize];
        let to = representative[link.to.0 as usize];
        if from == to && contracted[link.from.0 as usize] {
            continue;
        }
        push(&mut outgoing[from.0 as usize], to)?;
        incoming[to.0 as usize] = incoming[to.0 as usize]
            .checked_add(1)
            .ok_or(crate::RequestError::PlatformCapacity)?;
    }
    let mut queue = Vec::new();
    let mut remaining = 0usize;
    for (n, &inside) in arm.iter().enumerate() {
        if inside && representative[n].0 as usize == n {
            remaining += 1;
            if incoming[n] == 0 {
                push(&mut queue, NodeId(id(n)?))?;
            }
        }
    }
    while let Some(n) = queue.pop() {
        remaining -= 1;
        for &to in &outgoing[n.0 as usize] {
            incoming[to.0 as usize] -= 1;
            if incoming[to.0 as usize] == 0 {
                push(&mut queue, to)?;
            }
        }
    }
    Ok(remaining == 0)
}
pub(super) fn match_successor_boundary(
    f: &RegionFacts,
    owners: &[RegionId],
    c: &Candidate,
) -> Result<Result<(), RecoveryReason>, RegionError> {
    if c.successor_boundary.is_none() {
        return Ok(Ok(()));
    }
    let follow = c.follow.ok_or(bug("successor boundary has no follow"))?;
    let mut arm = mask(f.nodes.len(), &c.members)?;
    if arm[follow.0 as usize] || c.parts[1..].iter().any(|p| p.nodes.contains(&c.entry)) {
        return Ok(Err(RecoveryReason::Unstructured));
    }
    if arm[0] && c.entry != NodeId(0) {
        return Ok(Err(RecoveryReason::MultipleEntry));
    }
    for link in &f.links {
        if arm[link.from.0 as usize] && !arm[link.to.0 as usize] && link.to != follow {
            return Ok(Err(RecoveryReason::Unstructured));
        }
    }
    arm[c.entry.0 as usize] = false;
    if !successor_arm_acyclic(f, owners, &arm)? {
        return Ok(Err(RecoveryReason::Unstructured));
    }
    Ok(Ok(()))
}
