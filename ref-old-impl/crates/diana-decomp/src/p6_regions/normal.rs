use super::branch::{BodyBranch, Guard};
use super::flow::{Flow, Transfer};
use super::graph::{dominators, graph, index, push};
use crate::p2_flow::{BlockId, EdgeId};
use crate::substrate::owned::reserve;
use crate::{RequestError, RunContext};
pub(super) struct Candidate {
    pub branches: Vec<BodyBranch>,
    pub prelude: Vec<BlockId>,
    pub header: BlockId,
    pub body: Vec<BlockId>,
    pub tail: Vec<BlockId>,
    pub body_yes: bool,
    pub enter: EdgeId,
    pub leave: EdgeId,
    pub latch: EdgeId,
}
fn exit_stub(
    storage: &Flow,
    target: BlockId,
    exit: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<Option<Option<BlockId>>, RequestError> {
    budget.checkpoint()?;
    if target == exit {
        return Ok(Some(None));
    }
    Ok(match &storage.cases[index(target)?].transfer {
        Transfer::Jump(edge) if edge.target == exit => Some(Some(target)),
        _ => None,
    })
}
fn loop_kernel(
    graph: &super::graph::Graph,
    dom: &super::graph::Dominators,
    header: BlockId,
    body: &[BlockId],
    branches: &[BodyBranch],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(&latch) = body.last() else {
        return Ok(false);
    };
    let mut members = Vec::new();
    reserve(&mut members, graph.predecessors.len(), budget)?;
    for _ in &graph.predecessors {
        budget.checkpoint()?;
        members.push(false);
    }
    members[index(header)?] = true;
    let mut count = 1usize;
    let mut stack = Vec::new();
    push(&mut stack, latch, budget)?;
    // The closure stops at the header: preheader predecessors are not loop members.
    while let Some(node) = stack.pop() {
        budget.checkpoint()?;
        if members[index(node)?] {
            continue;
        }
        if !dom.dominates(header, node, budget)? {
            return Ok(false);
        }
        members[index(node)?] = true;
        count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        for &predecessor in &graph.predecessors[index(node)?] {
            budget.checkpoint()?;
            if !members[index(predecessor)?] {
                push(&mut stack, predecessor, budget)?;
            }
        }
    }
    let mut expected = 1usize;
    for &node in body {
        budget.checkpoint()?;
        if !members[index(node)?] {
            return Ok(false);
        }
        expected = expected
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    for branch in branches {
        budget.checkpoint()?;
        if let BodyBranch::Diamond(_) = branch {
            for node in branch.leaves().into_iter().flatten() {
                budget.checkpoint()?;
                if !members[index(node)?] {
                    return Ok(false);
                }
                expected = expected
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
        }
    }
    Ok(count == expected)
}
pub(super) fn plan(
    storage: &Flow,
    budget: &mut RunContext<'_>,
) -> Result<Option<Candidate>, RequestError> {
    let Some(graph) = graph(storage, budget)? else {
        return Ok(None);
    };
    let Some(dom) = dominators(&graph.successors, &graph.predecessors, BlockId(0), budget)? else {
        return Ok(None);
    };
    let Some(post) = dominators(
        &graph.predecessors,
        &graph.successors,
        graph.returned,
        budget,
    )?
    else {
        return Ok(None);
    };
    let n = storage.cases.len();
    let mut prelude = Vec::new();
    let mut seen = Vec::new();
    reserve(&mut seen, n, budget)?;
    for _ in 0..n {
        budget.checkpoint()?;
        seen.push(false);
    }
    let mut header = BlockId(0);
    loop {
        budget.checkpoint()?;
        if seen[index(header)?] {
            return Ok(None);
        }
        seen[index(header)?] = true;
        match &storage.cases[index(header)?].transfer {
            Transfer::Jump(edge) => {
                push(&mut prelude, header, budget)?;
                header = edge.target;
            }
            Transfer::Branch { .. } => break,
            _ => return Ok(None),
        }
    }
    let Transfer::Branch { yes, no, .. } = &storage.cases[index(header)?].transfer else {
        return Err(RequestError::InternalInvariant(
            "region header lost conditional",
        ));
    };
    for body_yes in [true, false] {
        budget.checkpoint()?;
        let (enter, leave) = if body_yes { (yes, no) } else { (no, yes) };
        let mut used = Vec::new();
        reserve(&mut used, n, budget)?;
        for &value in &seen {
            budget.checkpoint()?;
            used.push(value);
        }
        let mut body = Vec::new();
        let mut branches = Vec::new();
        let mut current = enter.target;
        let mut latch = None;
        let mut fits = true;
        while current != header {
            budget.checkpoint()?;
            if used[index(current)?] || !dom.dominates(header, current, budget)? {
                fits = false;
                break;
            }
            used[index(current)?] = true;
            push(&mut body, current, budget)?;
            let edge = match &storage.cases[index(current)?].transfer {
                Transfer::Jump(edge) => edge,
                Transfer::Branch { yes, no, .. } => {
                    let yes_exit = exit_stub(storage, yes.target, leave.target, budget)?;
                    let no_exit = exit_stub(storage, no.target, leave.target, budget)?;
                    let (exit, continuation, stub) = match (yes_exit, no_exit) {
                        (Some(stub), None) => (yes, no, stub),
                        (None, Some(stub)) => (no, yes, stub),
                        (None, None) => {
                            let Some(diamond) =
                                super::branch::diamond(storage, current, yes, no, budget)?
                            else {
                                fits = false;
                                break;
                            };
                            if diamond.join == header
                                || diamond.join == leave.target
                                || used[index(diamond.join)?]
                                || !post.dominates(diamond.join, current, budget)?
                            {
                                fits = false;
                                break;
                            }
                            for arm in [&diamond.yes, &diamond.no].into_iter().flatten() {
                                budget.checkpoint()?;
                                if used[index(arm.block)?]
                                    || !dom.dominates(header, arm.block, budget)?
                                {
                                    fits = false;
                                    break;
                                }
                                used[index(arm.block)?] = true;
                            }
                            if !fits {
                                break;
                            }
                            current = diamond.join;
                            push(&mut branches, BodyBranch::Diamond(diamond), budget)?;
                            continue;
                        }
                        _ => {
                            fits = false;
                            break;
                        }
                    };
                    if let Some(stub) = stub {
                        if used[index(stub)?] || !dom.dominates(header, stub, budget)? {
                            fits = false;
                            break;
                        }
                        used[index(stub)?] = true;
                    }
                    push(
                        &mut branches,
                        BodyBranch::Exit(Guard {
                            block: current,
                            exit: exit.id,
                            stub,
                        }),
                        budget,
                    )?;
                    continuation
                }
                _ => {
                    fits = false;
                    break;
                }
            };
            if edge.target == header {
                latch = Some(edge.id);
            }
            current = edge.target;
        }
        if !fits || body.is_empty() {
            continue;
        }
        let Some(latch) = latch else {
            continue;
        };
        let mut tail = Vec::new();
        current = leave.target;
        loop {
            budget.checkpoint()?;
            if used[index(current)?] {
                fits = false;
                break;
            }
            used[index(current)?] = true;
            push(&mut tail, current, budget)?;
            match &storage.cases[index(current)?].transfer {
                Transfer::Jump(edge) => current = edge.target,
                Transfer::Return => break,
                _ => {
                    fits = false;
                    break;
                }
            }
        }
        if !fits || !post.dominates(leave.target, header, budget)? {
            continue;
        }
        for &visited in &used {
            budget.checkpoint()?;
            fits &= visited;
        }
        if !fits {
            continue;
        }
        if !loop_kernel(&graph, &dom, header, &body, &branches, budget)? {
            continue;
        }
        return Ok(Some(Candidate {
            branches,
            prelude,
            header,
            body,
            tail,
            body_yes,
            enter: enter.id,
            leave: leave.id,
            latch,
        }));
    }
    Ok(None)
}
