use super::branch::{LocalId, ValueUse};
use crate::p2_flow::normal::Op;
use crate::p3_ssa::normal::BoundOp;
use crate::p4_calls::field_facts::{FieldStorage, FieldUse, IntFieldAccess};
use crate::p4_calls::normal::FieldDeclaration;
use crate::p5_types::normal::{FieldUse as TypedFieldUse, Kind, TypedBody};
use crate::{FieldId, MethodBudget, RequestError};

pub(crate) enum Statement {
    Load {
        receiver: LocalId,
        name: String,
        reference: FieldId,
        flags: u32,
        result: LocalId,
    },
    Store {
        receiver: LocalId,
        name: String,
        reference: FieldId,
        flags: u32,
        value: ValueUse,
    },
}
fn declaration<'a>(
    typed: &'a TypedBody<'_>,
    op: &BoundOp,
) -> Result<&'a IntFieldAccess, RequestError> {
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
        FieldDeclaration::Constant => Err(RequestError::JavaConstantFieldRead {
            reference: evidence.reference,
            pc: op.pc,
        }),
    }
}
pub(crate) fn lower(
    typed: &TypedBody<'_>,
    op: &BoundOp,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Statement>, RequestError> {
    let declaration = declaration(typed, op)?;
    let Some(name) = super::calls::identifier_text(&declaration.name, budget)? else {
        return Ok(None);
    };
    let receiver = LocalId(op.input(0)?.0);
    let index = usize::try_from(op.id.0).map_err(|_| RequestError::PlatformCapacity)?;
    let usage = typed.field_uses.get(index).and_then(Option::as_ref).ok_or(
        RequestError::InternalInvariant("field lost final use conversions"),
    )?;
    Ok(Some(match (op.op, usage) {
        (Op::FieldLoad(..), TypedFieldUse::Read) => Statement::Load {
            receiver,
            name,
            reference: declaration.reference,
            flags: declaration.flags,
            result: LocalId(
                op.result
                    .ok_or(RequestError::InternalInvariant("field read lost result"))?
                    .0,
            ),
        },
        (Op::FieldStore(..), TypedFieldUse::Write(conversion)) => Statement::Store {
            receiver,
            name,
            reference: declaration.reference,
            flags: declaration.flags,
            value: super::values::lower(LocalId(op.input(1)?.0), *conversion),
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
    op: &BoundOp,
    actual: &Statement,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let declaration = declaration(typed, op)?;
    let (opcode, registers, original_reference, usage) = match op.op {
        Op::FieldLoad(dst, receiver, reference) => {
            (0x52, [dst, receiver], reference, FieldUse::Read)
        }
        Op::FieldStore(receiver, value, reference) => {
            (0x59, [value, receiver], reference, FieldUse::Write)
        }
        _ => return Ok(false),
    };
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
    for register in registers {
        budget.charge_work(1)?;
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
        } => (*receiver, name, *reference, *flags),
    };
    if reference != original_reference
        || reference != declaration.reference
        || flags != declaration.flags
        || declaration.storage != FieldStorage::Instance
        || declaration.usage != usage
        || receiver.0 != op.input(0)?.0
        || typed.kinds[usize::try_from(receiver.0).map_err(|_| RequestError::PlatformCapacity)?]
            != Some(Kind::Nominal(declaration.owner_type))
    {
        return Ok(false);
    }
    let mut units = name.encode_utf16();
    for &expected in &declaration.name {
        budget.charge_work(1)?;
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
                    == Some(Kind::Int)
        }
        Statement::Store { value, .. } => {
            let source = op.input(1)?;
            usage == FieldUse::Write
                && op.result.is_none()
                && value.value().0 == source.0
                && matches!(
                    (
                        *value,
                        typed.kinds[usize::try_from(source.0)
                            .map_err(|_| RequestError::PlatformCapacity)?]
                    ),
                    (ValueUse::Direct(_), Some(Kind::Int))
                        | (ValueUse::BooleanWord(_), Some(Kind::Boolean))
                )
        }
    })
}
