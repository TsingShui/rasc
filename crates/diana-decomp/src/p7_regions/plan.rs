use super::{
    NaturalLoop, SwitchForm,
    flow::{Flow, Transfer},
    normal, verify,
};
use crate::p2_flow::normal::{BlockId, MethodFrame, Op, Terminal};
use crate::p6_variables::normal::StorageBody;
use crate::substrate::owned::push;
use crate::{MethodBudget, RepresentationPolicy, RequestError};

pub(crate) enum Region {
    Loop(NaturalLoop),
    Diamond(super::branch::Diamond),
}
pub(crate) enum Shape {
    Protected(Option<super::protection::Protection>),
    Dispatcher,
    Structured(Region),
}
pub(crate) struct PlannedBody<'a> {
    _sealed: (),
    pub storage: StorageBody<'a>,
    pub shape: Shape,
    pub switches: Vec<SwitchForm>,
}
fn diamond(
    storage: &StorageBody<'_>,
    flow: &Flow,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<super::branch::Diamond>, RequestError> {
    let ssa = &storage.typed.ssa;
    if flow.cases.len() != 4 || ssa.edges.len() != 4 || !matches!(ssa.frame, MethodFrame::Static) {
        return Ok(None);
    }
    for op in &ssa.operations {
        budget.charge_work(1)?;
        if op.op.effects().observable() {
            return Ok(None);
        }
    }
    let Terminal::Test { other: None, .. } = ssa.blocks[0].terminal else {
        return Ok(None);
    };
    let Transfer::Branch { yes, no } = &flow.cases[0].transfer else {
        return Ok(None);
    };
    let Some(diamond) = super::branch::diamond(flow, BlockId(0), yes, no, budget)? else {
        return Ok(None);
    };
    let (Some(y), Some(n)) = (&diamond.yes, &diamond.no) else {
        return Ok(None);
    };
    if [diamond.block, y.block, n.block, diamond.join]
        .iter()
        .any(|id| id.0 >= 4)
        || y.block == n.block
        || y.block == BlockId(0)
        || n.block == BlockId(0)
        || diamond.join == BlockId(0)
        || diamond.join == y.block
        || diamond.join == n.block
        || !matches!(
            ssa.blocks
                [usize::try_from(diamond.join.0).map_err(|_| RequestError::PlatformCapacity)?]
            .terminal,
            Terminal::Return(_)
        )
    {
        return Ok(None);
    }
    for &id in
        &ssa.bound[usize::try_from(diamond.join.0).map_err(|_| RequestError::PlatformCapacity)?].ops
    {
        budget.charge_work(1)?;
        if !matches!(ssa.operation(id)?.op, Op::Nop) {
            return Ok(None);
        }
    }
    Ok(Some(diamond))
}
pub(crate) fn build<'a>(
    storage: StorageBody<'a>,
    policy: RepresentationPolicy,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<PlannedBody<'a>, RequestError> {
    let mut switches = Vec::new();
    for table in &storage.typed.ssa.switches {
        budget.charge_work(1)?;
        push(
            &mut switches,
            super::integer_switch::choose(&table.cases, budget)?,
            budget,
        )?;
    }
    let mut has_monitors = false;
    for op in &storage.typed.ssa.operations {
        budget.charge_work(1)?;
        has_monitors |= matches!(op.op, Op::Monitor(..));
    }
    let shape = if has_monitors || !storage.typed.ssa.raw.try_byte_range().as_range().is_empty() {
        Shape::Protected(super::protection::build(&storage, budget)?)
    } else if matches!(policy, RepresentationPolicy::DispatchOnly) {
        Shape::Dispatcher
    } else {
        let flow = Flow::build(&storage, budget)?;
        if let Some(diamond) = diamond(&storage, &flow, budget)? {
            Shape::Structured(Region::Diamond(diamond))
        } else if let Some(candidate) = normal::plan(&flow, budget)? {
            Shape::Structured(Region::Loop(verify::verify(&flow, candidate, budget)?))
        } else {
            Shape::Dispatcher
        }
    };
    Ok(PlannedBody {
        _sealed: (),
        storage,
        shape,
        switches,
    })
}
