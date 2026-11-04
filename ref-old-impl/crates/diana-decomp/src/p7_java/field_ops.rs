use super::branch::{LocalId, ValueUse};
use crate::p2_flow::Op;
use crate::p3_ssa::BoundOp;
use crate::p4_types::bindings::FieldDeclaration;
use crate::p4_types::declarations::field_facts::{FieldAccess, FieldStorage, FieldUse};
use crate::p4_types::{FieldUse as TypedFieldUse, Kind, TypedBody};
use crate::{FieldId, RequestError, RunContext};

pub(crate) enum Receiver {
    Instance(LocalId),
    This(LocalId),
    Static(String),
    OwnStatic(String),
    Outer(String, LocalId),
}
pub(crate) enum Statement {
    Load {
        receiver: Receiver,
        name: String,
        reference: FieldId,
        flags: Option<u32>,
        result: LocalId,
    },
    Store {
        receiver: Receiver,
        name: String,
        reference: FieldId,
        flags: Option<u32>,
        value: ValueUse,
    },
}
fn entry_this(
    typed: &TypedBody<'_>,
    value: crate::p3_ssa::ValueId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    use crate::p3_ssa::{Definition, Incoming, Sym};
    if matches!(typed.ssa.frame, crate::p2_flow::MethodFrame::Static) {
        return Ok(false);
    }
    budget.checkpoint()?;
    let definition = typed
        .ssa
        .values
        .get(usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?);
    match definition {
        Some(Definition::Parameter(0)) => Ok(true),
        Some(Definition::Phi) => {
            for phi in &typed.ssa.phis {
                budget.checkpoint()?;
                if phi.result == value {
                    if let [(Incoming::EntrySeed, Sym::Value(seed))] = phi.inputs.as_slice() {
                        return Ok(matches!(
                            typed.ssa.values.get(
                                usize::try_from(seed.0)
                                    .map_err(|_| RequestError::PlatformCapacity)?
                            ),
                            Some(Definition::Parameter(0))
                        ));
                    }
                    return Ok(false);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
fn qualifier(owner: &[u16], budget: &mut RunContext<'_>) -> Result<Option<String>, RequestError> {
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    let mut text = crate::substrate::owned::TextBuffer::new();
    for (index, segment) in owner[1..owner.len() - 1]
        .split(|&unit| unit == 47)
        .enumerate()
    {
        budget.checkpoint()?;
        let Some(segment) = super::calls::identifier_text(segment, budget)? else {
            return Ok(None);
        };
        if index != 0 {
            text.push(".", budget)?;
        }
        text.push(&segment, budget)?;
    }
    Ok(Some(text.finish()))
}
fn declaration<'a>(
    typed: &'a TypedBody<'_>,
    op: &BoundOp,
) -> Result<&'a FieldAccess, RequestError> {
    let index = usize::try_from(op.id.0).map_err(|_| RequestError::PlatformCapacity)?;
    let evidence = typed
        .field_declarations
        .get(index)
        .and_then(Option::as_ref)
        .ok_or(RequestError::InternalInvariant(
            "field lost declaration evidence",
        ))?;
    match &evidence.declaration {
        FieldDeclaration::Known(declaration) => Ok(declaration),
        FieldDeclaration::Missing => Err(RequestError::FieldBindingEvidenceMissing {
            reference: evidence.reference,
            pc: op.pc,
        }),
    }
}
pub(crate) fn lower(
    typed: &TypedBody<'_>,
    names: &super::types::TypeNames,
    op: &BoundOp,
    budget: &mut RunContext<'_>,
) -> Result<Option<Statement>, RequestError> {
    let declaration = declaration(typed, op)?;
    let Some(name) = super::calls::identifier_text(&declaration.name, budget)? else {
        return Ok(None);
    };
    let receiver = match declaration.storage {
        FieldStorage::Instance => {
            let local = LocalId(op.input(0)?.0);
            if declaration.assignment
                == crate::p4_types::declarations::field_facts::AssignmentObligation::OwnFinalInitialization
            {
                Receiver::This(local)
            } else {
                Receiver::Instance(local)
            }
        }
        FieldStorage::Static => {
            let Some(name) = qualifier(&declaration.owner, budget)? else {
                return Ok(None);
            };
            if declaration.assignment
                == crate::p4_types::declarations::field_facts::AssignmentObligation::OwnFinalInitialization
            {
                if declaration.name.first() == Some(&118)
                    && declaration.name.len() > 1
                    && declaration.name[1..]
                        .iter()
                        .all(|unit| (48..=57).contains(unit))
                {
                    return Ok(None);
                }
                Receiver::OwnStatic(name)
            } else {
                Receiver::Static(name)
            }
        }
    };
    let index = usize::try_from(op.id.0).map_err(|_| RequestError::PlatformCapacity)?;
    let usage = typed.field_uses.get(index).and_then(Option::as_ref).ok_or(
        RequestError::InternalInvariant("field lost final use conversions"),
    )?;
    Ok(Some(match (op.op, usage) {
        (Op::FieldLoad(..) | Op::StaticLoad(..), TypedFieldUse::Read) => Statement::Load {
            receiver: if declaration.outer_capture.is_some() {
                if !entry_this(typed, op.input(0)?, budget)? {
                    return Ok(None);
                }
                let mut name = crate::substrate::owned::TextBuffer::new();
                name.push(names.name(declaration.kind.kind(), budget)?, budget)?;
                Receiver::Outer(name.finish(), LocalId(op.input(0)?.0))
            } else {
                receiver
            },
            name,
            reference: declaration.reference,
            flags: declaration.flags,
            result: LocalId(
                op.result
                    .ok_or(RequestError::InternalInvariant("field read lost result"))?
                    .0,
            ),
        },
        (
            Op::FieldStore(..) | Op::StaticStore(..),
            TypedFieldUse::Write(conversion) | TypedFieldUse::WriteThis(conversion),
        ) => Statement::Store {
            receiver,
            name,
            reference: declaration.reference,
            flags: declaration.flags,
            value: super::values::lower(
                LocalId(
                    op.input(usize::from(declaration.storage == FieldStorage::Instance))?
                        .0,
                ),
                *conversion,
            ),
        },
        _ => {
            return Err(RequestError::InternalInvariant(
                "field use changed operation kind",
            ));
        }
    }))
}
pub(crate) fn verify(
    typed: &TypedBody<'_>,
    names: &super::types::TypeNames,
    op: &BoundOp,
    actual: &Statement,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let declaration = declaration(typed, op)?;
    let offset = declaration.kind.field_offset();
    let (opcode, registers, original_reference, usage, kind) = match op.op {
        Op::FieldLoad(dst, receiver, reference, kind) => (
            0x52 + offset,
            [Some(dst), Some(receiver)],
            reference,
            FieldUse::Read,
            kind,
        ),
        Op::FieldStore(receiver, value, reference, kind) => (
            0x59 + offset,
            [Some(value), Some(receiver)],
            reference,
            FieldUse::Write,
            kind,
        ),
        Op::StaticLoad(dst, reference, kind) => (
            0x60 + offset,
            [Some(dst), None],
            reference,
            FieldUse::Read,
            kind,
        ),
        Op::StaticStore(value, reference, kind) => (
            0x67 + offset,
            [Some(value), None],
            reference,
            FieldUse::Write,
            kind,
        ),
        _ => return Ok(false),
    };
    if kind != declaration.kind {
        return Ok(false);
    }
    let original = crate::raw::instruction_layout(
        typed.ssa.raw.instructions(),
        op.pc,
        typed.ssa.raw.version(),
    )
    .map_err(crate::input::MethodError::Decode)?;
    if !matches!(original.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == opcode)
        || !matches!(original.references()[0], Some(reference) if reference.target == crate::raw::OperandReference::Field(original_reference))
    {
        return Ok(false);
    }
    let operands =
        crate::input::operands::instruction_operands(typed.ssa.raw.instructions(), op.pc, original)
            .map_err(crate::input::MethodError::Decode)?;
    let mut words = operands.registers().iter();
    for register in registers.into_iter().flatten() {
        budget.checkpoint()?;
        if words.next() != Some(register) {
            return Ok(false);
        }
    }
    if words.next().is_some() {
        return Ok(false);
    }
    let (receiver, name, reference, flags) = match actual {
        Statement::Load {
            receiver,
            name,
            reference,
            flags,
            ..
        }
        | Statement::Store {
            receiver,
            name,
            reference,
            flags,
            ..
        } => (receiver, name, *reference, *flags),
    };
    if reference != original_reference
        || reference != declaration.reference
        || flags != declaration.flags
        || declaration.usage != usage
    {
        return Ok(false);
    }
    match (receiver, declaration.storage, op.op) {
        (Receiver::Outer(name, source), FieldStorage::Instance, Op::FieldLoad(..)) => {
            if declaration.outer_capture.is_none() {
                return Ok(false);
            }
            if names.name(declaration.kind.kind(), budget)? != name
                || source.0 != op.input(0)?.0
                || !entry_this(typed, op.input(0)?, budget)?
            {
                return Ok(false);
            }
        }
        (
            Receiver::Instance(receiver) | Receiver::This(receiver),
            FieldStorage::Instance,
            Op::FieldLoad(..) | Op::FieldStore(..),
        ) => {
            let final_write = declaration.assignment
                == crate::p4_types::declarations::field_facts::AssignmentObligation::OwnFinalInitialization;
            if matches!(
                actual,
                Statement::Store {
                    receiver: Receiver::This(_),
                    ..
                }
            ) != final_write
                || final_write
                    && !matches!(
                        typed
                            .field_uses
                            .get(
                                usize::try_from(op.id.0)
                                    .map_err(|_| RequestError::PlatformCapacity)?
                            )
                            .and_then(Option::as_ref),
                        Some(TypedFieldUse::WriteThis(_))
                    )
                || receiver.0 != op.input(0)?.0
                || typed.kinds
                    [usize::try_from(receiver.0).map_err(|_| RequestError::PlatformCapacity)?]
                    != Some(Kind::Nominal(declaration.owner_type))
            {
                return Ok(false);
            }
        }
        (
            Receiver::Static(actual) | Receiver::OwnStatic(actual),
            FieldStorage::Static,
            Op::StaticLoad(..) | Op::StaticStore(..),
        ) => {
            let final_write = declaration.assignment
                == crate::p4_types::declarations::field_facts::AssignmentObligation::OwnFinalInitialization;
            if matches!(receiver, Receiver::OwnStatic(_)) != final_write
                || qualifier(&declaration.owner, budget)?.as_ref() != Some(actual)
            {
                return Ok(false);
            }
        }
        _ => return Ok(false),
    }
    let mut units = name.encode_utf16();
    for &expected in &declaration.name {
        budget.checkpoint()?;
        if units.next() != Some(expected) {
            return Ok(false);
        }
    }
    if units.next().is_some() {
        return Ok(false);
    }
    Ok(match actual {
        Statement::Load { result, .. } => {
            usage == FieldUse::Read
                && op.result.is_some_and(|value| value.0 == result.0)
                && typed.kinds
                    [usize::try_from(result.0).map_err(|_| RequestError::PlatformCapacity)?]
                    == Some(kind.kind())
        }
        Statement::Store { value, .. } => {
            let source = op.input(usize::from(declaration.storage == FieldStorage::Instance))?;
            usage == FieldUse::Write
                && op.result.is_none()
                && if kind.field_offset() == 2 {
                    super::values::verify_reference_store(typed, *value, source, kind, budget)?
                } else {
                    super::values::verify_primitive_store(typed, *value, source, kind)?
                }
        }
    })
}
