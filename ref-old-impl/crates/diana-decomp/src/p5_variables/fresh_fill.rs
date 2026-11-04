use crate::p2_flow::{Op, OpId};
use crate::p3_ssa::{BoundOp, Definition, ValueId};
use crate::p4_types::TypedBody;
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RequestError, RunContext};

pub(crate) struct FreshArray {
    _sealed: (),
    pub allocation: OpId,
    pub array: ValueId,
    pub pc: CodeUnitOffset,
    pub payload: CodeUnitOffset,
    pub elements: Vec<crate::p4_types::ArrayElement>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn origin<'s>(
    typed: &'s TypedBody<'_>,
    mut value: ValueId,
    budget: &mut RunContext<'_>,
) -> Result<Option<&'s BoundOp>, RequestError> {
    for _ in &typed.ssa.values {
        budget.checkpoint()?;
        let Definition::OpResult(id) = typed.ssa.values[ix(value.0)?] else {
            return Ok(None);
        };
        let op = typed.ssa.operation(id)?;
        if matches!(op.op, Op::Move(..)) {
            value = op.input(0)?;
        } else {
            return Ok(Some(op));
        }
    }
    Err(RequestError::InternalInvariant(
        "cyclic move origin in FreshArray",
    ))
}
pub(super) fn prove(
    typed: &TypedBody<'_>,
    fill: &BoundOp,
    budget: &mut RunContext<'_>,
) -> Result<FreshArray, RequestError> {
    let unavailable = || RequestError::ArrayFillShape { pc: fill.pc };
    let broken = || {
        RequestError::InternalInvariant("FreshArray differs from original allocation or payload")
    };
    let Op::FreshFill(register, id) = fill.op else {
        return Err(broken());
    };
    let site = typed.ssa.fresh_fills.get(ix(id.0)?).ok_or_else(broken)?;
    let usage = typed.fresh_fills.get(ix(id.0)?).ok_or_else(broken)?;
    let array = fill.input(0)?;
    if usage.array != array {
        return Err(broken());
    }
    let allocation = origin(typed, array, budget)?.ok_or_else(unavailable)?;
    let Op::NewArray(destination, length_register, creation) = allocation.op else {
        return Err(unavailable());
    };
    let creation = typed
        .ssa
        .array_creations
        .get(ix(creation.0)?)
        .ok_or_else(broken)?;
    if creation.component != crate::p2_flow::ArrayKind::Primitive(usage.component)
        || allocation.pc.0 >= fill.pc.0
    {
        return Err(unavailable());
    }
    let length = origin(typed, allocation.input(0)?, budget)?.ok_or_else(unavailable)?;
    let Op::Constant(length_destination, literal) = length.op else {
        return Err(unavailable());
    };
    if !u32::try_from(literal).is_ok_and(|value| value >= site.count) {
        return Err(unavailable());
    }
    let mut same_block = false;
    for block in &typed.ssa.blocks {
        budget.checkpoint()?;
        if block.pc.0 <= allocation.pc.0 && fill.pc.0 <= block.terminal_pc.0 {
            same_block = true;
            for op in &block.ops {
                budget.checkpoint()?;
                if allocation.pc.0 < op.pc.0
                    && op.pc.0 < fill.pc.0
                    && op.kind.effects().observable()
                {
                    return Err(unavailable());
                }
            }
            break;
        }
    }
    if !same_block {
        return Err(unavailable());
    }
    let raw = &typed.ssa.raw;
    let allocation_layout =
        crate::raw::instruction_layout(raw.instructions(), allocation.pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
    let allocation_operands = crate::input::operands::instruction_operands(
        raw.instructions(),
        allocation.pc,
        allocation_layout,
    )
    .map_err(crate::input::MethodError::Decode)?;
    let mut registers = allocation_operands.registers().iter();
    if !matches!(allocation_layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x23)
        || !matches!(allocation_layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Type(creation.reference))
        || registers.next() != Some(destination)
        || registers.next() != Some(length_register)
        || registers.next().is_some()
    {
        return Err(broken());
    }
    let constant_layout =
        crate::raw::instruction_layout(raw.instructions(), length.pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
    let constant_operands = crate::input::operands::instruction_operands(
        raw.instructions(),
        length.pc,
        constant_layout,
    )
    .map_err(crate::input::MethodError::Decode)?;
    let bits = match constant_operands.literal() {
        Some(crate::raw::RawLiteral::Bits32(value)) => i32::from_le_bytes(value.to_le_bytes()),
        Some(crate::raw::RawLiteral::Signed8(value)) => i32::from(value),
        Some(crate::raw::RawLiteral::Signed16(value)) => i32::from(value),
        _ => return Err(broken()),
    };
    if !matches!(constant_layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x12..=0x15))
        || bits != literal
        || constant_operands.registers().iter().next() != Some(length_destination)
    {
        return Err(broken());
    }
    let layout = crate::raw::instruction_layout(raw.instructions(), fill.pc, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    let operands =
        crate::input::operands::instruction_operands(raw.instructions(), fill.pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x26)
        || registers.next() != Some(register)
        || registers.next().is_some()
        || !matches!(operands.offset(),Some(delta) if fill.pc.0.checked_add_signed(delta.0) == Some(site.payload.0))
    {
        return Err(broken());
    }
    let data = crate::raw::instruction_layout(raw.instructions(), site.payload, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(data.kind(),crate::raw::LayoutKind::ArrayData{ element_width, element_count } if element_width.0 == usage.component.width() && element_count == site.count)
    {
        return Err(broken());
    }
    let payload = crate::input::operands::array_payload(raw.instructions(), site.payload, data)
        .map_err(crate::input::MethodError::Decode)?;
    let mut elements = Vec::new();
    for value in payload {
        budget.checkpoint()?;
        if value.width.0 != usage.component.width() {
            return Err(broken());
        }
        let element = usage
            .component
            .element(value.bits)
            .ok_or_else(unavailable)?;
        push(&mut elements, element, budget)?;
    }
    if elements.len() != usize::try_from(site.count).map_err(|_| RequestError::PlatformCapacity)? {
        return Err(broken());
    }
    Ok(FreshArray {
        _sealed: (),
        allocation: allocation.id,
        array,
        pc: fill.pc,
        payload: site.payload,
        elements,
    })
}
