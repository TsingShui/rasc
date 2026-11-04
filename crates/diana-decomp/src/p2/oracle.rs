//! Independent P1-to-P2 operation trace checks. This module derives expected
//! reads, writes, result commit and throwability from the P1 instruction facts;
//! it never calls the P2 translation table.

use std::collections::BTreeMap;

use super::{
    ArrayAccess, BinaryForm, BinaryOp, BranchOp, CommitRule, CompareOp, FieldAccess, FlowError,
    FlowMethod, InvokeKind, MoveKind, OpKind, SwitchKind, Terminal, UnaryOp, ValueKind,
};
use crate::input::formats::FlowKind;
use crate::p1::{DecodedInstruction, DecodedMethod};
use crate::raw::{OperandReference, RawLiteral};
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{CodeUnitOffset, RegisterId, RunContext, TypeId};

struct Expected {
    reads: Vec<RegisterSlot>,
    writes: Vec<RegisterSlot>,
    may_throw: bool,
    commit: CommitRule,
}

pub(crate) fn verify(flow: &FlowMethod<'_>, control: &mut RunContext<'_>) -> Result<(), FlowError> {
    let decoded = flow.decoded();
    let mut operations = BTreeMap::new();
    for operation in flow.operations() {
        if operations.insert(operation.pc, operation).is_some() {
            return Err(FlowError::InternalInvariant(
                "two P2 operations share one original pc",
            ));
        }
    }
    for instruction in decoded.instructions() {
        control.checkpoint()?;
        if !decoded.is_reachable(instruction.pc()) {
            if operations.contains_key(&instruction.pc()) {
                return Err(FlowError::InternalInvariant(
                    "unreachable instruction has a P2 operation",
                ));
            }
            continue;
        }
        if is_terminal(instruction) || matches!(instruction.opcode(), 0x0a..=0x0c) {
            continue;
        }
        let expected = expected(decoded, instruction, control)?;
        let Some(operation) = operations.remove(&instruction.pc()) else {
            return Err(FlowError::InternalInvariant(
                "reachable instruction has no P2 operation",
            ));
        };
        if flow.reads(operation) != expected.reads.as_slice()
            || flow.writes(operation) != expected.writes.as_slice()
        {
            return Err(FlowError::InternalInvariant(
                "P2 operation slots disagree with the P1 instruction trace",
            ));
        }
        if operation.effects.may_throw() != expected.may_throw {
            return Err(FlowError::InternalInvariant(
                "P2 may-throw fact disagrees with the P1 instruction trace",
            ));
        }
        if operation.commit != expected.commit {
            return Err(FlowError::InternalInvariant(
                "P2 commit rule disagrees with the P1 instruction trace",
            ));
        }
        check_kind(decoded, instruction, operation)?;
    }
    if let Some(operation) = operations.values().next() {
        let _ = operation;
        return Err(FlowError::InternalInvariant(
            "P2 contains an operation with no reachable P1 instruction",
        ));
    }
    check_terminal_kinds(flow)?;
    Ok(())
}

fn first_reference(instruction: &DecodedInstruction) -> Option<OperandReference> {
    instruction
        .references()
        .next()
        .map(|reference| reference.target)
}

fn type_from(instruction: &DecodedInstruction) -> Result<TypeId, FlowError> {
    match first_reference(instruction) {
        Some(OperandReference::Type(value)) => Ok(value),
        _ => Err(FlowError::InternalInvariant(
            "P1 instruction has no checked type reference",
        )),
    }
}

