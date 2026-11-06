//! The vocabulary shared by region proposals: a candidate, its ordered parts and
//! the exported entries a part hands to its successors. A part is not a region
//! yet; it names original nodes whose already-selected owners a region will own.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::*;
use crate::p5::StorageMethod;

pub(super) fn copy<T: Copy>(source: &[T]) -> Result<Vec<T>, RegionError> {
    let mut out = Vec::new();
    crate::substrate::owned::reserve(&mut out, source.len())?;
    out.extend_from_slice(source);
    Ok(out)
}
pub(super) fn mask(n: usize, nodes: &[NodeId]) -> Result<Vec<bool>, RegionError> {
    let mut out = filled(n, false)?;
    for p in nodes {
        out[p.0 as usize] = true;
    }
    Ok(out)
}
pub(super) fn list(set: &[bool]) -> Result<Vec<NodeId>, RegionError> {
    let mut out = Vec::new();
    for (i, &inside) in set.iter().enumerate() {
        if inside {
            push(&mut out, NodeId(id(i)?))?;
        }
    }
    Ok(out)
}
#[derive(Clone, Copy)]
pub(super) enum PartCompletion {
    Ordinary,
    LoopBody(LoopId),
}
pub(super) struct Part {
    pub(super) nodes: Vec<NodeId>,
    pub(super) entry: Option<NodeId>,
    pub(super) follow: Option<NodeId>,
    pub(super) completion: PartCompletion,
}
#[derive(Clone, Copy)]
pub(super) enum SuccessorBoundary {
    True(LinkId),
    False(LinkId),
}
pub(super) struct Candidate {
    pub(super) kind: RegionKind,
    pub(super) entry: NodeId,
    pub(super) follow: Option<NodeId>,
    pub(super) members: Vec<NodeId>,
    pub(super) parts: Vec<Part>,
    pub(super) successor_boundary: Option<SuccessorBoundary>,
}
pub(super) fn add(
    f: &mut RegionFacts,
    kind: RegionKind,
    entry: Option<NodeId>,
    follow: Option<NodeId>,
    children: Vec<RegionId>,
    nodes: Vec<NodeId>,
) -> Result<RegionId, RegionError> {
    let region = RegionId(id(f.regions.len())?);
    for &child in &children {
        if f.parents[child.0 as usize].is_some() {
            return Err(bug("region child has two execution owners"));
        }
    }
    push(&mut f.parents, None)?;
    for &child in &children {
        f.parents[child.0 as usize] = Some(region);
    }
    push(
        &mut f.regions,
        Region {
            id: region,
            kind,
            entry,
            follow,
            children,
            nodes,
            continuation: Continuation::None,
        },
    )?;
    Ok(region)
}
pub(super) fn entries(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    set: &[bool],
    entry: Option<NodeId>,
) -> Result<Vec<NodeId>, RegionError> {
    let mut out = Vec::new();
    if let Some(p) = entry {
        push(&mut out, p)?;
    }
    if set[0] && !out.contains(&NodeId(0)) {
        push(&mut out, NodeId(0))?;
    }
    for l in &f.links {
        if set[l.to.0 as usize] && !set[l.from.0 as usize] && !out.contains(&l.to) {
            push(&mut out, l.to)?;
        }
    }
    for (from, row) in g.exceptions.iter().enumerate() {
        if set[from] {
            continue;
        }
        for &e in row {
            if let Some(to) = Graph::exception_target(f, storage, e)
                && set[to.0 as usize]
                && !out.contains(&to)
            {
                push(&mut out, to)?;
            }
        }
    }
    Ok(out)
}
pub(in crate::p6) fn graph_kind(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    members: &[NodeId],
    entry: Option<NodeId>,
    reason: RecoveryReason,
) -> Result<RegionKind, RegionError> {
    let set = mask(f.nodes.len(), members)?;
    let entries = entries(storage, f, g, &set, entry)?;
    let mut exits = Vec::new();
    for l in &f.links {
        if set[l.from.0 as usize] && !set[l.to.0 as usize] {
            push(&mut exits, l.id)?;
        }
    }
    Ok(RegionKind::Graph {
        reason,
        entries,
        exits,
    })
}
