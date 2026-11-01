use super::graph::{dominators, graph, index, push};
use crate::p8_java::dispatcher::{Dispatcher, Transfer};
use crate::pipeline::branch::{BlockId, EdgeId};
use crate::substrate::owned::reserve;
use crate::{MethodBudget, RequestError};
pub(crate) struct Guard {
    pub block: BlockId,
    pub exit: EdgeId,
    pub stub: Option<BlockId>,
}
pub(super) struct Candidate {
    pub guards: Vec<Guard>,
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
    storage: &Dispatcher,
    target: BlockId,
    exit: BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Option<BlockId>>, RequestError> {
    budget.charge_work(1)?;
    if target == exit {
        return Ok(Some(None));
    }
    Ok(match &storage.cases[index(target)?].transfer {
        Transfer::Jump(edge) if edge.target == exit => Some(Some(target)),
        _ => None,
    })
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
        let mut guards = Vec::new();
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
            let edge = match &storage.cases[index(current)?].transfer {
                Transfer::Jump(edge) => edge,
                Transfer::Branch { yes, no, .. } => {
                    let yes_exit = exit_stub(storage, yes.target, leave.target, budget)?;
                    let no_exit = exit_stub(storage, no.target, leave.target, budget)?;
                    let (exit, continuation, stub) = match (yes_exit, no_exit) {
                        (Some(stub), None) => (yes, no, stub),
                        (None, Some(stub)) => (no, yes, stub),
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
                        &mut guards,
                        Guard {
                            block: current,
                            exit: exit.id,
                            stub,
                        },
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
            guards,
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