fn check_kind(
    _decoded: &DecodedMethod<'_>,
    instruction: &DecodedInstruction,
    operation: &super::Operation,
) -> Result<(), FlowError> {
    let opcode = instruction.opcode();
    let ok = match (&operation.kind, opcode) {
        (OpKind::Nop, 0x00) => true,
        (OpKind::Move { kind }, 0x01..=0x09) => {
            *kind
                == match opcode {
                    0x01..=0x03 => MoveKind::Word,
                    0x04..=0x06 => MoveKind::Wide,
                    _ => MoveKind::Object,
                }
        }
        (OpKind::MoveException, 0x0d) => true,
        (OpKind::Const { bits, width }, 0x12..=0x19) => {
            let expected_width = if opcode >= 0x16 {
                SlotWidth::Two
            } else {
                SlotWidth::One
            };
            *width == expected_width && *bits == literal_bits(instruction)?
        }
        (OpKind::ConstString { value }, 0x1a | 0x1b) => {
            matches!(first_reference(instruction), Some(OperandReference::String(id)) if id == *value)
        }
        (OpKind::ConstClass { type_id }, 0x1c) => type_from(instruction)? == *type_id,
        (OpKind::ConstMethodHandle { handle }, 0xfe) => {
            matches!(first_reference(instruction), Some(OperandReference::MethodHandle(id)) if id == *handle)
        }
        (OpKind::ConstMethodType { proto }, 0xff) => {
            matches!(first_reference(instruction), Some(OperandReference::Proto(id)) if id == *proto)
        }
        (OpKind::Monitor { enter }, 0x1d | 0x1e) => *enter == (opcode == 0x1d),
        (OpKind::Cast { type_id }, 0x1f) => type_from(instruction)? == *type_id,
        (OpKind::InstanceOf { type_id }, 0x20) => type_from(instruction)? == *type_id,
        (OpKind::ArrayLength, 0x21) => true,
        (OpKind::NewInstance { type_id }, 0x22) => type_from(instruction)? == *type_id,
        (OpKind::NewArray { type_id }, 0x23) => type_from(instruction)? == *type_id,
        (OpKind::FilledNewArray { type_id }, 0x24 | 0x25) => type_from(instruction)? == *type_id,
        (OpKind::FillArrayData { payload }, 0x26) => instruction.payload_target() == Some(*payload),
        (OpKind::Compare { op }, 0x2d..=0x31) => {
            *op == match opcode {
                0x2d => CompareOp::CmplFloat,
                0x2e => CompareOp::CmpgFloat,
                0x2f => CompareOp::CmplDouble,
                0x30 => CompareOp::CmpgDouble,
                _ => CompareOp::CmpLong,
            }
        }
        (OpKind::Array { access, kind }, 0x44..=0x51) => {
            *access
                == if opcode < 0x4c {
                    ArrayAccess::Get
                } else {
                    ArrayAccess::Put
                }
                && *kind == array_value_kind(opcode)
        }
        (
            OpKind::Field {
                access,
                static_field,
                field,
                kind,
            },
            0x52..=0x6d,
        ) => {
            let expected_static = opcode >= 0x60;
            let expected_get = if expected_static {
                opcode < 0x67
            } else {
                opcode < 0x59
            };
            let expected_field = match first_reference(instruction) {
                Some(OperandReference::Field(value)) => value,
                _ => {
                    return Err(FlowError::InternalInvariant(
                        "P1 field instruction has no field reference",
                    ));
                }
            };
            *access
                == if expected_get {
                    FieldAccess::Get
                } else {
                    FieldAccess::Put
                }
                && *static_field == expected_static
                && *field == expected_field
                && *kind
                    == field_value_kind(if expected_static {
                        (opcode - 0x60) % 7
                    } else {
                        (opcode - 0x52) % 7
                    })
        }
        (
            OpKind::Invoke {
                kind,
                target,
                result,
            },
            0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd,
        ) => {
            let expected_target = match (opcode, first_reference(instruction)) {
                (0xfc | 0xfd, Some(OperandReference::CallSite(value))) => {
                    OperandReference::CallSite(value)
                }
                (_, Some(OperandReference::Method(value))) => OperandReference::Method(value),
                _ => {
                    return Err(FlowError::InternalInvariant(
                        "P1 invoke has no checked target reference",
                    ));
                }
            };
            let expected_result = _decoded
                .invoke_layout_at(instruction.pc())
                .and_then(|layout| layout.result());
            *kind == invoke_kind(opcode) && *target == expected_target && *result == expected_result
        }
        (OpKind::Unary { op }, 0x7b..=0x8f) => *op == unary_op(opcode)?,
        (OpKind::Binary { op, form, literal }, 0x90..=0xaf) => {
            *op == binary_op((opcode - 0x90) % 32)?
                && *form == BinaryForm::ThreeAddress
                && literal.is_none()
        }
        (OpKind::Binary { op, form, literal }, 0xb0..=0xcf) => {
            *op == binary_op((opcode - 0xb0) % 32)?
                && *form == BinaryForm::TwoAddress
                && literal.is_none()
        }
        (OpKind::Binary { op, form, literal }, 0xd0..=0xd7) => {
            *op == lit16_op((opcode - 0xd0) % 8)?
                && *form == BinaryForm::Literal
                && *literal == Some(signed_literal(instruction)?)
        }
        (OpKind::Binary { op, form, literal }, 0xd8..=0xe2) => {
            *op == lit8_op((opcode - 0xd8) % 11)?
                && *form == BinaryForm::Literal
                && *literal == Some(signed_literal(instruction)?)
        }
        _ => false,
    };
    if !ok {
        return Err(FlowError::InternalInvariant(
            "P2 operation kind disagrees with the original opcode",
        ));
    }
    Ok(())
}

