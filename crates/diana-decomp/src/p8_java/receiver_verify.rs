use crate::pipeline::receiver::{PrefixOp, ReceiverPlan, ResultUse};
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};

#[derive(Clone, Copy, PartialEq)]
enum Value {
    Undefined,
    Receiver,
    String(u32),
    Result(bool),
}
fn index(register: RegisterId) -> Result<usize, RequestError> {
    usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
}
fn mismatch<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "receiver AST plan differs from original definitions/effects",
    ))
}
pub(super) fn verify(
    plan: &ReceiverPlan<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let raw = &plan.raw;
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(Value::Undefined);
    }
    let entry = slots
        .len()
        .checked_sub(1)
        .ok_or(RequestError::InternalInvariant(
            "receiver replay has no input word",
        ))?;
    slots[entry] = Value::Receiver;
    let mut pc = CodeUnitOffset(0);
    let mut prefix = 0usize;
    let mut invoked = false;
    let mut result = false;
    let mut adjusted = false;
    let mut literal = 0u32;
    while pc.0 <= plan.returned.0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return mismatch();
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "receiver replay lost operand",
                ))
        };
        if pc.0 < plan.invoke.0 {
            match spec.opcode {
                0 => {}
                7..=9 => {
                    let value = slots[index(reg(1)?)?];
                    if value == Value::Undefined {
                        return mismatch();
                    }
                    slots[index(reg(0)?)?] = value;
                }
                0x1a | 0x1b => {
                    let Some(PrefixOp::String {
                        value,
                        reference,
                        pc: origin,
                    }) = plan.prefix.get(prefix)
                    else {
                        return mismatch();
                    };
                    if *origin != pc
                        || value.0 != literal
                        || !matches!(layout.references()[0],Some(reference_operand) if matches!(reference_operand.target,OperandReference::String(id) if id==*reference))
                    {
                        return mismatch();
                    }
                    slots[index(reg(0)?)?] = Value::String(literal);
                    literal = literal
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                    prefix = prefix
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                }
                0x71 | 0x77 => {
                    let Some(PrefixOp::Effect {
                        contract,
                        argument,
                        pc: origin,
                    }) = plan.prefix.get(prefix)
                    else {
                        return mismatch();
                    };
                    if *origin != pc
                        || operands.registers().iter().count() != 1
                        || slots[index(reg(0)?)?] != Value::String(argument.0)
                        || !matches!(layout.references()[0],Some(reference_operand) if matches!(reference_operand.target,OperandReference::Method(id) if id==contract.reference))
                    {
                        return mismatch();
                    }
                    prefix = prefix
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                }
                _ => return mismatch(),
            }
        } else if pc == plan.invoke {
            let expected = match plan.contract.kind {
                crate::p4_calls::ReceiverKind::Direct => matches!(spec.opcode, 0x70 | 0x76),
                crate::p4_calls::ReceiverKind::Super => matches!(spec.opcode, 0x6f | 0x75),
                crate::p4_calls::ReceiverKind::Virtual => matches!(spec.opcode, 0x6e | 0x74),
                crate::p4_calls::ReceiverKind::Interface => matches!(spec.opcode, 0x72 | 0x78),
            };
            if !expected
                || prefix != plan.prefix.len()
                || operands.registers().iter().count() != 1
                || reg(0)? != plan.receiver
                || slots[index(reg(0)?)?] != Value::Receiver
                || !matches!(layout.references()[0],Some(reference_operand) if matches!(reference_operand.target,OperandReference::Method(id) if id==plan.contract.reference))
            {
                return mismatch();
            }
            invoked = true;
        } else if pc == plan.result {
            if !invoked
                || spec.opcode != 0x0a
                || plan.invoke.checked_advance(crate::CodeUnitCount(3)) != Some(pc)
            {
                return mismatch();
            }
            slots[index(reg(0)?)?] = Value::Result(false);
            result = true;
        } else if pc == plan.returned {
            if !result
                || spec.opcode != 0x0f
                || slots[index(reg(0)?)?]
                    != Value::Result(matches!(plan.use_result, ResultUse::Add { .. }))
                || adjusted != matches!(plan.use_result, ResultUse::Add { .. })
            {
                return mismatch();
            }
            return Ok(());
        } else if let ResultUse::Add {
            literal,
            pc: origin,
        } = plan.use_result
            && pc == origin
        {
            let immediate = match operands.literal() {
                Some(RawLiteral::Signed8(v)) => i32::from(v),
                Some(RawLiteral::Signed16(v)) => i32::from(v),
                _ => return mismatch(),
            };
            if !result
                || adjusted
                || !matches!(spec.opcode, 0xd0 | 0xd8)
                || literal != immediate
                || slots[index(reg(1)?)?] != Value::Result(false)
            {
                return mismatch();
            }
            slots[index(reg(0)?)?] = Value::Result(true);
            adjusted = true;
        } else if spec.opcode != 0 {
            return mismatch();
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    mismatch()
}
