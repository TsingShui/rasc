use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::p5_types::PrimitiveArray;
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError, TypeId};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ArrayAllocationId(pub CodeUnitOffset);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ConstantId(pub u32);
#[derive(Clone, Copy)]
pub(crate) struct Allocation {
    pub id: ArrayAllocationId,
    pub register: RegisterId,
    pub length_register: RegisterId,
    pub length: ConstantId,
    pub length_value: i32,
    pub reference: TypeId,
    pub kind: PrimitiveArray,
}
#[derive(Clone, Copy)]
pub(crate) struct Fill {
    pub pc: CodeUnitOffset,
    pub register: RegisterId,
    pub payload: CodeUnitOffset,
    pub count: u32,
}
pub(crate) enum Step {
    Constant {
        pc: CodeUnitOffset,
        register: RegisterId,
        value: ConstantId,
        literal: i32,
    },
    Allocate,
    Fill,
}
pub(crate) struct Candidate<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub steps: Vec<Step>,
    pub allocation: Allocation,
    pub fill: Fill,
    pub returned: CodeUnitOffset,
}
#[derive(Clone, Copy)]
enum Slot {
    Undefined,
    Int(ConstantId, i32),
    Array(ArrayAllocationId),
}
enum Stage {
    Start,
    Allocated(Allocation),
    Filled(Allocation, Fill),
}
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Candidate<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 8 == 0
        || raw.ins_size() != 0
        || !raw.try_byte_range().as_range().is_empty()
    {
        return Ok(None);
    }
    let Some(kind) = PrimitiveArray::parse(input.declaration().prototype.return_type, budget)?
    else {
        return Ok(None);
    };
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(Slot::Undefined);
    }
    let mut stage = Stage::Start;
    let mut steps = Vec::new();
    let mut next_constant = 0u32;
    let mut pc = CodeUnitOffset(0);
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        if !checked.layout().is_reachable(pc) {
            return Ok(None);
        }
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return Ok(None);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "array candidate lost checked register",
                ))
        };
        let ix = |register: RegisterId| {
            usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
        };
        match spec.opcode {
            0 => {}
            0x12..=0x15 => {
                let literal = match operands.literal() {
                    Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => return Err(RequestError::InternalInvariant("constant lost exact bits")),
                };
                let value = ConstantId(next_constant);
                next_constant = next_constant
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                let register = reg(0)?;
                slots[ix(register)?] = Slot::Int(value, literal);
                push(
                    &mut steps,
                    Step::Constant {
                        pc,
                        register,
                        value,
                        literal,
                    },
                    budget,
                )?;
            }
            0x07..=0x09 => {
                let source = reg(1)?;
                let value = slots[ix(source)?];
                match value {
                    Slot::Undefined => {
                        return Err(RequestError::UndefinedRegister {
                            register: source,
                            pc,
                        });
                    }
                    Slot::Array(_) => slots[ix(reg(0)?)?] = value,
                    _ => return Ok(None),
                }
            }
            0x23 => {
                if !matches!(stage, Stage::Start) {
                    return Ok(None);
                }
                let length_register = reg(1)?;
                let (length, length_value) = match slots[ix(length_register)?] {
                    Slot::Int(value, literal) => (value, literal),
                    Slot::Undefined => {
                        return Err(RequestError::UndefinedRegister {
                            register: length_register,
                            pc,
                        });
                    }
                    _ => return Ok(None),
                };
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant("array candidate lost type"));
                };
                let OperandReference::Type(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant("array type changed domain"));
                };
                let Some(allocated_kind) =
                    PrimitiveArray::parse(input.type_name(reference, budget)?, budget)?
                else {
                    return Ok(None);
                };
                let register = reg(0)?;
                let allocation = Allocation {
                    id: ArrayAllocationId(pc),
                    register,
                    length_register,
                    length,
                    length_value,
                    reference,
                    kind: allocated_kind,
                };
                slots[ix(register)?] = Slot::Array(allocation.id);
                push(&mut steps, Step::Allocate, budget)?;
                stage = Stage::Allocated(allocation);
            }
            0x26 => {
                let Stage::Allocated(allocation) = stage else {
                    return Err(RequestError::ArrayFillShape { pc });
                };
                let register = reg(0)?;
                if !matches!(slots[ix(register)?],Slot::Array(id) if id==allocation.id) {
                    return Err(RequestError::ArrayFillShape { pc });
                }
                let Some(delta) = operands.offset() else {
                    return Err(RequestError::InternalInvariant("array fill lost payload"));
                };
                let payload = CodeUnitOffset(
                    pc.0.checked_add_signed(delta.0)
                        .ok_or(RequestError::PlatformCapacity)?,
                );
                let data = instruction_layout(raw.instructions(), payload, raw.version())
                    .map_err(crate::input::MethodError::Decode)?;
                let LayoutKind::ArrayData {
                    element_width,
                    element_count,
                } = data.kind()
                else {
                    return Err(RequestError::InternalInvariant(
                        "array fill payload changed kind",
                    ));
                };
                if element_width.0 != allocation.kind.width() {
                    return Err(crate::input::MethodError::Malformed {
                        pc,
                        problem: crate::input::MethodProblem::InvalidArrayPayloadType,
                    }
                    .into());
                }
                if allocation.length_value < 0
                    || !u32::try_from(allocation.length_value)
                        .is_ok_and(|length| length >= element_count)
                {
                    return Err(RequestError::ArrayFillShape { pc });
                }
                let fill = Fill {
                    pc,
                    register,
                    payload,
                    count: element_count,
                };
                push(&mut steps, Step::Fill, budget)?;
                stage = Stage::Filled(allocation, fill);
            }
            0x11 => {
                let Stage::Filled(allocation, fill) = stage else {
                    return Ok(None);
                };
                if !matches!(slots[ix(reg(0)?)?],Slot::Array(id) if id==allocation.id) {
                    return Ok(None);
                }
                if allocation.kind != kind {
                    return Err(crate::input::MethodError::Malformed {
                        pc,
                        problem: crate::input::MethodProblem::InvalidResultKind,
                    }
                    .into());
                }
                return Ok(Some(Candidate {
                    _sealed: (),
                    raw,
                    steps,
                    allocation,
                    fill,
                    returned: pc,
                }));
            }
            _ => return Ok(None),
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(None)
}
