//! Plans a switch's private case parts. This happens before transfer planning so
//! a common-follow exit leaves the switch, not an artificial labelled wrapper
//! around a multi-entry graph. Exact disjoint arm interfaces are selected before
//! constructing their nested owners; shared continuation work stays outside.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::*;
use super::parts::*;
use crate::p5::StorageMethod;

pub(super) fn private_switch_parts(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    c: &Candidate,
    boundary: &[bool],
) -> Result<Option<Vec<Part>>, RegionError> {
    let RegionKind::Switch { arms, .. } = &c.kind else {
        return Ok(None);
    };
    if arms.len() < 2 {
        return Ok(None);
    }
    let body = &c.parts[1];
    let set = mask(f.nodes.len(), &body.nodes)?;
    let mut starts = Vec::new();
    let mut partition = filled(f.nodes.len(), false)?;
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
    for &link in arms {
        let entry = f.links[link.0 as usize].to;
        if starts.contains(&entry) {
            return Ok(None);
        }
        push(&mut starts, entry)?;
        let arm = super::completion::reachable(f, g, &[entry], boundary, c.follow)?;
        let nodes = list(&arm)?;
        let interface = super::completion::CompletionInterface::collect(storage, f, g, &nodes)?;
        if interface.entrances.iter().any(|id| {
            let incoming = f.links[id.0 as usize];
            incoming.from != c.entry || incoming.to != entry
        }) || !interface.exceptional_entrances.is_empty()
            || interface.outlets.iter().any(|id| {
                let to = f.links[id.0 as usize].to;
                Some(to) != c.follow && !boundary[to.0 as usize]
            })
        {
            return Ok(None);
        }
        for &node in &interface.nodes {
            if !set[node.0 as usize] || partition[node.0 as usize] {
                return Ok(None);
            }
            partition[node.0 as usize] = true;
            if g.outgoing[node.0 as usize].is_empty() && !interface.abrupt.contains(&node) {
                return Ok(None);
            }
        }
        if !normal_arm_acyclic(f, g, &arm)? {
            return Ok(None);
        }
        push(
            &mut parts,
            Part {
                entry: (!nodes.is_empty()).then_some(entry),
                nodes,
                follow: c.follow,
                completion: PartCompletion::Ordinary,
            },
        )?;
    }
    if body.nodes.iter().any(|n| !partition[n.0 as usize]) {
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
