use crate::p2_flow::{Op, OpId};
use crate::p4_types::initialization::Initialization;
use crate::p4_types::{Kind, TypedBody};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

pub(crate) struct DeferredAllocation {
    _sealed: (),
    pub allocation: OpId,
    pub init: OpId,
    pub root: super::LocalId,
    pub aliases: Vec<super::LocalId>,
}
pub(super) fn prove(
    typed: &TypedBody<'_>,
    plan: &Initialization,
    budget: &mut RunContext<'_>,
) -> Result<DeferredAllocation, RequestError> {
    let broken = || {
        RequestError::InternalInvariant("deferred initialization changed original sites or aliases")
    };
    let allocation = typed.ssa.operation(plan.allocation)?;
    let init = typed.ssa.operation(plan.init)?;
    let Op::NewInstance(destination, site) = allocation.op else {
        return Err(broken());
    };
    if !matches!(init.op,Op::Initialize(call) if call == plan.call) || allocation.pc.0 >= init.pc.0
    {
        return Err(broken());
    }
    let site = typed
        .ssa
        .object_creations
        .get(usize::try_from(site.0).map_err(|_| RequestError::PlatformCapacity)?)
        .ok_or_else(broken)?;
    if site.reference != plan.object {
        return Err(broken());
    }
    let raw = &typed.ssa.raw;
    let layout = crate::raw::instruction_layout(raw.instructions(), allocation.pc, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    let operands =
        crate::input::operands::instruction_operands(raw.instructions(), allocation.pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x22)
        || !matches!(layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Type(plan.object))
        || registers.next() != Some(destination)
        || registers.next().is_some()
    {
        return Err(broken());
    }
    let root = allocation.result.ok_or_else(broken)?;
    let mut aliases = Vec::new();
    for alias in &plan.aliases {
        budget.checkpoint()?;
        let value = alias.value();
        if typed.kinds[usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
            != Some(Kind::Nominal(plan.object))
        {
            return Err(broken());
        }
        if value != root {
            let crate::p3_ssa::Definition::OpResult(id) = typed.ssa.values
                [usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
            else {
                return Err(broken());
            };
            let op = typed.ssa.operation(id)?;
            let layout = crate::raw::instruction_layout(raw.instructions(), op.pc, raw.version())
                .map_err(crate::input::MethodError::Decode)?;
            if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,7..=9))
            {
                return Err(RequestError::InitializationUnavailable { pc: op.pc });
            }
        }
        push(&mut aliases, super::LocalId(value.0), budget)?;
    }
    let layout = crate::raw::instruction_layout(raw.instructions(), init.pc, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    let site = typed
        .ssa
        .calls
        .get(usize::try_from(plan.call.0).map_err(|_| RequestError::PlatformCapacity)?)
        .ok_or_else(broken)?;
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x70|0x76))
        || !matches!(layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Method(site.reference))
    {
        return Err(broken());
    }
    Ok(DeferredAllocation {
        _sealed: (),
        allocation: plan.allocation,
        init: plan.init,
        root: super::LocalId(root.0),
        aliases,
    })
}