fn literal_bits(instruction: &DecodedInstruction) -> Result<u64, FlowError> {
    match instruction.operands().literal() {
        Some(RawLiteral::Bits32(value)) => Ok(u64::from(value)),
        Some(RawLiteral::Bits64(value)) => Ok(value),
        Some(RawLiteral::Signed8(value)) => Ok(u64::from(value as u32)),
        Some(RawLiteral::Signed16(value)) => Ok(u64::from(value as u32)),
        None => Err(FlowError::InternalInvariant(
            "constant instruction has no literal",
        )),
    }
}

fn signed_literal(instruction: &DecodedInstruction) -> Result<i64, FlowError> {
    match instruction.operands().literal() {
        Some(RawLiteral::Signed8(value)) => Ok(i64::from(value)),
        Some(RawLiteral::Signed16(value)) => Ok(i64::from(value)),
        Some(RawLiteral::Bits32(value)) => Ok(i64::from(value as i32)),
        Some(RawLiteral::Bits64(value)) => Ok(value as i64),
        None => Err(FlowError::InternalInvariant(
            "literal instruction has no literal",
        )),
    }
}

fn array_value_kind(opcode: u8) -> ValueKind {
    match (opcode - 0x44) % 7 {
        0 => ValueKind::Word,
        1 => ValueKind::Wide,
        2 => ValueKind::Object,
        3 => ValueKind::Boolean,
        4 => ValueKind::Byte,
        5 => ValueKind::Char,
        _ => ValueKind::Short,
    }
}

fn field_value_kind(index: u8) -> ValueKind {
    match index {
        0 => ValueKind::Word,
        1 => ValueKind::Wide,
        2 => ValueKind::Object,
        3 => ValueKind::Boolean,
        4 => ValueKind::Byte,
        5 => ValueKind::Char,
        _ => ValueKind::Short,
    }
}

fn invoke_kind(opcode: u8) -> InvokeKind {
    match opcode {
        0x6e | 0x74 => InvokeKind::Virtual,
        0x6f | 0x75 => InvokeKind::Super,
        0x70 | 0x76 => InvokeKind::Direct,
        0x71 | 0x77 => InvokeKind::Static,
        0x72 | 0x78 => InvokeKind::Interface,
        0xfa | 0xfb => InvokeKind::Polymorphic,
        _ => InvokeKind::Custom,
    }
}

fn unary_op(opcode: u8) -> Result<UnaryOp, FlowError> {
    Ok(match opcode - 0x7b {
        0 => UnaryOp::NegInt,
        1 => UnaryOp::NotInt,
        2 => UnaryOp::NegLong,
        3 => UnaryOp::NotLong,
        4 => UnaryOp::NegFloat,
        5 => UnaryOp::NegDouble,
        6 => UnaryOp::IntToLong,
        7 => UnaryOp::IntToFloat,
        8 => UnaryOp::IntToDouble,
        9 => UnaryOp::LongToInt,
        10 => UnaryOp::LongToFloat,
        11 => UnaryOp::LongToDouble,
        12 => UnaryOp::FloatToInt,
        13 => UnaryOp::FloatToLong,
        14 => UnaryOp::FloatToDouble,
        15 => UnaryOp::DoubleToInt,
        16 => UnaryOp::DoubleToLong,
        17 => UnaryOp::DoubleToFloat,
        18 => UnaryOp::IntToByte,
        19 => UnaryOp::IntToChar,
        20 => UnaryOp::IntToShort,
        _ => return Err(FlowError::InternalInvariant("unary opcode is not checked")),
    })
}

