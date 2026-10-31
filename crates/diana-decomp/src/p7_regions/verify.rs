use super::graph::index;
use super::normal::Candidate;
use crate::p8_java::dispatcher::{Dispatcher, Transfer};
use crate::pipeline::branch::{BlockId, EdgeId};
use crate::substrate::owned::reserve;
use crate::{MethodBudget, RequestError};
pub(crate) struct NaturalLoop {
    _sealed: (),
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
    storage: &Dispatcher,
    plan: Candidate,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<NaturalLoop, RequestError> {
    if plan.body.is_empty() || plan.tail.is_empty() {
        return bad();
    }
    let mut seen = Vec::new();
    reserve(&mut seen, storage.cases.len(), budget)?;
    for _ in &storage.cases {
        budget.charge_work(1)?;
        seen.push(false);
    }
    for block in plan
        .prelude
        .iter()
        .chain(std::iter::once(&plan.header))
        .chain(plan.body.iter())
        .chain(plan.tail.iter())
    {
        budget.charge_work(1)?;
        let Some(visited) = seen.get_mut(index(*block)?) else {
            return bad();
        };
        if *visited || storage.cases[index(*block)?].key != *block {
            return bad();
        }
        *visited = true;
    }
    for &visited in &seen {
        budget.charge_work(1)?;
        if !visited {
            return bad();
        }
    }
    let first = plan.prelude.first().copied().unwrap_or(plan.header);
    if first != BlockId(0) {
        return bad();
    }
    for (at, &node) in plan.prelude.iter().enumerate() {
        budget.charge_work(1)?;
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
    for (at, &node) in plan.body.iter().enumerate() {
        budget.charge_work(1)?;
        let target = plan.body.get(at + 1).copied().unwrap_or(plan.header);
        let Transfer::Jump(edge) = &storage.cases[index(node)?].transfer else {
            return bad();
        };
        if edge.target != target || (at + 1 == plan.body.len() && edge.id != plan.latch) {
            return bad();
        }
    }
    for (at, &node) in plan.tail.iter().enumerate() {
        budget.charge_work(1)?;
        match plan.tail.get(at + 1) {
            Some(&next) => {
                if !matches!(&storage.cases[index(node)?].transfer,Transfer::Jump(edge) if edge.target==next)
                {
                    return bad();
                }
            }
            None => {
                if !matches!(
                    storage.cases[index(node)?].transfer,
                    Transfer::Return(_) | Transfer::ReturnVoid
                ) {
                    return bad();
                }
            }
        }
    }
    Ok(NaturalLoop {
        _sealed: (),
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
