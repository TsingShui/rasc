use super::{
    NaturalLoop, SwitchForm,
    flow::{Flow, Transfer},
    normal, verify,
};
use crate::p2_flow::{BlockId, Op, Terminal};
use crate::p5_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{RepresentationPolicy, RequestError, RunContext};

pub(crate) enum Region {
    Sequence(BlockId),
    Loop(NaturalLoop),
    Diamond(super::branch::Diamond),
}
pub(crate) enum Shape {
    Protected(Option<super::protection::Protection>),
    Dispatcher,
    Structured(Region),
}
crate::substrate::checked::checked_product!(PlannedBody, PlannedBodyData);
impl<'a> PlannedBody<'a> {
    pub(crate) fn into_data(self) -> PlannedBodyData<'a> {
        self.0
    }
}
pub(crate) struct PlannedBodyData<'a> {
    _sealed: (),
    pub storage: StorageBody<'a>,
    pub shape: Shape,
    pub switches: Vec<SwitchForm>,
}
fn diamond(
    storage: &StorageBody<'_>,
    flow: &Flow,
    budget: &mut RunContext<'_>,
) -> Result<Option<super::branch::Diamond>, RequestError> {
    let ssa = &storage.typed.ssa;
    if flow.cases.len() != 4 || ssa.edges.len() != 4 {
        return Ok(None);
    }
    let Terminal::Test { .. } = ssa.blocks[0].terminal else {
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
            Terminal::Return(_) | Terminal::ReturnVoid
        )
    {
        return Ok(None);
    }
    Ok(Some(diamond))
}
pub(crate) fn build<'a>(
    _schedule: &crate::engine::SchedulePermit,
    storage: StorageBody<'a>,
    policy: RepresentationPolicy,
    budget: &mut RunContext<'_>,
) -> Result<PlannedBody<'a>, RequestError> {
    build_impl(storage, policy, budget)
}

fn build_impl<'a>(
    storage: StorageBody<'a>,
    policy: RepresentationPolicy,
    budget: &mut RunContext<'_>,
) -> Result<PlannedBody<'a>, RequestError> {
    let mut switches = Vec::new();
    for table in &storage.typed.ssa.switches {
        budget.checkpoint()?;
        push(
            &mut switches,
            super::integer_switch::choose(&table.cases, budget)?,
            budget,
        )?;
    }
    let mut has_monitors = false;
    for op in &storage.typed.ssa.operations {
        budget.checkpoint()?;
        has_monitors |= matches!(op.op, Op::Monitor(..));
    }
    let shape = if has_monitors || !storage.typed.ssa.raw.try_byte_range().as_range().is_empty() {
        Shape::Protected(super::protection::build(&storage, budget)?)
    } else if matches!(policy, RepresentationPolicy::DispatchOnly) {
        Shape::Dispatcher
    } else {
        let flow = Flow::build(&storage, budget)?;
        let mut initialization = matches!(
            storage.typed.ssa.frame,
            crate::p2_flow::MethodFrame::Constructor(_)
        );
        for evidence in storage.typed.field_declarations.iter().flatten() {
            budget.checkpoint()?;
            initialization |= matches!(&evidence.declaration, crate::p4_types::bindings::FieldDeclaration::Known(access) if access.assignment == crate::p4_types::declarations::field_facts::AssignmentObligation::OwnFinalInitialization);
        }
        if initialization
            && flow.cases.len() == 1
            && matches!(flow.cases[0].transfer, Transfer::Return)
        {
            Shape::Structured(Region::Sequence(BlockId(0)))
        } else if let Some(diamond) = diamond(&storage, &flow, budget)? {
            Shape::Structured(Region::Diamond(diamond))
        } else if let Some(candidate) = normal::plan(&flow, budget)? {
            Shape::Structured(Region::Loop(verify::verify(&flow, candidate, budget)?))
        } else {
            Shape::Dispatcher
        }
    };
    Ok(PlannedBody(PlannedBodyData {
        _sealed: (),
        storage,
        shape,
        switches,
    }))
}
