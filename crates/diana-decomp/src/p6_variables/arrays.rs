use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::fresh_array::{
    Allocation, ArrayAllocationId, Candidate, ConstantId, Fill, Step,
};
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};
pub(crate) struct FreshArray {
    _sealed: (),
    pub steps: Vec<Step>,
    pub allocation: Allocation,
    pub fill: Fill,
    pub returned: CodeUnitOffset,
}
#[derive(Clone, Copy)]
enum Value {
    Undefined,
    Constant(ConstantId, i32),
    Array(ArrayAllocationId),
}
enum State {
    Entry,
    Allocated,
    Filled,
}
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "FreshArray allocation/length/alias/no-escape certificate differs from original code",
    ))
}
pub(crate) fn prove(
    input: &MethodInput<'_, '_>,
    candidate: Candidate<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<FreshArray, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return bad();
    };
    let raw = checked.layout().original();
    if !std::ptr::eq(raw.instructions(), candidate.raw.instructions())
        || input.access_flags() & 8 == 0
        || raw.ins_size() != 0
        || !raw.try_byte_range().as_range().is_empty()
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(candidate.allocation.kind.descriptor(), budget)?
    {
        return bad();
    }
    let mut values = Vec::new();
    reserve(&mut values, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        values.push(Value::Undefined);
    }
    let mut state = State::Entry;
    let mut cursor = 0usize;
    let mut next_constant = 0u32;
    let mut pc = CodeUnitOffset(0);
    loop {
        budget.charge_work(1)?;
        if !checked.layout().is_reachable(pc) {
            return bad();
        }
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return bad();
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "FreshArray replay lost checked register",
                ))
        };
        let ix = |register: RegisterId| {
            usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
        };
        match spec.opcode {
            0 => {}
            0x12..=0x15 => {
                let Some(Step::Constant {
                    pc: origin,
                    register,
                    value,
                    literal,
                }) = candidate.steps.get(cursor)
                else {
                    return bad();
                };
                let bits = match operands.literal() {
                    Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => return bad(),
                };
                if *origin != pc
                    || *register != reg(0)?
                    || *literal != bits
                    || value.0 != next_constant
                {
                    return bad();
                }
                values[ix(*register)?] = Value::Constant(*value, bits);
                next_constant = next_constant
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x07..=0x09 => {
                let source = reg(1)?;
                let Value::Array(id) = values[ix(source)?] else {
                    return bad();
                };
                if !matches!(state, State::Allocated | State::Filled)
                    || id != candidate.allocation.id
                {
                    return bad();
                }
                values[ix(reg(0)?)?] = Value::Array(id);
            }
            0x23 => {
                let allocation = &candidate.allocation;
                if !matches!(state, State::Entry)
                    || !matches!(candidate.steps.get(cursor), Some(Step::Allocate))
                    || allocation.id.0 != pc
                    || allocation.register != reg(0)?
                    || allocation.length_register != reg(1)?
                    || !matches!(layout.references()[0],Some(reference) if reference.target==OperandReference::Type(allocation.reference))
                    || !input
                        .type_name(allocation.reference, budget)?
                        .matches_with(allocation.kind.descriptor(), budget)?
                {
                    return bad();
                }
                let Value::Constant(value, length) = values[ix(reg(1)?)?] else {
                    return bad();
                };
                if value != allocation.length || length != allocation.length_value || length < 0 {
                    return bad();
                }
                values[ix(reg(0)?)?] = Value::Array(allocation.id);
                state = State::Allocated;
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x26 => {
                let fill = &candidate.fill;
                if !matches!(state, State::Allocated)
                    || !matches!(candidate.steps.get(cursor), Some(Step::Fill))
                    || fill.pc != pc
                    || fill.register != reg(0)?
                    || !matches!(values[ix(reg(0)?)?],Value::Array(id) if id==candidate.allocation.id)
                {
                    return bad();
                }
                let Some(delta) = operands.offset() else {
                    return bad();
                };
                if pc.0.checked_add_signed(delta.0) != Some(fill.payload.0) {
                    return bad();
                }
                let data = instruction_layout(raw.instructions(), fill.payload, raw.version())
                    .map_err(crate::input::MethodError::Decode)?;
                if !matches!(data.kind(),LayoutKind::ArrayData{element_width,element_count} if element_width.0==candidate.allocation.kind.width()&&element_count==fill.count)
                    || !u32::try_from(candidate.allocation.length_value)
                        .is_ok_and(|length| length >= fill.count)
                {
                    return bad();
                }
                state = State::Filled;
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x11 => {
                if !matches!(state, State::Filled)
                    || pc != candidate.returned
                    || cursor != candidate.steps.len()
                    || !matches!(values[ix(reg(0)?)?],Value::Array(id) if id==candidate.allocation.id)
                {
                    return bad();
                }
                return Ok(FreshArray {
                    _sealed: (),
                    steps: candidate.steps,
                    allocation: candidate.allocation,
                    fill: candidate.fill,
                    returned: pc,
                });
            }
            _ => return bad(),
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
}