fn binary_op(index: u8) -> Result<BinaryOp, FlowError> {
    Ok(match index {
        0 => BinaryOp::AddInt,
        1 => BinaryOp::SubInt,
        2 => BinaryOp::MulInt,
        3 => BinaryOp::DivInt,
        4 => BinaryOp::RemInt,
        5 => BinaryOp::AndInt,
        6 => BinaryOp::OrInt,
        7 => BinaryOp::XorInt,
        8 => BinaryOp::ShlInt,
        9 => BinaryOp::ShrInt,
        10 => BinaryOp::UshrInt,
        11 => BinaryOp::AddLong,
        12 => BinaryOp::SubLong,
        13 => BinaryOp::MulLong,
        14 => BinaryOp::DivLong,
        15 => BinaryOp::RemLong,
        16 => BinaryOp::AndLong,
        17 => BinaryOp::OrLong,
        18 => BinaryOp::XorLong,
        19 => BinaryOp::ShlLong,
        20 => BinaryOp::ShrLong,
        21 => BinaryOp::UshrLong,
        22 => BinaryOp::AddFloat,
        23 => BinaryOp::SubFloat,
        24 => BinaryOp::MulFloat,
        25 => BinaryOp::DivFloat,
        26 => BinaryOp::RemFloat,
        27 => BinaryOp::AddDouble,
        28 => BinaryOp::SubDouble,
        29 => BinaryOp::MulDouble,
        30 => BinaryOp::DivDouble,
        31 => BinaryOp::RemDouble,
        _ => return Err(FlowError::InternalInvariant("binary opcode is not checked")),
    })
}

fn lit16_op(index: u8) -> Result<BinaryOp, FlowError> {
    Ok(match index {
        0 => BinaryOp::AddInt,
        1 => BinaryOp::RsubInt,
        2 => BinaryOp::MulInt,
        3 => BinaryOp::DivInt,
        4 => BinaryOp::RemInt,
        5 => BinaryOp::AndInt,
        6 => BinaryOp::OrInt,
        7 => BinaryOp::XorInt,
        _ => return Err(FlowError::InternalInvariant("lit16 opcode is not checked")),
    })
}

fn lit8_op(index: u8) -> Result<BinaryOp, FlowError> {
    Ok(match index {
        0 => BinaryOp::AddInt,
        1 => BinaryOp::RsubInt,
        2 => BinaryOp::MulInt,
        3 => BinaryOp::DivInt,
        4 => BinaryOp::RemInt,
        5 => BinaryOp::AndInt,
        6 => BinaryOp::OrInt,
        7 => BinaryOp::XorInt,
        8 => BinaryOp::ShlInt,
        9 => BinaryOp::ShrInt,
        10 => BinaryOp::UshrInt,
        _ => return Err(FlowError::InternalInvariant("lit8 opcode is not checked")),
    })
}

