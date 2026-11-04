use super::flow::{Flow, Transfer};
use super::graph::index;
use super::normal::Candidate;
use crate::p2_flow::{BlockId, EdgeId};
use crate::substrate::owned::reserve;
use crate::{RequestError, RunContext};
pub(crate) struct NaturalLoop {
    _sealed: (),
    pub branches: Vec<super::BodyBranch>,
    pub prelude: Vec<BlockId>,
    pub header: BlockId,
    pub body: Vec<BlockId>,
    pub tail: Vec<BlockId>,
    pub body_yes: bool,
    pub enter: EdgeId,
    pub leave: EdgeId,
    pub latch: EdgeId,
}
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "natural region coverage/transfer certificate differs from storage CFG",
    ))
}
pub(super) fn verify(
    storage: &Flow,
    plan: Candidate,
    budget: &mut RunContext<'_>,
) -> Result<NaturalLoop, RequestError> {
    if plan.body.is_empty() || plan.tail.is_empty() {
        return bad();
    }
    let mut seen = Vec::new();
    reserve(&mut seen, storage.cases.len(), budget)?;
    for _ in &storage.cases {
        budget.checkpoint()?;
        seen.push(false);
    }
    for block in plan
        .prelude
        .iter()
        .chain(std::iter::once(&plan.header))
        .chain(plan.body.iter())
        .chain(plan.tail.iter())
    {
        budget.checkpoint()?;
        let Some(visited) = seen.get_mut(index(*block)?) else {
            return bad();
        };
        if *visited || storage.cases[index(*block)?].key != *block {
            return bad();
        }
        *visited = true;
    }
    for branch in &plan.branches {
        for block in branch.leaves().into_iter().flatten() {
            budget.checkpoint()?;
            let Some(visited) = seen.get_mut(index(block)?) else {
                return bad();
            };
            if *visited || storage.cases[index(block)?].key != block {
                return bad();
            }
            *visited = true;
        }
    }
    for &visited in &seen {
        budget.checkpoint()?;
        if !visited {
            return bad();
        }
    }
    let first = plan.prelude.first().copied().unwrap_or(plan.header);
    if first != BlockId(0) {
        return bad();
    }
    for (at, &node) in plan.prelude.iter().enumerate() {
        budget.checkpoint()?;
        let target = plan.prelude.get(at + 1).copied().unwrap_or(plan.header);
        if !matches!(&storage.cases[index(node)?].transfer,Transfer::Jump(edge) if edge.target==target)
        {
            return bad();
        }
    }
    let Transfer::Branch { yes, no, .. } = &storage.cases[index(plan.header)?].transfer else {
        return bad();
    };
    let (enter, leave) = if plan.body_yes { (yes, no) } else { (no, yes) };
    if enter.id != plan.enter
        || leave.id != plan.leave
        || plan.body.first() != Some(&enter.target)
        || plan.tail.first() != Some(&leave.target)
    {
        return bad();
    }
    let mut branches = plan.branches.iter();
    for (at, &node) in plan.body.iter().enumerate() {
        budget.checkpoint()?;
        let target = plan.body.get(at + 1).copied().unwrap_or(plan.header);
        let edge = match &storage.cases[index(node)?].transfer {
            Transfer::Jump(edge) => edge,
            Transfer::Branch { yes, no, .. } => {
                let Some(branch) = branches.next() else {
                    return bad();
                };
                if branch.block() != node {
                    return bad();
                }
                let guard = match branch {
                    super::BodyBranch::Exit(guard) => guard,
                    super::BodyBranch::Diamond(diamond) => {
                        if diamond.join != target
                            || diamond.join == plan.header
                            || diamond.yes_entry != yes.id
                            || diamond.no_entry != no.id
                        {
                            return bad();
                        }
                        for (edge, arm) in [(yes, &diamond.yes), (no, &diamond.no)] {
                            budget.checkpoint()?;
                            match arm {
                                Some(arm) => {
                                    if edge.target != arm.block {
                                        return bad();
                                    }
                                    if !matches!(&storage.cases[index(arm.block)?].transfer,Transfer::Jump(exit) if exit.target==target&&exit.id==arm.exit)
                                    {
                                        return bad();
                                    }
                                }
                                None => {
                                    if edge.target != target {
                                        return bad();
                                    }
                                }
                            }
                        }
                        continue;
                    }
                };
                let (exit, continuation) = if guard.exit == yes.id {
                    (yes, no)
                } else if guard.exit == no.id {
                    (no, yes)
                } else {
                    return bad();
                };
                if exit.target != guard.stub.unwrap_or(leave.target) {
                    return bad();
                }
                if let Some(stub) = guard.stub {
                    budget.checkpoint()?;
                    if !matches!(&storage.cases[index(stub)?].transfer,Transfer::Jump(edge) if edge.target==leave.target)
                    {
                        return bad();
                    }
                }
                continuation
            }
            _ => return bad(),
        };
        if edge.target != target || (at + 1 == plan.body.len() && edge.id != plan.latch) {
            return bad();
        }
    }
    if branches.next().is_some() {
        return bad();
    }
    for (at, &node) in plan.tail.iter().enumerate() {
        budget.checkpoint()?;
        match plan.tail.get(at + 1) {
            Some(&next) => {
                if !matches!(&storage.cases[index(node)?].transfer,Transfer::Jump(edge) if edge.target==next)
                {
                    return bad();
                }
            }
            None => {
                if !matches!(storage.cases[index(node)?].transfer, Transfer::Return) {
                    return bad();
                }
            }
        }
    }
    Ok(NaturalLoop {
        _sealed: (),
        branches: plan.branches,
        prelude: plan.prelude,
        header: plan.header,
        body: plan.body,
        tail: plan.tail,
        body_yes: plan.body_yes,
        enter: plan.enter,
        leave: plan.leave,
        latch: plan.latch,
    })
}
