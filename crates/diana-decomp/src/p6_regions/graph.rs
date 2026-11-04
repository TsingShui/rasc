use super::flow::{Flow, Transfer};
use crate::p2_flow::BlockId;
use crate::substrate::owned::reserve;
use crate::{RequestError, RunContext};
pub(super) fn index(id: BlockId) -> Result<usize, RequestError> {
    usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
pub(super) struct Graph {
    pub successors: Vec<Vec<BlockId>>,
    pub predecessors: Vec<Vec<BlockId>>,
    pub returned: BlockId,
}
pub(super) fn graph(
    body: &Flow,
    budget: &mut RunContext<'_>,
) -> Result<Option<Graph>, RequestError> {
    let n = body.cases.len();
    if n == 0 {
        return Ok(None);
    }
    let mut successors = Vec::new();
    let mut predecessors = Vec::new();
    reserve(&mut successors, n, budget)?;
    reserve(&mut predecessors, n, budget)?;
    for _ in 0..n {
        budget.checkpoint()?;
        successors.push(Vec::new());
        predecessors.push(Vec::new());
    }
    let mut returned = None;
    for (at, case) in body.cases.iter().enumerate() {
        budget.checkpoint()?;
        if index(case.key)? != at {
            return Ok(None);
        }
        match &case.transfer {
            Transfer::Jump(edge) => push(&mut successors[at], edge.target, budget)?,
            Transfer::Branch { yes, no, .. } => {
                push(&mut successors[at], yes.target, budget)?;
                push(&mut successors[at], no.target, budget)?;
            }
            Transfer::Return => {
                if returned.is_some() {
                    return Ok(None);
                }
                returned = Some(case.key);
            }
            Transfer::Switch => return Ok(None),
        }
    }
    let Some(returned) = returned else {
        return Ok(None);
    };
    for (at, edges) in successors.iter().enumerate() {
        let source = BlockId(u32::try_from(at).map_err(|_| RequestError::PlatformCapacity)?);
        for &target in edges {
            budget.checkpoint()?;
            let Some(pred) = predecessors.get_mut(index(target)?) else {
                return Err(RequestError::InternalInvariant(
                    "region graph has missing target",
                ));
            };
            push(pred, source, budget)?;
        }
    }
    Ok(Some(Graph {
        successors,
        predecessors,
        returned,
    }))
}
fn rpo(
    successors: &[Vec<BlockId>],
    root: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<BlockId>>, RequestError> {
    let mut seen = Vec::new();
    reserve(&mut seen, successors.len(), budget)?;
    for _ in successors {
        budget.checkpoint()?;
        seen.push(false);
    }
    let mut stack = Vec::new();
    let mut order = Vec::new();
    seen[index(root)?] = true;
    push(&mut stack, (root, 0usize), budget)?;
    while let Some((block, cursor)) = stack.pop() {
        budget.checkpoint()?;
        if let Some(&next) = successors[index(block)?].get(cursor) {
            push(
                &mut stack,
                (
                    block,
                    cursor
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?,
                ),
                budget,
            )?;
            if !seen[index(next)?] {
                seen[index(next)?] = true;
                push(&mut stack, (next, 0), budget)?;
            }
        } else {
            push(&mut order, block, budget)?;
        }
    }
    if order.len() != successors.len() {
        return Ok(None);
    }
    budget.checkpoint()?;
    order.reverse();
    Ok(Some(order))
}
pub(super) struct Dominators {
    parents: Vec<Option<BlockId>>,
}
fn intersect(
    mut a: BlockId,
    mut b: BlockId,
    parents: &[Option<BlockId>],
    positions: &[usize],
    budget: &mut RunContext<'_>,
) -> Result<BlockId, RequestError> {
    while a != b {
        budget.checkpoint()?;
        let (current, other) = if positions[index(a)?] > positions[index(b)?] {
            (&mut a, b)
        } else {
            (&mut b, a)
        };
        let at = index(*current)?;
        let parent = parents[at].ok_or(RequestError::InternalInvariant(
            "dominator intersection lost parent",
        ))?;
        if positions[index(parent)?] >= positions[at]
            || positions[index(*current)?] == positions[index(other)?]
        {
            return Err(RequestError::InternalInvariant(
                "dominator intersection made no progress",
            ));
        }
        *current = parent;
    }
    Ok(a)
}
pub(super) fn dominators(
    successors: &[Vec<BlockId>],
    predecessors: &[Vec<BlockId>],
    root: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<Option<Dominators>, RequestError> {
    let Some(order) = rpo(successors, root, budget)? else {
        return Ok(None);
    };
    let n = order.len();
    let mut positions = Vec::new();
    let mut parents = Vec::new();
    reserve(&mut positions, n, budget)?;
    reserve(&mut parents, n, budget)?;
    for _ in 0..n {
        budget.checkpoint()?;
        positions.push(0);
        parents.push(None);
    }
    for (position, &node) in order.iter().enumerate() {
        budget.checkpoint()?;
        positions[index(node)?] = position;
    }
    parents[index(root)?] = Some(root);
    let mut queued = Vec::new();
    let mut work = Vec::new();
    reserve(&mut queued, n, budget)?;
    reserve(&mut work, n, budget)?;
    for _ in 0..n {
        budget.checkpoint()?;
        queued.push(false);
    }
    for &node in order.iter().skip(1).rev() {
        budget.checkpoint()?;
        queued[index(node)?] = true;
        work.push(node);
    }
    while let Some(node) = work.pop() {
        budget.checkpoint()?;
        queued[index(node)?] = false;
        let mut candidate = None;
        for &predecessor in &predecessors[index(node)?] {
            budget.checkpoint()?;
            if parents[index(predecessor)?].is_some() {
                candidate = Some(match candidate {
                    None => predecessor,
                    Some(previous) => {
                        intersect(previous, predecessor, &parents, &positions, budget)?
                    }
                });
            }
        }
        if candidate != parents[index(node)?] {
            parents[index(node)?] = candidate;
            for &successor in &successors[index(node)?] {
                budget.checkpoint()?;
                if successor != root && !queued[index(successor)?] {
                    push(&mut work, successor, budget)?;
                    queued[index(successor)?] = true;
                }
            }
        }
    }
    Ok(Some(Dominators { parents }))
}
impl Dominators {
    pub(super) fn dominates(
        &self,
        ancestor: BlockId,
        mut node: BlockId,
        budget: &mut RunContext<'_>,
    ) -> Result<bool, RequestError> {
        loop {
            budget.checkpoint()?;
            if node == ancestor {
                return Ok(true);
            }
            let Some(parent) = self.parents[index(node)?] else {
                return Ok(false);
            };
            if parent == node {
                return Ok(false);
            }
            node = parent;
        }
    }
}
