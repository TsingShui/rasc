//! Turns one part into a region: give its nodes a single owner. A part whose
//! nodes already have one contiguous owner keeps that owner; anything else
//! becomes a sequence when the owners chain linearly and an explicit graph
//! otherwise. Only a graph keeps a labelled block, because only a graph needs a
//! lexical target for a transfer that leaves it.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::*;
use super::parts::*;
use crate::p5::StorageMethod;

/// A printed order for the owners of one part: every original edge between two
/// owners points forward. Ties break on the owner's first node, so the order
/// follows the original block layout. `None` when an owner cycle remains.
fn topological(
    f: &RegionFacts,
    g: &Graph,
    set: &[bool],
    owners: &[RegionId],
    roots: &[RegionId],
    entry: Option<NodeId>,
    loop_continue: Option<NodeId>,
) -> Result<Option<Vec<RegionId>>, RegionError> {
    let mut index = vec![usize::MAX; f.regions.len()];
    for (slot, &root) in roots.iter().enumerate() {
        index[root.0 as usize] = slot;
    }
    let mut outgoing = vec![Vec::new(); roots.len()];
    let mut incoming = vec![0usize; roots.len()];
    for &root in roots {
        let from = index[root.0 as usize];
        for &node in &f.regions[root.0 as usize].nodes {
            for &link in &g.outgoing[node.0 as usize] {
                let target = f.links[link.0 as usize].to;
                if Some(target) == loop_continue || !set[target.0 as usize] {
                    continue;
                }
                let to = index[owners[target.0 as usize].0 as usize];
                if to == from || outgoing[from].contains(&to) {
                    continue;
                }
                push(&mut outgoing[from], to)?;
                incoming[to] += 1;
            }
        }
    }
    // The part's entry owner prints first; every other owner waits for its
    // original predecessors.
    let mut ready = Vec::new();
    let start = entry.map(|node| index[owners[node.0 as usize].0 as usize]);
    for (slot, &pending) in incoming.iter().enumerate() {
        if pending == 0 {
            push(&mut ready, slot)?;
        }
    }
    let mut order = Vec::new();
    while !ready.is_empty() {
        let pick = match start {
            Some(wanted) if order.is_empty() && ready.contains(&wanted) => wanted,
            _ => {
                let mut best = 0;
                for (i, &slot) in ready.iter().enumerate() {
                    let first = f.regions[roots[slot].0 as usize].nodes[0];
                    let current = f.regions[roots[ready[best]].0 as usize].nodes[0];
                    if first < current {
                        best = i;
                    }
                }
                ready[best]
            }
        };
        ready.retain(|&slot| slot != pick);
        push(&mut order, roots[pick])?;
        for to in std::mem::take(&mut outgoing[pick]) {
            incoming[to] -= 1;
            if incoming[to] == 0 {
                push(&mut ready, to)?;
            }
        }
    }
    Ok((order.len() == roots.len()).then_some(order))
}

pub(super) fn compose(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    owners: &mut [RegionId],
    part: Part,
) -> Result<RegionId, RegionError> {
    let loop_continue = match part.completion {
        PartCompletion::Ordinary => None,
        PartCompletion::LoopBody(id) => {
            let target = f.loops[id.0 as usize].continue_target();
            if part.follow != Some(target) {
                return Err(bug("loop body lost its continue boundary"));
            }
            Some(target)
        }
    };
    let set = mask(f.nodes.len(), &part.nodes)?;
    let mut roots = Vec::new();
    for &p in &part.nodes {
        let r = owners[p.0 as usize];
        if !roots.contains(&r) {
            push(&mut roots, r)?;
        }
    }
    if roots.len() == 1 {
        return Ok(roots[0]);
    }
    let exported = entries(storage, f, g, &set, part.entry)?;
    let mut order = Vec::new();
    let mut at = part.entry.map(|p| owners[p.0 as usize]);
    let mut linear = exported.len() <= 1;
    while let Some(r) = at {
        if order.contains(&r) {
            linear = false;
            break;
        }
        push(&mut order, r)?;
        let mut next = None;
        for &p in &f.regions[r.0 as usize].nodes {
            for &l in &g.outgoing[p.0 as usize] {
                let q = f.links[l.0 as usize].to;
                // The accepted loop owns this body-completion link, not a
                // successor sibling. Its original transfer and copies survive.
                if Some(q) == loop_continue {
                    continue;
                }
                if set[q.0 as usize] && owners[q.0 as usize] != r {
                    let q = owners[q.0 as usize];
                    if next.is_some_and(|old| old != q) {
                        linear = false;
                    } else {
                        next = Some(q);
                    }
                }
            }
        }
        at = next;
    }
    linear &= order.len() == roots.len();
    // A single-entry part whose owners form a DAG still prints as a sequence: the
    // original transfers say where control goes, so only the printed order needs
    // to respect them. A join reached from two earlier owners is the ordinary
    // case, not a reason for an explicit graph.
    if !linear
        && exported.len() <= 1
        && let Some(sorted) = topological(f, g, &set, owners, &roots, part.entry, loop_continue)?
    {
        let region = add(
            f,
            RegionKind::Sequence,
            part.entry,
            part.follow,
            sorted,
            copy(&part.nodes)?,
        )?;
        for p in part.nodes {
            owners[p.0 as usize] = region;
        }
        return Ok(region);
    }
    let kind = if linear {
        RegionKind::Sequence
    } else {
        order = roots;
        let reason=if f.issues.iter().any(|issue|matches!(issue,PlanningIssue::Graph{entry,reason:RecoveryReason::Irreducible}if set[entry.0 as usize])){RecoveryReason::Irreducible}else if exported.len()>1{RecoveryReason::MultipleEntry}else{RecoveryReason::Unstructured};
        super::trace::graph(
            part.entry,
            &reason,
            part.nodes.len(),
            exported.len(),
            order.len(),
        );
        if let Some(entry) = part.entry {
            push(&mut f.issues, PlanningIssue::Graph { entry, reason })?;
        }
        graph_kind(storage, f, g, &part.nodes, part.entry, reason)?
    };
    let is_graph = matches!(kind, RegionKind::Graph { .. });
    let mut region = add(f, kind, part.entry, part.follow, order, copy(&part.nodes)?)?;
    if is_graph && let Some(target) = part.follow.filter(|n| !set[n.0 as usize]) {
        let mut children = Vec::new();
        push(&mut children, region)?;
        region = add(
            f,
            RegionKind::LabelledBlock { target },
            part.entry,
            part.follow,
            children,
            copy(&part.nodes)?,
        )?;
    }
    for p in part.nodes {
        owners[p.0 as usize] = region;
    }
    Ok(region)
}
