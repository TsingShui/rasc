use super::graph::{dominators, graph, index, push};
use crate::p8_java::dispatcher::{Dispatcher, Transfer};
use crate::pipeline::branch::{BlockId, EdgeId};
use crate::substrate::owned::reserve;
use crate::{MethodBudget, RequestError};
pub(super) struct Candidate {
    pub prelude: Vec<BlockId>,
    pub header: BlockId,
    pub body: Vec<BlockId>,
    pub tail: Vec<BlockId>,
    pub body_yes: bool,
    pub enter: EdgeId,
    pub leave: EdgeId,
    pub latch: EdgeId,
}
pub(super) fn plan(
    storage: &Dispatcher,
    budget: &mut MethodBudget<'_, '_>,
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
        budget.charge_work(1)?;
        seen.push(false);
    }
    let mut header = BlockId(0);
    loop {
        budget.charge_work(1)?;
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
        budget.charge_work(1)?;
        let (enter, leave) = if body_yes { (yes, no) } else { (no, yes) };
        let mut used = Vec::new();
        reserve(&mut used, n, budget)?;
        for &value in &seen {
            budget.charge_work(1)?;
            used.push(value);
        }
        let mut body = Vec::new();
        let mut current = enter.target;
        let mut latch = None;
        let mut fits = true;
        while current != header {
            budget.charge_work(1)?;
            if used[index(current)?] || !dom.dominates(header, current, budget)? {
                fits = false;
                break;
            }
            used[index(current)?] = true;
            push(&mut body, current, budget)?;
            let Transfer::Jump(edge) = &storage.cases[index(current)?].transfer else {
                fits = false;
                break;
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
            budget.charge_work(1)?;
            if used[index(current)?] {
                fits = false;
                break;
            }
            used[index(current)?] = true;
            push(&mut tail, current, budget)?;
            match &storage.cases[index(current)?].transfer {
                Transfer::Jump(edge) => current = edge.target,
                Transfer::Return(_) | Transfer::ReturnVoid => break,
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
            budget.charge_work(1)?;
            fits &= visited;
        }
        if !fits {
            continue;
        }
        return Ok(Some(Candidate {
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
