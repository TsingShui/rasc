use super::branch::{LocalId, ValueUse};
use crate::p2_flow::Op;
use crate::p3_ssa::{BoundOp, ValueId};
use crate::p4_types::{ArrayUse, Kind, TypedBody};
use crate::{RequestError, RunContext};

pub(crate) enum Statement {
    Length {
        array: LocalId,
        result: LocalId,
    },
    Load {
        array: LocalId,
        index: ValueUse,
        result: LocalId,
    },
    Store {
        array: ValueUse,
        index: ValueUse,
        value: ValueUse,
    },
}
pub(super) fn validate(
    input: &crate::input::MethodSource<'_, '_>,
    _catalog: &crate::context::Context,
    typed: &TypedBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for evidence in &typed.arrays {
        budget.checkpoint()?;
        if matches!(
            evidence,
            Some(crate::p4_types::ArrayEvidence {
                usage: ArrayUse::Store {
                    receiver: crate::p4_types::ArrayReceiver::ObjectView,
                    ..
                },
                ..
            })
        ) {
            return Ok(!input.qualifier_shadowed(&[106, 97, 118, 97], budget)?);
        }
    }
    Ok(true)
}
pub(crate) fn lower(ssa: &TypedBody<'_>, op: &BoundOp) -> Result<Statement, RequestError> {
    let evidence = ssa
        .arrays
        .get(usize::try_from(op.id.0).map_err(|_| RequestError::PlatformCapacity)?)
        .and_then(Option::as_ref)
        .ok_or(RequestError::InternalInvariant(
            "array operation lost type evidence",
        ))?;
    let array = LocalId(op.input(0)?.0);
    let result = || {
        op.result
            .map(|value| LocalId(value.0))
            .ok_or(RequestError::InternalInvariant(
                "array read lost its result",
            ))
    };
    match (op.op, &evidence.usage) {
        (Op::ArrayLength(..), ArrayUse::Length) => Ok(Statement::Length {
            array,
            result: result()?,
        }),
        (Op::ArrayLoad(..), ArrayUse::Load { index }) => Ok(Statement::Load {
            array,
            index: super::values::lower(LocalId(op.input(1)?.0), *index),
            result: result()?,
        }),
        (
            Op::ArrayStore(..),
            ArrayUse::Store {
                index,
                value,
                receiver,
            },
        ) => Ok(Statement::Store {
            array: match receiver {
                crate::p4_types::ArrayReceiver::Direct => ValueUse::Direct(array),
                crate::p4_types::ArrayReceiver::ObjectView => ValueUse::ObjectArrayView(array),
            },
            index: super::values::lower(LocalId(op.input(1)?.0), *index),
            value: super::values::lower(LocalId(op.input(2)?.0), *value),
        }),
        _ => Err(RequestError::InternalInvariant(
            "array type evidence changed operation kind",
        )),
    }
}
fn valid_view(
    ssa: &TypedBody<'_>,
    actual: ValueUse,
    value: ValueId,
    expected: Kind,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    if actual.value().0 != value.0 {
        return Ok(false);
    }
    if matches!(
        expected,
        Kind::Int | Kind::Float | Kind::Long | Kind::Double
    ) {
        return super::dispatcher_verify::numeric_view(ssa, actual, Some(&value), expected);
    }
    if expected == Kind::Object {
        return super::values::verify_reference_store(
            ssa,
            actual,
            value,
            crate::p2_flow::DeclarationType::Object,
            budget,
        );
    }
    Ok(false)
}
pub(crate) fn verify(
    input: &crate::input::MethodSource<'_, '_>,
    ssa: &TypedBody<'_>,
    op: &BoundOp,
    actual: &Statement,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let original =
        crate::raw::instruction_layout(ssa.ssa.raw.instructions(), op.pc, ssa.ssa.raw.version())
            .map_err(crate::input::MethodError::Decode)?;
    let (opcode, registers) = match op.op {
        Op::ArrayLength(dst, array) => (0x21, [Some(dst), Some(array), None]),
        Op::ArrayLoad(dst, array, index, element) => (
            0x44 + element.offset(),
            [Some(dst), Some(array), Some(index)],
        ),
        Op::ArrayStore(array, index, value, element) => (
            0x4b + element.offset(),
            [Some(value), Some(array), Some(index)],
        ),
        _ => return Ok(false),
    };
    if !matches!(original.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == opcode)
    {
        return Ok(false);
    }
    let operands =
        crate::input::operands::instruction_operands(ssa.ssa.raw.instructions(), op.pc, original)
            .map_err(crate::input::MethodError::Decode)?;
    let mut original_registers = operands.registers().iter();
    for register in registers.into_iter().flatten() {
        budget.checkpoint()?;
        if original_registers.next() != Some(register) {
            return Ok(false);
        }
    }
    if original_registers.next().is_some() {
        return Ok(false);
    }
    let array = op.input(0)?;
    let component =
        match ssa.kinds[usize::try_from(array.0).map_err(|_| RequestError::PlatformCapacity)?] {
            Some(Kind::PrimitiveArray(kind)) => kind.value_type(),
            Some(Kind::ObjectArray) => crate::p2_flow::DeclarationType::Object,
            Some(Kind::ReferenceArray(key)) => {
                crate::p2_flow::ArrayKind::Reference(key).element_type(input, budget)?
            }
            _ => return Ok(false),
        };
    let result_matches = |result: LocalId, kind: Kind| -> Result<bool, RequestError> {
        Ok(op.result.is_some_and(|id| id.0 == result.0)
            && ssa.kinds[usize::try_from(result.0).map_err(|_| RequestError::PlatformCapacity)?]
                == Some(kind))
    };
    Ok(match (op.op, actual) {
        (
            Op::ArrayLength(..),
            Statement::Length {
                array: actual,
                result,
            },
        ) => actual.0 == array.0 && result_matches(*result, Kind::Int)?,
        (
            Op::ArrayLoad(_, _, _, element),
            Statement::Load {
                array: actual,
                index,
                result,
            },
        ) => {
            actual.0 == array.0
                && element.accepts(component)
                && result_matches(*result, component.kind())?
                && valid_view(ssa, *index, op.input(1)?, Kind::Int, budget)?
        }
        (
            Op::ArrayStore(_, _, _, element),
            Statement::Store {
                array: actual,
                index,
                value,
            },
        ) => {
            op.result.is_none()
                && actual.value().0 == array.0
                && matches!(
                    (
                        *actual,
                        ssa.kinds[usize::try_from(array.0)
                            .map_err(|_| RequestError::PlatformCapacity)?]
                    ),
                    (ValueUse::ObjectArrayView(_), Some(Kind::ReferenceArray(_)))
                        | (
                            ValueUse::Direct(_),
                            Some(Kind::PrimitiveArray(_) | Kind::ObjectArray)
                        )
                )
                && element.accepts(component)
                && valid_view(ssa, *index, op.input(1)?, Kind::Int, budget)?
                && if matches!(actual, ValueUse::ObjectArrayView(_)) {
                    valid_view(ssa, *value, op.input(2)?, Kind::Object, budget)?
                } else if component.kind() == Kind::Object {
                    valid_view(ssa, *value, op.input(2)?, component.kind(), budget)?
                } else {
                    super::values::verify_primitive_store(ssa, *value, op.input(2)?, component)?
                }
        }
        _ => false,
    })
}
