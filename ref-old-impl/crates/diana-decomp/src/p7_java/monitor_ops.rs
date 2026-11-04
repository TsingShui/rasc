use super::branch::{LocalId, ValueUse};
use super::calls::Statement;
use crate::p2_flow::{MonitorKind, Op, OpId};
use crate::p3_ssa::BoundOp;
use crate::p4_types::TypedBody;
use crate::p4_types::reference_roots::Root;
use crate::{RequestError, RunContext};
pub(super) fn lower(
    typed: &TypedBody<'_>,
    id: OpId,
    op: &BoundOp,
) -> Result<Statement, RequestError> {
    let Op::Monitor(kind, _) = op.op else {
        return Err(RequestError::InternalInvariant(
            "monitor lowering changed operation",
        ));
    };
    let value = op.input(0)?;
    let lock = if typed.roots.one(value)? == Some(Root::Null) {
        ValueUse::NullAssignment(LocalId(value.0))
    } else {
        ValueUse::Direct(LocalId(value.0))
    };
    Ok(Statement::Monitor {
        operation: id,
        kind,
        lock,
    })
}
pub(super) fn verify(
    typed: &TypedBody<'_>,
    id: OpId,
    op: &BoundOp,
    actual: &Statement,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
    let Statement::Monitor {
        operation,
        kind,
        lock,
    } = actual
    else {
        return Ok(false);
    };
    let Op::Monitor(original, register) = op.op else {
        return Ok(false);
    };
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
    let value = op.input(0)?;
    let view = match *lock {
        ValueUse::Direct(local) => {
            local.0 == value.0
                && typed.kinds
                    [usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
                .is_some_and(crate::p4_types::reference_roots::reference)
        }
        ValueUse::NullAssignment(local) => {
            local.0 == value.0 && typed.roots.one(value)? == Some(Root::Null)
        }
        _ => false,
    };
    Ok(*operation == id
        && *kind == original
        && op.result.is_none()
        && registers.next() == Some(register)
        && registers.next().is_none()
        && matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode==if original==MonitorKind::Enter{0x1d}else{0x1e})
        && view)
}
