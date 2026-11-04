use crate::input::MethodSource;
use crate::p2_flow::{BlockId, EdgeId, Op};
use crate::p4_types::TypedBody;
use crate::p5_variables::StorageBody;
use crate::substrate::owned::reserve;
use crate::{JavaMethod, RequestError, RunContext};

pub(crate) use super::local_declarations::Local;
pub(crate) use crate::p5_variables::LocalId;
pub(crate) enum Right {
    Local(ValueUse),
    Literal(i32),
}
pub(crate) enum Expression {
    CopyLiteral(crate::p5_variables::CopySource),
    Unary(super::numeric_expression::Unary, ValueUse),
    ReverseSubtract(i32, ValueUse),
    Compare(super::numeric_expression::Expression),
    Binary(ValueUse, crate::substrate::operators::Arithmetic, Right),
    Local(ValueUse),
    Constant(i32),
    ConstantWide(i64),
}
pub(crate) struct Assignment {
    pub target: LocalId,
    pub expression: Expression,
}
pub(crate) use super::values::ValueUse;
pub(super) fn returned(
    ssa: &TypedBody<'_>,
    value: LocalId,
) -> Result<Option<ValueUse>, RequestError> {
    let conversion = ssa
        .returned_uses
        .get(ix(value.0)?)
        .ok_or(RequestError::InternalInvariant(
            "return use has foreign value",
        ))?;
    Ok(conversion.map(|conversion| super::values::lower(value, conversion)))
}

fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn append<T>(items: &mut Vec<T>, item: T, budget: &mut RunContext<'_>) -> Result<(), RequestError> {
    reserve(items, 1, budget)?;
    items.push(item);
    Ok(())
}
pub(super) fn copies(
    storage: &StorageBody<'_>,
    incoming: Option<EdgeId>,
    target: BlockId,
    output: &mut Vec<Assignment>,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for copy in &storage.copies(incoming, target)?.steps {
        budget.checkpoint()?;
        append(
            output,
            Assignment {
                target: copy.target,
                expression: copy_expression(copy.source),
            },
            budget,
        )?;
    }
    Ok(())
}
pub(super) fn operation(
    typed: &TypedBody<'_>,
    op: &crate::p3_ssa::BoundOp,
    budget: &mut RunContext<'_>,
) -> Result<Option<Assignment>, RequestError> {
    use crate::p2_flow::Operand;
    let numeric = |index: usize| -> Result<ValueUse, RequestError> {
        let usage = typed
            .numeric_uses
            .get(ix(op.id.0)?)
            .and_then(Option::as_ref)
            .and_then(|uses| uses.uses.get(index))
            .ok_or(RequestError::InternalInvariant(
                "numeric operation lost typed use",
            ))?;
        if usage.value != op.input(index)? {
            return Err(RequestError::InternalInvariant(
                "numeric use changed original input",
            ));
        }
        Ok(super::values::lower(
            LocalId(usage.value.0),
            usage.conversion,
        ))
    };
    let expression = match op.op {
        Op::Unary(_, unary, _) => {
            Expression::Unary(super::numeric_expression::unary(unary), numeric(0)?)
        }
        Op::ReverseSubtract(_, literal, _) => Expression::ReverseSubtract(literal, numeric(0)?),
        Op::Compare(_, _, compare, _) => Expression::Compare(
            super::numeric_expression::comparison(numeric(0)?, compare, numeric(1)?, budget)?,
        ),
        Op::LongBinary(_, _, kind, _)
        | Op::DoubleBinary(_, _, kind, _)
        | Op::FloatBinary(_, _, kind, _) => {
            Expression::Binary(numeric(0)?, kind, Right::Local(numeric(1)?))
        }
        Op::Binary(_, _, kind, right) => Expression::Binary(
            numeric(0)?,
            kind,
            match right {
                Operand::Register(_) => Right::Local(numeric(1)?),
                Operand::Literal(value) => Right::Literal(value),
            },
        ),
        Op::Nop => return Ok(None),
        Op::Exception(_) => {
            return Err(RequestError::InternalInvariant(
                "handler entered ordinary Java lowering",
            ));
        }
        Op::Monitor(..)
        | Op::Class(..)
        | Op::NewInstance(..)
        | Op::Initialize(..)
        | Op::NewArray(..)
        | Op::FilledArray(..)
        | Op::FreshFill(..)
        | Op::EmptyFill(..)
        | Op::Cast(..)
        | Op::InstanceOf(..)
        | Op::String(..)
        | Op::Call(_, _)
        | Op::ArrayLength(..)
        | Op::ArrayLoad(..)
        | Op::ArrayStore(..)
        | Op::FieldLoad(..)
        | Op::FieldStore(..)
        | Op::StaticLoad(..)
        | Op::StaticStore(..) => {
            return Err(RequestError::InternalInvariant(
                "call entered pure assignment lowering",
            ));
        }
        Op::Constant(_, value) => Expression::Constant(value),
        Op::ConstantWide(_, value) => Expression::ConstantWide(value),
        Op::Move(_, _) | Op::MoveWide(_, _) => {
            Expression::Local(if typed.numeric_uses[ix(op.id.0)?].is_some() {
                numeric(0)?
            } else {
                ValueUse::Direct(LocalId(op.input(0)?.0))
            })
        }
    };
    Ok(Some(Assignment {
        target: LocalId(
            op.result
                .ok_or(RequestError::InternalInvariant(
                    "verified operation lost its local",
                ))?
                .0,
        ),
        expression,
    }))
}
pub(crate) fn reconstruct(
    _schedule: &crate::engine::SchedulePermit,
    input: &MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    plan: crate::p6_regions::PlannedBody<'_>,
    chain: &super::constructor_graph::Proof<'_, '_, '_>,
    nested_context: bool,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<JavaMethod>, RequestError> {
    reconstruct_impl(input, catalog, plan, chain, nested_context, budget)
}

fn reconstruct_impl(
    input: &MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    plan: crate::p6_regions::PlannedBody<'_>,
    chain: &super::constructor_graph::Proof<'_, '_, '_>,
    nested_context: bool,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<JavaMethod>, RequestError> {
    use crate::substrate::outcome::Outcome;
    let storage = &plan.storage;
    chain.check(input, storage, budget)?;
    let capture_field = storage
        .typed
        .capture
        .as_ref()
        .map(|capture| capture.field());
    let mut capture_dependent = capture_field.is_some();
    for evidence in storage.typed.field_declarations.iter().flatten() {
        budget.checkpoint()?;
        if let crate::p4_types::bindings::FieldDeclaration::Known(access) = &evidence.declaration {
            capture_dependent |= access.outer_capture.is_some();
        }
    }
    if !nested_context && capture_dependent {
        return Ok(Outcome::Declined(crate::OutputReason::JavaCheckFailed {
            code: crate::DiagnosticCode::JavaAnalysisUnavailable,
            phase: "P8",
            check: "capture_requires_nested_context",
            pc: None,
        }));
    }
    let code = if storage.typed.ssa.raw.try_byte_range().as_range().is_empty() {
        crate::DiagnosticCode::BranchJavaUnsupported
    } else {
        crate::DiagnosticCode::JavaAnalysisUnavailable
    };
    let reason = |check| crate::OutputReason::JavaCheckFailed {
        code,
        phase: "P8",
        check,
        pc: None,
    };
    if matches!(plan.shape, crate::p6_regions::Shape::Protected(None)) {
        return Ok(Outcome::Declined(reason("protected_region_plan")));
    }
    let mut checks = super::dispatcher::LoweringChecks::default();
    for evidence in storage.typed.field_declarations.iter().flatten() {
        budget.checkpoint()?;
        if let crate::p4_types::bindings::FieldDeclaration::Known(access) = &evidence.declaration
            && access.flags.is_none()
        {
            super::method::note(
                &mut checks.uncertainties,
                "field_declaration_unknown",
                None,
                budget,
            )?;
        }
    }
    if !super::array_ops::validate(input, catalog, &storage.typed, budget)? {
        super::method::note(
            &mut checks.uncertainties,
            "array_operation_binding",
            None,
            budget,
        )?;
    }
    if !super::fresh_fill::validate(input, catalog, &storage.typed, budget)? {
        return Ok(Outcome::Declined(reason("fresh_array_fill")));
    }
    if !super::cast_ops::validate(input, catalog, &storage.typed, budget)? {
        super::method::note(&mut checks.uncertainties, "cast_binding", None, budget)?;
    }
    let _bind = crate::pipeline::phase_span("p8_bindings");
    let Some(mut calls) = super::calls::validate_bindings(
        input,
        catalog,
        &storage.typed,
        &mut checks.uncertainties,
        budget,
    )?
    else {
        return Ok(Outcome::Declined(reason("call_source_binding")));
    };
    drop(_bind);
    let _types = crate::pipeline::phase_span("p8_types");
    let Some(type_names) = super::types::resolve(input, catalog, storage, budget)? else {
        return Ok(Outcome::Declined(reason("storage_type_names")));
    };
    drop(_types);
    let _low = crate::pipeline::phase_span("p8_lowering");
    let method = super::dispatcher::lower(
        input,
        catalog,
        plan,
        &type_names,
        &mut calls,
        budget,
        &mut checks,
    )?;
    Ok(match method {
        Some(mut method) => {
            method.type_names = type_names;
            method.capture_field = capture_field;
            method.uncertainties = checks.uncertainties;
            Outcome::Ready(method)
        }
        None => Outcome::Declined(reason(
            checks.refused.unwrap_or("java_lowering_or_verification"),
        )),
    })
}

pub(crate) fn copy_expression(source: crate::p5_variables::CopySource) -> Expression {
    match source {
        crate::p5_variables::CopySource::Local(local) => Expression::Local(ValueUse::Direct(local)),
        literal @ crate::p5_variables::CopySource::Zero { .. } => Expression::CopyLiteral(literal),
    }
}