fn check_terminal_kinds(flow: &FlowMethod<'_>) -> Result<(), FlowError> {
    for block in flow.blocks() {
        let Some(pc) = block.terminal_pc else {
            continue;
        };
        let instruction = flow
            .decoded()
            .instruction_at(pc)
            .ok_or(FlowError::InternalInvariant(
                "terminal pc is not an instruction",
            ))?;
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
                let expected = switch_cases(flow.decoded(), instruction)?;
                if *cases != expected {
                    return Err(FlowError::InternalInvariant(
                        "P2 switch cases disagree with the original payload",
                    ));
                }
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

fn switch_cases(
    decoded: &DecodedMethod<'_>,
    instruction: &DecodedInstruction,
) -> Result<Vec<(i64, CodeUnitOffset)>, FlowError> {
    let payload_pc = instruction
        .payload_target()
        .ok_or(FlowError::InternalInvariant(
            "switch has no checked payload",
        ))?;
    let payload = decoded
        .payloads()
        .find(|payload| payload.pc() == payload_pc)
        .ok_or(FlowError::InternalInvariant(
            "switch payload is missing from P1",
        ))?;
    let cases = payload
        .switch_cases(decoded.source())
        .map_err(|_| FlowError::InternalInvariant("switch payload could not be read"))?
        .ok_or(FlowError::InternalInvariant(
            "switch payload is not a switch",
        ))?;
    let mut result = Vec::new();
    for case in cases {
        let target = i64::from(instruction.pc().0)
            .checked_add(i64::from(case.offset.0))
            .and_then(|value| u32::try_from(value).ok())
            .map(CodeUnitOffset)
            .ok_or(FlowError::PlatformCapacity)?;
        result.push((case.key, target));
    }
    Ok(result)
}

fn is_terminal(instruction: &DecodedInstruction) -> bool {
    matches!(
        instruction.spec().flow,
        FlowKind::Return
            | FlowKind::Throw
            | FlowKind::Goto
            | FlowKind::Conditional
            | FlowKind::PackedSwitch
            | FlowKind::SparseSwitch
    )
}

fn register(instruction: &DecodedInstruction, index: usize) -> Result<RegisterId, FlowError> {
    instruction
        .registers()
        .nth(index)
        .ok_or(FlowError::InternalInvariant(
            "P1 instruction lost a checked register",
        ))
}

fn slot(register: RegisterId, width: SlotWidth) -> RegisterSlot {
    RegisterSlot::new(register, width)
}

fn expected(
    decoded: &DecodedMethod<'_>,
    instruction: &DecodedInstruction,
    control: &mut RunContext<'_>,
) -> Result<Expected, FlowError> {
    let opcode = instruction.opcode();
    let spec = instruction.spec();
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let mut may_throw = spec.may_throw;
    match opcode {
        0x00 => {}
        0x01..=0x09 => {
            let width = match opcode {
                0x04..=0x06 => SlotWidth::Two,
                _ => SlotWidth::One,
            };
            reads.push(slot(register(instruction, 1)?, width));
            writes.push(slot(register(instruction, 0)?, width));
        }
        0x0d => {
            writes.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x12..=0x19 => {
            let width = if opcode >= 0x16 {
                SlotWidth::Two
            } else {
                SlotWidth::One
            };
            writes.push(slot(register(instruction, 0)?, width));
        }
        0x1a..=0x1c | 0x22 | 0xfe | 0xff => {
            writes.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x1d | 0x1e => {
            reads.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x1f => {
            reads.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x20 => {
            reads.push(slot(register(instruction, 1)?, SlotWidth::One));
            writes.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x21 => {
            reads.push(slot(register(instruction, 1)?, SlotWidth::One));
            writes.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x23 => {
            reads.push(slot(register(instruction, 1)?, SlotWidth::One));
            writes.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x24 | 0x25 => {
            for register in instruction.registers() {
                control.checkpoint()?;
                reads.push(slot(register, SlotWidth::One));
            }
        }
        0x26 => {
            reads.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x2d..=0x31 => {
            let width = if opcode >= 0x2f {
                SlotWidth::Two
            } else {
                SlotWidth::One
            };
            reads.push(slot(register(instruction, 1)?, width));
            reads.push(slot(register(instruction, 2)?, width));
            writes.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        0x44..=0x51 => {
            let get = opcode < 0x4c;
            let kind = array_kind(opcode);
            if get {
                reads.push(slot(register(instruction, 1)?, SlotWidth::One));
                reads.push(slot(register(instruction, 2)?, SlotWidth::One));
                writes.push(slot(register(instruction, 0)?, kind));
            } else {
                reads.push(slot(register(instruction, 0)?, kind));
                reads.push(slot(register(instruction, 1)?, SlotWidth::One));
                reads.push(slot(register(instruction, 2)?, SlotWidth::One));
            }
        }
        0x52..=0x6d => {
            let static_field = opcode >= 0x60;
            let get = if static_field {
                opcode < 0x67
            } else {
                opcode < 0x59
            };
            let kind = field_kind(if static_field {
                (opcode - 0x60) % 7
            } else {
                (opcode - 0x52) % 7
            });
            if get {
                if !static_field {
                    reads.push(slot(register(instruction, 1)?, SlotWidth::One));
                }
                writes.push(slot(register(instruction, 0)?, kind));
            } else {
                reads.push(slot(register(instruction, 0)?, kind));
                if !static_field {
                    reads.push(slot(register(instruction, 1)?, SlotWidth::One));
                }
            }
        }
        0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd => {
            let layout =
                decoded
                    .invoke_layout_at(instruction.pc())
                    .ok_or(FlowError::InternalInvariant(
                        "invoke has no checked P1 layout",
                    ))?;
            let mut registers = instruction.registers();
            if layout.has_receiver() {
                reads.push(slot(
                    registers
                        .next()
                        .ok_or(FlowError::InternalInvariant("invoke receiver missing"))?,
                    SlotWidth::One,
                ));
            }
            for width in layout.arguments() {
                let first = registers
                    .next()
                    .ok_or(FlowError::InternalInvariant("invoke argument missing"))?;
                reads.push(slot(first, *width));
                if *width == SlotWidth::Two {
                    let second = registers
                        .next()
                        .ok_or(FlowError::InternalInvariant("invoke wide word missing"))?;
                    if first.0.checked_add(1) != Some(second.0) {
                        return Err(FlowError::InternalInvariant(
                            "invoke wide argument is not adjacent",
                        ));
                    }
                }
            }
            if registers.next().is_some() {
                return Err(FlowError::InternalInvariant(
                    "invoke has unclaimed register words",
                ));
            }
            may_throw = true;
        }
        0x7b..=0x8f => {
            let (source, dest) = unary_widths(opcode)?;
            reads.push(slot(register(instruction, 1)?, source));
            writes.push(slot(register(instruction, 0)?, dest));
        }
        0x90..=0xaf => {
            let (left, right, result) = binary_widths(opcode)?;
            reads.push(slot(register(instruction, 1)?, left));
            reads.push(slot(register(instruction, 2)?, right));
            writes.push(slot(register(instruction, 0)?, result));
        }
        0xb0..=0xcf => {
            let (left, right, result) = binary_widths(opcode)?;
            reads.push(slot(register(instruction, 0)?, left));
            reads.push(slot(register(instruction, 1)?, right));
            writes.push(slot(register(instruction, 0)?, result));
        }
        0xd0..=0xe2 => {
            reads.push(slot(register(instruction, 1)?, SlotWidth::One));
            writes.push(slot(register(instruction, 0)?, SlotWidth::One));
        }
        _ => {
            return Err(FlowError::InternalInvariant(
                "P1 instruction has no independent P2 trace rule",
            ));
        }
    }
    if let Some(result_pc) = instruction.result_pc() {
        let result = decoded
            .instruction_at(result_pc)
            .ok_or(FlowError::InternalInvariant(
                "producer result pc is not an instruction",
            ))?;
        let width = if result.opcode() == 0x0b {
            SlotWidth::Two
        } else {
            SlotWidth::One
        };
        writes = vec![slot(register(result, 0)?, width)];
    }
    let commit = if instruction.result_pc().is_some() {
        CommitRule::AfterResult
    } else if writes.is_empty() {
        CommitRule::None
    } else {
        CommitRule::AtOperation
    };
    Ok(Expected {
        reads,
        writes,
        may_throw,
        commit,
    })
}

fn array_kind(opcode: u8) -> SlotWidth {
    match (opcode - 0x44) % 7 {
        1 | 2 => match (opcode - 0x44) % 7 {
            1 => SlotWidth::Two,
            _ => SlotWidth::One,
        },
        _ => SlotWidth::One,
    }
}

fn field_kind(index: u8) -> SlotWidth {
    if index == 1 {
        SlotWidth::Two
    } else {
        SlotWidth::One
    }
}

fn unary_widths(opcode: u8) -> Result<(SlotWidth, SlotWidth), FlowError> {
    let index = opcode - 0x7b;
    let pair = match index {
        0 | 1 | 4 => (SlotWidth::One, SlotWidth::One),
        2 | 3 | 5 => (SlotWidth::Two, SlotWidth::Two),
        6 => (SlotWidth::One, SlotWidth::Two),
        7 => (SlotWidth::One, SlotWidth::One),
        8 => (SlotWidth::One, SlotWidth::Two),
        9 => (SlotWidth::Two, SlotWidth::One),
        10 => (SlotWidth::Two, SlotWidth::One),
        11 => (SlotWidth::Two, SlotWidth::Two),
        12 => (SlotWidth::One, SlotWidth::One),
        13 | 14 => (SlotWidth::One, SlotWidth::Two),
        15 | 17 => (SlotWidth::Two, SlotWidth::One),
        16 => (SlotWidth::Two, SlotWidth::Two),
        18..=20 => (SlotWidth::One, SlotWidth::One),
        _ => return Err(FlowError::InternalInvariant("unary opcode is not checked")),
    };
    Ok(pair)
}

fn binary_widths(opcode: u8) -> Result<(SlotWidth, SlotWidth, SlotWidth), FlowError> {
    let index = if (0x90..=0xaf).contains(&opcode) {
        (opcode - 0x90) % 32
    } else if (0xb0..=0xcf).contains(&opcode) {
        (opcode - 0xb0) % 32
    } else {
        return Ok((SlotWidth::One, SlotWidth::One, SlotWidth::One));
    };
    let (left, right, result) = match index {
        0..=10 => (SlotWidth::One, SlotWidth::One, SlotWidth::One),
        11..=21 => {
            let right = if index >= 19 {
                SlotWidth::One
            } else {
                SlotWidth::Two
            };
            (SlotWidth::Two, right, SlotWidth::Two)
        }
        22..=26 => (SlotWidth::One, SlotWidth::One, SlotWidth::One),
        27..=31 => (SlotWidth::Two, SlotWidth::Two, SlotWidth::Two),
        _ => return Err(FlowError::InternalInvariant("binary opcode is not checked")),
    };
    Ok((left, right, result))
}
