//! Plans a switch's private case parts. This happens before transfer planning so
//! a common-follow exit leaves the switch, not an artificial labelled wrapper
//! around a multi-entry graph. Selected nested owners remain whole, so their
//! scopes and exception ownership are unchanged.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::protection;
use super::super::*;
use super::parts::*;
use crate::p2::Terminal;
use crate::p5::StorageMethod;

pub(super) fn private_switch_parts(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    owners: &[RegionId],
    c: &Candidate,
) -> Result<Option<Vec<Part>>, RegionError> {
    let RegionKind::Switch { arms, .. } = &c.kind else {
        return Ok(None);
    };
    if arms.len() < 2 {
        return Ok(None);
    }
    let body = &c.parts[1];
    let set = mask(f.nodes.len(), &body.nodes)?;
    let exported = entries(storage, f, g, &set, body.entry)?;
    let mut starts = Vec::new();
    for &arm in arms {
        let target = f.links[arm.0 as usize].to;
        if !set[target.0 as usize] || starts.contains(&target) {
            return Ok(None);
        }
        push(&mut starts, target)?;
    }
    if exported.len() != starts.len() || exported.iter().any(|n| !starts.contains(n)) {
        return Ok(None);
    }
    let mut partition = filled(f.nodes.len(), None)?;
    let empty = filled(f.nodes.len(), false)?;
    let mut parts = Vec::new();
    push(
        &mut parts,
        Part {
            nodes: copy(&[c.entry])?,
            entry: Some(c.entry),
            follow: None,
            completion: PartCompletion::Ordinary,
        },
    )?;
    for (index, &entry) in starts.iter().enumerate() {
        let arm = protection::reachable(storage, f, g, &[entry], c.follow, &empty, false)?;
        let nodes = list(&arm)?;
        let mut roots = Vec::new();
        for &node in &nodes {
            if !set[node.0 as usize] || partition[node.0 as usize].replace(index).is_some() {
                return Ok(None);
            }
            // Every normal entrance is either private or this selector's
            // original arm. Even an outside arrival at the case label rejects.
            for &link in &g.incoming[node.0 as usize] {
                let link = f.links[link.0 as usize];
                if !arm[link.from.0 as usize] && !(link.from == c.entry && node == entry) {
                    return Ok(None);
                }
            }
            let root = owners[node.0 as usize];
            if !roots.contains(&root) {
                push(&mut roots, root)?;
            }
            if f.regions[root.0 as usize]
                .nodes
                .iter()
                .any(|n| !arm[n.0 as usize])
            {
                return Ok(None);
            }
            for &link in &g.outgoing[node.0 as usize] {
                let target = f.links[link.0 as usize].to;
                if !arm[target.0 as usize] && Some(target) != c.follow {
                    return Ok(None);
                }
            }
            if g.outgoing[node.0 as usize].is_empty()
                && (!f.nodes[node.0 as usize].terminal
                    || !matches!(
                        storage.execution().blocks()[f.nodes[node.0 as usize].block.0 as usize]
                            .terminal(),
                        Some(Terminal::Return { .. } | Terminal::Throw { .. })
                    ))
            {
                return Ok(None);
            }
        }
        for (from, edges) in g.exceptions.iter().enumerate() {
            if arm[from] {
                continue;
            }
            for &edge in edges {
                if Graph::exception_possible(f, edge)
                    && Graph::exception_target(f, storage, edge).is_some_and(|n| arm[n.0 as usize])
                {
                    return Ok(None);
                }
            }
        }
        // Do not create a new graph/labelled scope within an arm. Only a
        // single chain of existing owners is admitted; no shared tail or
        // cross-arm fallthrough is moved or duplicated.
        let mut order = Vec::new();
        let mut at = Some(owners[entry.0 as usize]);
        while let Some(root) = at {
            if order.contains(&root) {
                return Ok(None);
            }
            push(&mut order, root)?;
            let mut next = None;
            for &node in &f.regions[root.0 as usize].nodes {
                for &link in &g.outgoing[node.0 as usize] {
                    let target = f.links[link.0 as usize].to;
                    if arm[target.0 as usize] && owners[target.0 as usize] != root {
                        let target = owners[target.0 as usize];
                        if next.is_some_and(|old| old != target) {
                            return Ok(None);
                        }
                        next = Some(target);
                    }
                }
            }
            at = next;
        }
        if order.len() != roots.len() {
            return Ok(None);
        }
        // A private continuation must terminate, not merely have a finite
        // postdominator candidate on one path. Reject cycles conservatively.
        if !normal_arm_acyclic(f, g, &arm)? {
            return Ok(None);
        }
        push(
            &mut parts,
            Part {
                nodes,
                entry: Some(entry),
                follow: c.follow,
                completion: PartCompletion::Ordinary,
            },
        )?;
    }
    if body.nodes.iter().any(|n| partition[n.0 as usize].is_none()) {
        return Ok(None);
    }
    Ok(Some(parts))
}

fn normal_arm_acyclic(f: &RegionFacts, g: &Graph, arm: &[bool]) -> Result<bool, RegionError> {
    let mut incoming = filled(f.nodes.len(), 0usize)?;
    let mut queue = Vec::new();
    let mut remaining = 0usize;
    for (node, &inside) in arm.iter().enumerate() {
        if inside {
            remaining += 1;
            incoming[node] = g.incoming[node]
                .iter()
                .filter(|l| arm[f.links[l.0 as usize].from.0 as usize])
                .count();
            if incoming[node] == 0 {
                push(&mut queue, NodeId(id(node)?))?;
            }
        }
    }
    while let Some(node) = queue.pop() {
        remaining -= 1;
        for &link in &g.outgoing[node.0 as usize] {
            let target = f.links[link.0 as usize].to;
            if arm[target.0 as usize] {
                incoming[target.0 as usize] -= 1;
                if incoming[target.0 as usize] == 0 {
                    push(&mut queue, target)?;
                }
            }
        }
    }
    Ok(remaining == 0)
}
