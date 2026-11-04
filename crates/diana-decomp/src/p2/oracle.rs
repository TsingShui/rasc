//! DEX effect obligations and terminal correspondence. Operation execution
//! relations are compared by replay; neither check calls the P2 builder.

use super::{BranchOp, Effects, FlowError, FlowMethod, SwitchKind, Terminal};
use crate::p1::{DecodedInstruction, DecodedMethod};
use crate::{CodeUnitOffset, RegisterId, RegisterSlot, SlotWidth};

pub(super) fn verify_operation(
    flow: &FlowMethod<'_>,
    instruction: &DecodedInstruction,
    operation: &super::Operation,
) -> Result<(), FlowError> {
    if operation.effects != expected_effects(instruction.opcode()) {
        return Err(FlowError::Correspondence {
            pc: instruction.pc(),
            obligation: "complete DEX effect obligations",
        });
    }
    super::replay::verify(flow, instruction, operation)
}

pub(super) fn verify_terminal(
    flow: &FlowMethod<'_>,
    block: &super::Block,
    original: &DecodedInstruction,
) -> Result<(), FlowError> {
    let valid = matches!(
        (&block.terminal, original.opcode()),
        (Some(Terminal::Goto { .. }), 0x28..=0x2a)
            | (Some(Terminal::Branch { .. }), 0x32..=0x3d)
            | (Some(Terminal::Switch { .. }), 0x2b | 0x2c)
            | (Some(Terminal::Return { .. }), 0x0e..=0x11)
            | (Some(Terminal::Throw { .. }), 0x27)
    );
    if !valid {
        return Err(FlowError::InternalInvariant(
            "terminal kind differs from original opcode",
        ));
    }
    let instruction = original;
    match &block.terminal {
        Some(Terminal::Goto { target }) => {
            if instruction.branch_target() != Some(*target) {
                return Err(FlowError::InternalInvariant(
                    "P2 goto target disagrees with P1",
                ));
            }
        }
        Some(Terminal::Branch {
            op,
            target,
            fallthrough,
        }) => {
            if instruction.branch_target() != Some(*target)
                || instruction.opcode() != branch_opcode(*op)?
                || instruction.pc().checked_advance(instruction.width()) != Some(*fallthrough)
            {
                return Err(FlowError::InternalInvariant(
                    "P2 branch terminal disagrees with P1",
                ));
            }
        }
        Some(Terminal::Switch {
            kind,
            payload,
            default,
            cases,
        }) => {
            if instruction.payload_target() != Some(*payload)
                || *default
                    != instruction
                        .pc()
                        .checked_advance(instruction.width())
                        .ok_or(FlowError::PlatformCapacity)?
                || *kind
                    != if instruction.opcode() == 0x2b {
                        SwitchKind::Packed
                    } else {
                        SwitchKind::Sparse
                    }
            {
                return Err(FlowError::InternalInvariant(
                    "P2 switch terminal disagrees with P1",
                ));
            }
            verify_switch_cases(flow.decoded(), instruction, cases)?;
        }
        Some(Terminal::Return { width, pc: site }) => {
            if *site != instruction.pc()
                || *width
                    != match instruction.opcode() {
                        0x0e => None,
                        0x10 => Some(SlotWidth::Two),
                        _ => Some(SlotWidth::One),
                    }
            {
                return Err(FlowError::InternalInvariant(
                    "P2 return terminal disagrees with P1",
                ));
            }
        }
        Some(Terminal::Throw { pc: site }) => {
            if *site != instruction.pc() {
                return Err(FlowError::InternalInvariant(
                    "P2 throw terminal disagrees with P1",
                ));
            }
        }
        None => {
            return Err(FlowError::InternalInvariant(
                "terminal block has no terminal",
            ));
        }
    }
    let count = match original.opcode() {
        0x0e | 0x28..=0x2a => 0,
        0x32..=0x37 => 2,
        _ => 1,
    };
    let reads = flow.terminal_reads(block);
    if reads.len() != count {
        return Err(FlowError::InternalInvariant("terminal read count differs"));
    }
    for (index, actual) in reads.iter().enumerate() {
        let width = if original.opcode() == 0x10 {
            SlotWidth::Two
        } else {
            SlotWidth::One
        };
        if *actual != slot(register(original, index)?, width) {
            return Err(FlowError::InternalInvariant(
                "terminal ordered slot differs from P1",
            ));
        }
    }
    Ok(())
}

