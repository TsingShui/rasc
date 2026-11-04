use crate::p2_flow::Op;
use crate::p3_ssa::{BoundOp, ValueId};
use crate::p4_types::{Kind, TypedBody};
use crate::{CodeUnitOffset, RequestError, RunContext};

pub(crate) struct LengthOnly {
    _sealed: (),
    pub array: ValueId,
    pub result: super::LocalId,
    pub pc: CodeUnitOffset,
    pub payload: CodeUnitOffset,
}
pub(super) fn prove(
    typed: &TypedBody<'_>,
    op: &BoundOp,
    result: super::LocalId,
    budget: &mut RunContext<'_>,
) -> Result<LengthOnly, RequestError> {
    budget.checkpoint()?;
    let bad = || RequestError::InternalInvariant("LengthOnly differs from original zero fill");
    let Op::EmptyFill(register, id) = op.op else {
        return Err(bad());
    };
    let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
    let site = typed.ssa.empty_fills.get(index).ok_or_else(bad)?;
    let usage = typed.empty_fills.get(index).ok_or_else(bad)?;
    let value = op.input(0)?;
    let kind = typed.kinds[usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?];
    if value != usage.array
        || kind != Some(Kind::PrimitiveArray(usage.component))
        || usage.component.width() != site.width
        || op.result.is_some()
    {
        return Err(bad());
    }
    let layout = crate::raw::instruction_layout(
        typed.ssa.raw.instructions(),
        op.pc,
        typed.ssa.raw.version(),
    )
    .map_err(crate::input::MethodError::Decode)?;
    let operands =
        crate::input::operands::instruction_operands(typed.ssa.raw.instructions(), op.pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x26)
        || registers.next() != Some(register)
        || registers.next().is_some()
        || !matches!(operands.offset(),Some(delta) if op.pc.0.checked_add_signed(delta.0) == Some(site.payload.0))
    {
        return Err(bad());
    }
    let data = crate::raw::instruction_layout(
        typed.ssa.raw.instructions(),
        site.payload,
        typed.ssa.raw.version(),
    )
    .map_err(crate::input::MethodError::Decode)?;
    if !matches!(data.kind(),crate::raw::LayoutKind::ArrayData { element_width, element_count: 0 } if element_width.0 == usage.component.width())
    {
        return Err(bad());
    }
    Ok(LengthOnly {
        _sealed: (),
        array: value,
        result,
        pc: op.pc,
        payload: site.payload,
    })
}
