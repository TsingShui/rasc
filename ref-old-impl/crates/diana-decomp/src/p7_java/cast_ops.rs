use crate::p2_flow::{DeclarationType, Op};
use crate::p3_ssa::BoundOp;
use crate::p4_types::{Kind, TypedBody};
use crate::{RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CheckKind {
    Cast,
    InstanceOf,
}
pub(crate) struct Statement {
    pub target: super::branch::LocalId,
    pub reference: crate::TypeId,
    pub kind: DeclarationType,
    pub operation: CheckKind,
    pub type_name: String,
    pub source: super::branch::ValueUse,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn target(
    typed: &TypedBody<'_>,
    op: &BoundOp,
) -> Result<Option<(crate::TypeId, DeclarationType, CheckKind)>, RequestError> {
    Ok(match op.op {
        Op::Cast(_, id) => {
            let site = typed
                .ssa
                .casts
                .get(ix(id.0)?)
                .ok_or(RequestError::InternalInvariant("cast lost target"))?;
            Some((site.reference, site.kind, CheckKind::Cast))
        }
        Op::InstanceOf(_, _, reference, kind) => Some((reference, kind, CheckKind::InstanceOf)),
        _ => None,
    })
}
pub(super) fn validate(
    input: &crate::input::MethodSource<'_, '_>,
    _catalog: &crate::context::Context,
    typed: &TypedBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let mut checked_qualifier = false;
    for op in &typed.ssa.operations {
        budget.checkpoint()?;
        let Some((reference, kind, _)) = target(typed, op)? else {
            continue;
        };
        if !checked_qualifier {
            if input.qualifier_shadowed(&[106, 97, 118, 97], budget)? {
                return Ok(false);
            }
            checked_qualifier = true;
        }
        if crate::p2_flow::reference_type(input, reference, budget)? != Some(kind) {
            return Err(RequestError::InternalInvariant(
                "type check changed original target",
            ));
        }
    }
    Ok(true)
}
pub(super) fn lower(
    typed: &TypedBody<'_>,
    op: &BoundOp,
    names: &super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<Statement, RequestError> {
    let (reference, kind, operation) = target(typed, op)?.ok_or(
        RequestError::InternalInvariant("non-check entered type lowering"),
    )?;
    let usage = typed
        .cast_uses
        .get(ix(op.id.0)?)
        .and_then(Option::as_ref)
        .ok_or(RequestError::InternalInvariant(
            "type check lost final input view",
        ))?;
    let mut name = crate::substrate::owned::TextBuffer::new();
    name.push(names.declaration(kind, budget)?, budget)?;
    Ok(Statement {
        target: super::branch::LocalId(
            op.result
                .ok_or(RequestError::InternalInvariant("type check lost result"))?
                .0,
        ),
        reference,
        kind,
        operation,
        type_name: name.finish(),
        source: super::values::lower(super::branch::LocalId(usage.value.0), usage.conversion),
    })
}
pub(super) fn verify(
    typed: &TypedBody<'_>,
    op: &BoundOp,
    actual: &Statement,
    names: &super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
    let Some((reference, kind, operation)) = target(typed, op)? else {
        return Ok(false);
    };
    let (opcode, registers) = match op.op {
        Op::Cast(register, _) => (0x1f, [Some(register), None]),
        Op::InstanceOf(dst, source, _, _) => (0x20, [Some(dst), Some(source)]),
        _ => return Ok(false),
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
    let mut words = operands.registers().iter();
    for register in registers.into_iter().flatten() {
        budget.checkpoint()?;
        if words.next() != Some(register) {
            return Ok(false);
        }
    }
    let source = op.input(0)?;
    let expected_kind = if operation == CheckKind::Cast {
        kind.kind()
    } else {
        Kind::Boolean
    };
    if words.next().is_some()
        || !matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == opcode)
        || !matches!(layout.references()[0],Some(item) if item.target == crate::raw::OperandReference::Type(reference))
        || actual.reference != reference
        || actual.kind != kind
        || actual.operation != operation
        || actual.type_name != names.declaration(kind, budget)?
        || op.result.map(|value| value.0) != Some(actual.target.0)
        || typed.kinds[ix(actual.target.0)?] != Some(expected_kind)
        || actual.source.value().0 != source.0
        || op.inputs.len() != 1
    {
        return Ok(false);
    }
    super::values::verify_reference_store(
        typed,
        actual.source,
        source,
        crate::p2_flow::DeclarationType::Object,
        budget,
    )
}
