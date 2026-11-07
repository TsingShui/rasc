//! Original-target interfaces used while selecting a region and its children.
//! Storage availability is not a control-edge predicate: links and terminals
//! retain their original copy, operand, completion and site products.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::*;
use super::parts::{copy, mask};
use crate::p2::Terminal;
use crate::p5::StorageMethod;

pub(super) struct CompletionInterface {
    pub nodes: Vec<NodeId>,
    pub entrances: Vec<LinkId>,
    pub exceptional_entrances: Vec<crate::p2::EdgeId>,
    pub outlets: Vec<LinkId>,
    pub abrupt: Vec<NodeId>,
}
impl CompletionInterface {
    pub fn collect(
        storage: &StorageMethod<'_>,
        f: &RegionFacts,
        g: &Graph,
        nodes: &[NodeId],
    ) -> Result<Self, RegionError> {
        let set = mask(f.nodes.len(), nodes)?;
        let mut result = Self {
            nodes: copy(nodes)?,
            entrances: Vec::new(),
            exceptional_entrances: Vec::new(),
            outlets: Vec::new(),
            abrupt: Vec::new(),
        };
        for link in &f.links {
            match (set[link.from.0 as usize], set[link.to.0 as usize]) {
                (false, true) => push(&mut result.entrances, link.id)?,
                (true, false) => push(&mut result.outlets, link.id)?,
                _ => {}
            }
        }
        for (from, edges) in g.exceptions.iter().enumerate() {
            for &edge in edges {
                if !set[from]
                    && Graph::exception_possible(f, edge)
                    && Graph::exception_target(f, storage, edge)
                        .is_some_and(|to| set[to.0 as usize])
                {
                    push(&mut result.exceptional_entrances, edge)?;
                }
            }
        }
        for &node in nodes {
            let source = &f.nodes[node.0 as usize];
            if source.terminal
                && matches!(
                    storage.execution().blocks()[source.block.0 as usize].terminal(),
                    Some(Terminal::Return { .. } | Terminal::Throw { .. })
                )
            {
                push(&mut result.abrupt, node)?;
            }
        }
        Ok(result)
    }

    pub fn boundary(&self, f: &RegionFacts) -> Result<Vec<bool>, RegionError> {
        let mut boundary = filled(f.nodes.len(), false)?;
        for &link in &self.outlets {
            boundary[f.links[link.0 as usize].to.0 as usize] = true;
        }
        Ok(boundary)
    }
}

pub(super) fn reachable(
    f: &RegionFacts,
    g: &Graph,
    starts: &[NodeId],
    boundary: &[bool],
    follow: Option<NodeId>,
) -> Result<Vec<bool>, RegionError> {
    let mut reached = filled(f.nodes.len(), false)?;
    let mut stack = Vec::new();
    for &start in starts {
        push(&mut stack, start)?;
    }
    while let Some(node) = stack.pop() {
        let index = node.0 as usize;
        if reached[index] || boundary[index] || follow == Some(node) {
            continue;
        }
        reached[index] = true;
        for &link in &g.outgoing[index] {
            push(&mut stack, f.links[link.0 as usize].to)?;
        }
    }
    Ok(reached)
}

/// A unique first shared actual target, not a categorical return vertex. More
/// than one shared frontier stays explicit; no returned operands are compared.
pub(super) fn shared_target(
    f: &RegionFacts,
    g: &Graph,
    starts: &[NodeId],
    boundary: &[bool],
) -> Result<Option<NodeId>, RegionError> {
    let mut counts = filled(f.nodes.len(), 0usize)?;
    for &start in starts {
        let reached = reachable(f, g, &[start], boundary, None)?;
        for (index, inside) in reached.into_iter().enumerate() {
            if inside {
                counts[index] = counts[index]
                    .checked_add(1)
                    .ok_or(crate::RequestError::PlatformCapacity)?;
            }
        }
    }
    let mut target = None;
    for index in 0..counts.len() {
        if counts[index] < 2
            || g.incoming[index]
                .iter()
                .any(|link| counts[f.links[link.0 as usize].from.0 as usize] >= 2)
        {
            continue;
        }
        if target.is_some() {
            return Ok(None);
        }
        target = Some(NodeId(id(index)?));
    }
    Ok(target)
}
