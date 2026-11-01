use crate::p2_flow::normal::Op;
use crate::p3_ssa::normal::BoundOp;
use crate::p5_types::normal::{Kind, TypedBody};
use crate::{MethodBudget, RequestError};

pub(crate) struct Statement {
    pub target: super::branch::LocalId,
    pub reference: crate::TypeId,
    pub kind: crate::p5_types::BuiltinReference,
    pub source: super::branch::ValueUse,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn validate(
    input: &crate::input::MethodInput<'_, '_>,
    catalog: &crate::DeclarationCatalog,
    typed: &TypedBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    if typed.ssa.casts.is_empty() {
        return Ok(true);
    }
    if !crate::p4_calls::qualifier_available(input, catalog, &[106, 97, 118, 97], budget)? {
        return Ok(false);
    }
    for site in &typed.ssa.casts {
        budget.charge_work(1)?;
        if !input
            .type_name(site.reference, budget)?
            .matches_with(site.kind.descriptor(), budget)?
        {
            return Err(RequestError::InternalInvariant(
                "cast target changed original type",
            ));
        }
    }
    Ok(true)
}
pub(super) fn lower(typed: &TypedBody<'_>, op: &BoundOp) -> Result<Statement, RequestError> {
    let Op::Cast(_, id) = op.op else {
        return Err(RequestError::InternalInvariant(
            "non-cast entered cast lowering",
        ));
    };
    let site = typed
        .ssa
        .casts
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant("cast lost target"))?;
    let usage = typed
        .cast_uses
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant("cast lost final use"))?;
    Ok(Statement {
        target: super::branch::LocalId(
            op.result
                .ok_or(RequestError::InternalInvariant("cast lost result"))?
                .0,
        ),
        reference: site.reference,
        kind: site.kind,
        source: super::values::lower(super::branch::LocalId(usage.value.0), usage.conversion),
    })
}
pub(super) fn verify(
    typed: &TypedBody<'_>,
    op: &BoundOp,
    actual: &Statement,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    budget.charge_work(1)?;
    let Op::Cast(register, id) = op.op else {
        return Ok(false);
    };
    let site = typed
        .ssa
        .casts
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "cast certificate lost target",
        ))?;
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
    let source = op.input(0)?;
    let expected_kind = match site.kind {
        crate::p5_types::BuiltinReference::Object => Kind::Object,
        crate::p5_types::BuiltinReference::String => Kind::String,
    };
    if !matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x1f)
        || !matches!(layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Type(site.reference))
        || registers.next() != Some(register)
        || registers.next().is_some()
        || actual.reference != site.reference
        || actual.kind != site.kind
        || op.result.map(|value| value.0) != Some(actual.target.0)
        || typed.kinds[ix(actual.target.0)?] != Some(expected_kind)
        || actual.source.value().0 != source.0
        || op.inputs.len() != 1
    {
        return Ok(false);
    }
    Ok(match (actual.source, typed.kinds[ix(source.0)?]) {
        (super::branch::ValueUse::Direct(_), Some(Kind::Object | Kind::String)) => true,
        (
            super::branch::ValueUse::UpcastObject(_),
            Some(Kind::Nominal(_) | Kind::PrimitiveArray(_) | Kind::ObjectArray),
        ) => true,
        (
            super::branch::ValueUse::Null {
                kind: crate::p5_types::BuiltinReference::Object,
                ..
            },
            _,
        ) => typed.ssa.is_zero(source)?,
        _ => false,
    })
}