fn branch_opcode(op: BranchOp) -> Result<u8, FlowError> {
    Ok(match op {
        BranchOp::Eq => 0x32,
        BranchOp::Ne => 0x33,
        BranchOp::Lt => 0x34,
        BranchOp::Ge => 0x35,
        BranchOp::Gt => 0x36,
        BranchOp::Le => 0x37,
        BranchOp::Eqz => 0x38,
        BranchOp::Nez => 0x39,
        BranchOp::Ltz => 0x3a,
        BranchOp::Gez => 0x3b,
        BranchOp::Gtz => 0x3c,
        BranchOp::Lez => 0x3d,
    })
}

fn verify_switch_cases(
    decoded: &DecodedMethod<'_>,
    instruction: &DecodedInstruction,
    actual: &[(i64, CodeUnitOffset)],
) -> Result<(), FlowError> {
    let pc = instruction
        .payload_target()
        .ok_or(FlowError::InternalInvariant("switch lost payload"))?;
    let index = decoded
        .items()
        .binary_search_by_key(&pc, |item| item.pc())
        .map_err(|_| FlowError::InternalInvariant("switch payload is missing from P1"))?;
    let crate::p1::DecodedItem::Payload(payload) = &decoded.items()[index] else {
        return Err(FlowError::InternalInvariant(
            "switch target is not a payload",
        ));
    };
    let original = payload
        .switch_cases(decoded.source())
        .map_err(super::build::request_error)?
        .ok_or(FlowError::InternalInvariant("switch payload has no cases"))?;
    if original.len() != actual.len() {
        return Err(FlowError::Correspondence {
            pc: instruction.pc(),
            obligation: "switch case count",
        });
    }
    for (case, (key, target)) in original.zip(actual) {
        if case.key != *key || super::check::relative(instruction.pc(), case.offset.0)? != *target {
            return Err(FlowError::Correspondence {
                pc: instruction.pc(),
                obligation: "switch case key/target order",
            });
        }
    }
    Ok(())
}

fn register(instruction: &DecodedInstruction, index: usize) -> Result<RegisterId, FlowError> {
    instruction
        .registers()
        .nth(index)
        .ok_or(FlowError::InternalInvariant(
            "terminal lost checked register",
        ))
}

fn slot(register: RegisterId, width: SlotWidth) -> RegisterSlot {
    RegisterSlot::new(register, width)
}

// Independent DEX effect obligations, not the builder's classifications.
fn expected_effects(opcode: u8) -> Effects {
    let mut effects = Effects::default();
    if matches!(opcode, 0x1a..=0x26 | 0x44..=0x72 | 0x74..=0x78 |
        0x93 | 0x94 | 0x9e | 0x9f | 0xb3 | 0xb4 | 0xbe | 0xbf |
        0xd3 | 0xd4 | 0xdb | 0xdc | 0xfa..=0xff)
    {
        effects = effects.with(Effects::MAY_THROW);
    }
    if matches!(opcode, 0x21 | 0x44..=0x4a | 0x52..=0x58 | 0x60..=0x66 |
        0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd)
    {
        effects = effects.with(Effects::HEAP_READ);
    }
    if matches!(opcode, 0x24..=0x26 | 0x4b..=0x51 | 0x59..=0x5f | 0x67..=0x6d |
        0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd)
    {
        effects = effects.with(Effects::HEAP_WRITE);
    }
    if matches!(opcode, 0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd) {
        effects = effects.with(Effects::CALL).with(Effects::CLASS_INIT);
    }
    if matches!(opcode, 0x1a | 0x1b | 0x22 | 0x60..=0x6d) {
        effects = effects.with(Effects::CLASS_INIT);
    }
    if matches!(opcode, 0x22..=0x25) {
        effects = effects.with(Effects::ALLOCATION);
    }
    if matches!(opcode, 0x1d | 0x1e) {
        effects = effects.with(Effects::MONITOR);
    }
    effects
}
