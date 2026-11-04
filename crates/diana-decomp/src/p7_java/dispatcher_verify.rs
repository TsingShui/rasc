use super::branch::{Assignment, Expression, LocalId, Right};
use super::dispatcher::{Dispatcher, Edge, Transfer};
use crate::p2_flow::{BlockId, EdgeId, Op, Terminal};
use crate::p3_ssa::{Definition, Sym};
use crate::p4_types::{Kind, TypedBody};
use crate::p5_variables::{Copy, CopyPlan, LocalType, Temporary};
use crate::substrate::owned::reserve;
use crate::{RequestError, RunContext};
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn push<T>(items: &mut Vec<T>, item: T, budget: &mut RunContext<'_>) -> Result<(), RequestError> {
    reserve(items, 1, budget)?;
    items.push(item);
    Ok(())
}
use crate::p5_variables::local_type as ty;
pub(super) fn numeric_view(
    typed: &TypedBody<'_>,
    actual: super::branch::ValueUse,
    source: Option<&crate::p3_ssa::ValueId>,
    expected: Kind,
) -> Result<bool, RequestError> {
    let Some(source) = source else {
        return Ok(false);
    };
    if actual.value().0 != source.0 {
        return Ok(false);
    }
    use super::branch::ValueUse;
    Ok(match actual {
        ValueUse::Direct(_) => typed.kinds[ix(source.0)?] == Some(expected),
        ValueUse::BooleanWord(_) => {
            expected == Kind::Int && typed.kinds[ix(source.0)?] == Some(Kind::Boolean)
        }
        ValueUse::DoubleLiteral { bits, .. } => {
            if expected != Kind::Double
                || bits & 0x7ff0_0000_0000_0000 == 0x7ff0_0000_0000_0000
                    && bits & 0x000f_ffff_ffff_ffff != 0
            {
                return Ok(false);
            }
            let Definition::OpResult(id) = typed.ssa.values[ix(source.0)?] else {
                return Ok(false);
            };
            matches!(typed.ssa.operation(id)?.op,Op::ConstantWide(_,original) if u64::from_le_bytes(original.to_le_bytes()) == bits)
        }
        ValueUse::FloatLiteral { bits, .. } => {
            if expected != Kind::Float
                || bits & 0x7f80_0000 == 0x7f80_0000 && bits & 0x007f_ffff != 0
            {
                return Ok(false);
            }
            let Definition::OpResult(id) = typed.ssa.values[ix(source.0)?] else {
                return Ok(false);
            };
            matches!(typed.ssa.operation(id)?.op, Op::Constant(_,original) if u32::from_le_bytes(original.to_le_bytes()) == bits)
        }
        _ => false,
    })
}
fn same_expression(
    typed: &TypedBody<'_>,
    actual: &Expression,
    bound: &crate::p3_ssa::BoundOp,
) -> Result<bool, RequestError> {
    use crate::p2_flow::Operand;
    let left = bound.inputs.first();
    let right = bound.inputs.get(1);
    let required = |index| {
        crate::p4_types::numeric::expected(bound.op, index)?.ok_or(RequestError::InternalInvariant(
            "numeric verification lost operand family",
        ))
    };
    Ok(match (actual, bound.op) {
        (Expression::Unary(actual, source), Op::Unary(_, original, _)) => {
            super::numeric_expression::verify_unary(*actual, original)
                && numeric_view(typed, *source, left, required(0)?)?
        }
        (Expression::ReverseSubtract(actual, source), Op::ReverseSubtract(_, original, _)) => {
            *actual == original && numeric_view(typed, *source, left, Kind::Int)?
        }
        (Expression::Compare(tree), Op::Compare(_, _, original, _)) => {
            match super::numeric_expression::verify(tree, original)? {
                Some((a, b)) => {
                    numeric_view(typed, a, left, required(0)?)?
                        && numeric_view(typed, b, right, required(1)?)?
                }
                None => false,
            }
        }
        (Expression::Constant(actual), Op::Constant(_, original)) => *actual == original,
        (Expression::ConstantWide(actual), Op::ConstantWide(_, original)) => *actual == original,
        (Expression::Local(actual), Op::Move(..) | Op::MoveWide(..)) => {
            if typed.numeric_uses[ix(bound.id.0)?].is_some() {
                let kind = typed.kinds[ix(bound
                    .result
                    .ok_or(RequestError::InternalInvariant("copy lost result"))?
                    .0)?]
                .ok_or(RequestError::InternalInvariant("copy lost storage family"))?;
                numeric_view(typed, *actual, left, kind)?
            } else {
                left.is_some_and(|value| {
                    *actual == super::branch::ValueUse::Direct(LocalId(value.0))
                })
            }
        }
        (
            Expression::Binary(actual, kind, Right::Local(rhs)),
            Op::FloatBinary(_, _, original, _)
            | Op::LongBinary(_, _, original, _)
            | Op::DoubleBinary(_, _, original, _),
        ) => {
            numeric_view(typed, *actual, left, required(0)?)?
                && *kind == original
                && numeric_view(typed, *rhs, right, required(1)?)?
        }
        (Expression::Binary(actual, kind, rhs), Op::Binary(_, _, original, operand)) => {
            numeric_view(typed, *actual, left, Kind::Int)?
                && *kind == original
                && match (rhs, operand) {
                    (Right::Local(actual), Operand::Register(_)) => {
                        numeric_view(typed, *actual, right, Kind::Int)?
                    }
                    (Right::Literal(actual), Operand::Literal(original)) => *actual == original,
                    _ => false,
                }
        }
        _ => false,
    })
}
pub(super) fn call_argument(
    ssa: &TypedBody<'_>,
    view: super::branch::ValueUse,
    value: crate::p3_ssa::ValueId,
    formal: crate::p2_flow::DeclarationType,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    use super::branch::ValueUse;
    use crate::p2_flow::DeclarationType;
    let at = ix(value.0)?;
    if view.value().0 != value.0 {
        return Ok(false);
    }
    if formal.field_offset() == 2 {
        return super::values::verify_reference_store(ssa, view, value, formal, budget);
    }
    Ok(match formal {
        DeclarationType::Boolean => match view {
            ValueUse::Direct(_) => ssa.kinds[at] == Some(Kind::Boolean),
            ValueUse::Boolean01(_) => {
                ssa.kinds[at] == Some(Kind::Int)
                    && ssa.integral_bits[at] != 0
                    && ssa.integral_bits[at] & 4 == 0
            }
            _ => false,
        },
        DeclarationType::Narrow(expected) => match view {
            ValueUse::Narrow { kind, .. }
                if kind == expected && ssa.kinds[at] == Some(Kind::Int) =>
            {
                let range = if ssa.integral_bits[at] != 0 && ssa.integral_bits[at] & 4 == 0 {
                    ssa.ranges[at].and_then(crate::p4_types::IntRange::known01)
                } else {
                    ssa.ranges[at]
                };
                range.is_some_and(|range| range.within(kind.range()))
            }
            _ => false,
        },
        _ => numeric_view(ssa, view, Some(&value), formal.kind())?,
    })
}

fn static_call(
    ssa: &TypedBody<'_>,
    id: crate::p2_flow::CallId,
    actual: &super::calls::StaticCall,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let site = ssa
        .ssa
        .calls
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "call certificate has foreign site",
        ))?;
    let bound = ssa.ssa.call(id)?;
    let contract = ssa.contract(id)?;
    if actual.reference != contract.reference
        || actual.original_owner != contract.owner
        || actual.original_name != contract.name
        || actual.prototype != contract.prototype
        || actual.invoke != site.pc
        || actual.result_site != site.result_pc
        || actual.arguments.len() != bound.inputs.len()
        || actual.arguments.len() != site.formals.len()
    {
        return Ok(false);
    }
    let owner = &contract.owner;
    if owner.len() < 3 {
        return Ok(false);
    }
    let mut source = actual.qualifier.encode_utf16();
    for &unit in &owner[1..owner.len() - 1] {
        budget.checkpoint()?;
        if source.next() != Some(if unit == 47 { 46 } else { unit }) {
            return Ok(false);
        }
    }
    if source.next().is_some() {
        return Ok(false);
    }
    let mut source = actual.method.encode_utf16();
    for &unit in &contract.name {
        budget.checkpoint()?;
        if source.next() != Some(unit) {
            return Ok(false);
        }
    }
    if source.next().is_some() {
        return Ok(false);
    }
    for ((&view, &value), &formal) in actual
        .arguments
        .iter()
        .zip(&bound.inputs)
        .zip(&site.formals)
    {
        budget.checkpoint()?;
        if !call_argument(ssa, view, value, formal, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn copies(
    ssa: &TypedBody<'_>,
    body: &Dispatcher,
    incoming: Option<EdgeId>,
    target: BlockId,
    actual: &[Assignment],
    types: &[LocalType],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let mut parallel = Vec::new();
    for phi in &ssa.ssa.phis {
        budget.checkpoint()?;
        if phi.block == target {
            let mut source = None;
            for &(edge, sym) in &phi.inputs {
                budget.checkpoint()?;
                if edge
                    == incoming.map_or(
                        crate::p3_ssa::Incoming::EntrySeed,
                        crate::p3_ssa::Incoming::Normal,
                    )
                {
                    if source.is_some() {
                        return Err(RequestError::InternalInvariant(
                            "definition certificate has duplicate incoming edges",
                        ));
                    }
                    source = Some(sym);
                }
            }
            let Some(Sym::Value(source)) = source else {
                return Err(RequestError::InternalInvariant(
                    "definition certificate lost an available edge value",
                ));
            };
            let target_kind = ssa.kinds[ix(phi.result.0)?]
                .ok_or(RequestError::InternalInvariant("Phi target lost kind"))?;
            let source_kind = ssa.kinds[ix(source.0)?]
                .ok_or(RequestError::InternalInvariant("Phi source lost kind"))?;
            let source = if ty(source_kind)? == ty(target_kind)? {
                crate::p5_variables::CopySource::Local(LocalId(source.0))
            } else {
                if !crate::p4_types::zero::proven(&ssa.ssa, source, budget)? {
                    return Err(RequestError::InternalInvariant(
                        "Phi zero view lost original provenance",
                    ));
                }
                let evidence =
                    ssa.zeros
                        .evidence(source)?
                        .ok_or(RequestError::InternalInvariant(
                            "Phi literal lost zero evidence",
                        ))?;
                let view = crate::p4_types::zero::view(target_kind).ok_or(
                    RequestError::InternalInvariant("Phi literal has invalid view"),
                )?;
                crate::p5_variables::CopySource::Zero {
                    evidence,
                    view,
                    kind: ty(target_kind)?,
                }
            };
            push(
                &mut parallel,
                Copy {
                    target: LocalId(phi.result.0),
                    source,
                },
                budget,
            )?;
        }
    }
    let mut plan = CopyPlan {
        steps: Vec::new(),
        temporaries: Vec::new(),
    };
    for local in &body.locals[types.len()..] {
        budget.checkpoint()?;
        push(
            &mut plan.temporaries,
            Temporary {
                local: local.id,
                kind: ty(local.kind)?,
            },
            budget,
        )?;
    }
    for statement in actual {
        budget.checkpoint()?;
        let source = match statement.expression {
            Expression::Local(super::branch::ValueUse::Direct(local)) => {
                crate::p5_variables::CopySource::Local(local)
            }
            Expression::CopyLiteral(
                literal @ crate::p5_variables::CopySource::Zero { evidence, .. },
            ) => {
                if !crate::p4_types::zero::proven(&ssa.ssa, evidence.value(), budget)? {
                    return Err(RequestError::InternalInvariant(
                        "copy literal changed original value",
                    ));
                }
                literal
            }
            _ => {
                return Err(RequestError::InternalInvariant(
                    "edge copy evaluates a non-copy expression",
                ));
            }
        };
        push(
            &mut plan.steps,
            Copy {
                target: statement.target,
                source,
            },
            budget,
        )?;
    }
    crate::p5_variables::verify(&parallel, types, &plan, budget)
}
fn edge(
    ssa: &TypedBody<'_>,
    body: &Dispatcher,
    actual: &Edge,
    id: EdgeId,
    types: &[LocalType],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let original = ssa
        .ssa
        .edges
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "structure certificate has a foreign edge",
        ))?;
    if actual.id != id || actual.source != original.source || actual.target != original.target {
        return Err(RequestError::InternalInvariant(
            "structure certificate changed the selected edge",
        ));
    }
    copies(
        ssa,
        body,
        Some(id),
        original.target,
        &actual.copies,
        types,
        budget,
    )
}

pub(super) fn snapshot_copies(
    storage: &crate::p5_variables::StorageBody<'_>,
    block: BlockId,
    pc: crate::CodeUnitOffset,
    statements: &[super::calls::Statement],
    index: &mut usize,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for expected in &storage.exceptional.snapshots {
        budget.checkpoint()?;
        if expected.source_block != block || expected.before != pc {
            continue;
        }
        if !matches!(statements.get(*index), Some(super::calls::Statement::Snapshot(actual)) if actual == expected)
        {
            return Err(RequestError::InternalInvariant(
                "snapshot changed same-site placement",
            ));
        }
        *index = index.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(())
}

pub(super) fn verify(
    input: &crate::input::MethodSource<'_, '_>,
    storage: &crate::p5_variables::StorageBody<'_>,
    body: &Dispatcher,
    names: &super::types::TypeNames,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let ssa = &storage.typed;
    if body.prologue.len() > 1
        || !super::constructor_entry::verify(storage, body.prologue.first(), budget)?
    {
        return Err(RequestError::InternalInvariant(
            "constructor first-statement certificate changed",
        ));
    }
    let base_count = ssa
        .ssa
        .values
        .len()
        .checked_add(storage.empty_fills.len())
        .ok_or(RequestError::PlatformCapacity)?;
    if body.locals.len() < base_count || body.cases.len() != ssa.ssa.blocks.len() {
        return Err(RequestError::InternalInvariant(
            "dispatcher lost locals or block coverage",
        ));
    }
    let mut types = Vec::new();
    for (index, local) in body.locals.iter().enumerate() {
        budget.checkpoint()?;
        if !super::local_declarations::verify(local, names, budget)? {
            return Err(RequestError::InternalInvariant(
                "Java local declaration differs from storage",
            ));
        }
        if ix(local.id.0)? != index {
            return Err(RequestError::InternalInvariant(
                "dispatcher local identity was reused",
            ));
        }
        if index < ssa.ssa.values.len() {
            if ssa.kinds[index] != Some(local.kind) {
                return Err(RequestError::InternalInvariant(
                    "dispatcher changed a solved value type",
                ));
            }
            let parameter = match ssa.ssa.values[index] {
                Definition::Parameter(parameter) => Some(match ssa.ssa.frame {
                    crate::p2_flow::MethodFrame::Static => {
                        crate::p5_variables::Parameter::Explicit(parameter)
                    }
                    crate::p2_flow::MethodFrame::Instance(_)
                    | crate::p2_flow::MethodFrame::Constructor(_)
                        if parameter == 0 =>
                    {
                        crate::p5_variables::Parameter::This
                    }
                    crate::p2_flow::MethodFrame::Instance(_)
                    | crate::p2_flow::MethodFrame::Constructor(_) => {
                        crate::p5_variables::Parameter::Explicit(parameter.checked_sub(1).ok_or(
                            RequestError::InternalInvariant("source receiver frame underflow"),
                        )?)
                    }
                }),
                _ => None,
            };
            let initializer = if ssa.initializations.deferred.get(index)
                == Some(&Some(
                    crate::p4_types::initialization::InitializationId::This,
                )) {
                crate::p5_variables::Initializer::ThisAfterChain
            } else if let Some(parameter) = parameter {
                crate::p5_variables::Initializer::Parameter(parameter)
            } else if let Some(Some(id)) = ssa.initializations.deferred.get(index) {
                crate::p5_variables::Initializer::Deferred(*id)
            } else {
                crate::p5_variables::Initializer::Default
            };
            if local.initializer != initializer {
                return Err(RequestError::InternalInvariant(
                    "dispatcher fabricated an entry definition",
                ));
            }
            push(&mut types, ty(local.kind)?, budget)?;
        } else if index < base_count {
            let proof = &storage.empty_fills[index - ssa.ssa.values.len()];
            if proof.result != local.id
                || local.kind != Kind::Int
                || local.initializer != crate::p5_variables::Initializer::Default
            {
                return Err(RequestError::InternalInvariant(
                    "LengthOnly changed storage",
                ));
            }
            push(&mut types, ty(local.kind)?, budget)?;
        } else if local.initializer != crate::p5_variables::Initializer::Default {
            return Err(RequestError::InternalInvariant(
                "copy temporary became a parameter",
            ));
        }
    }
    copies(ssa, body, None, BlockId(0), &body.entry, &types, budget)?;
    let mut seen = Vec::new();
    reserve(&mut seen, ssa.ssa.blocks.len(), budget)?;
    for _ in &ssa.ssa.blocks {
        budget.checkpoint()?;
        seen.push(false);
    }
    for case in &body.cases {
        budget.checkpoint()?;
        let index = ix(case.key.0)?;
        let block = ssa
            .ssa
            .blocks
            .get(index)
            .ok_or(RequestError::InternalInvariant(
                "dispatcher has a foreign case key",
            ))?;
        if seen[index] {
            return Err(RequestError::InternalInvariant(
                "dispatcher duplicates a reachable block",
            ));
        }
        seen[index] = true;
        let mut statement = 0usize;
        for &id in &ssa.ssa.bound[index].ops {
            budget.checkpoint()?;
            let bound = ssa.ssa.operation(id)?;
            snapshot_copies(
                storage,
                case.key,
                bound.pc,
                &case.statements,
                &mut statement,
                budget,
            )?;
            if let Some(capture) = &ssa.capture
                && capture.operation() == id
            {
                if !matches!(bound.op, Op::FieldStore(_, _, field, _) if field == capture.field())
                    || bound.pc != crate::CodeUnitOffset(0)
                    || !matches!(storage.constructor_entry.as_ref(), Some(entry) if entry.pc == capture.initialization())
                {
                    return Err(RequestError::InternalInvariant(
                        "capture source omission lost shared certificate",
                    ));
                }
                continue;
            }
            if matches!(bound.op, Op::Nop) {
                continue;
            }
            let actual = case
                .statements
                .get(statement)
                .ok_or(RequestError::InternalInvariant(
                    "dispatcher omitted an original operation",
                ))?;
            let matches = match actual {
                super::calls::Statement::Monitor { .. } => {
                    super::monitor_ops::verify(ssa, id, bound, actual, budget)?
                }
                super::calls::Statement::Class {
                    target,
                    reference,
                    source,
                } => {
                    if let Op::Class(destination, original) = bound.op {
                        let layout = crate::raw::instruction_layout(
                            ssa.ssa.raw.instructions(),
                            bound.pc,
                            ssa.ssa.raw.version(),
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let operands = crate::input::operands::instruction_operands(
                            ssa.ssa.raw.instructions(),
                            bound.pc,
                            layout,
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let mut registers = operands.registers().iter();
                        *reference == original
                            && bound.result.map(|id| id.0) == Some(target.0)
                            && ssa.kinds[ix(target.0)?] == Some(Kind::Class)
                            && registers.next() == Some(destination)
                            && registers.next().is_none()
                            && matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x1c)
                            && matches!(layout.references()[0], Some(original) if original.target == crate::raw::OperandReference::Type(*reference))
                            && names.name(Kind::Nominal(*reference), budget)? == source
                    } else {
                        false
                    }
                }
                super::calls::Statement::Snapshot(_) => false,
                super::calls::Statement::ExceptionBind { handler, target } => {
                    matches!(bound.op, Op::Exception(_))
                        && *handler == case.key
                        && bound.result.map(|value| value.0) == Some(target.0)
                }
                super::calls::Statement::ConstructorChain(reference) => {
                    if let Op::Initialize(call) = bound.op {
                        body.prologue.first().is_some_and(|entry| {
                            entry.pc == bound.pc && entry.contract.reference == *reference
                        }) && ssa.initializations.calls.get(ix(call.0)?)
                            == Some(&Some(
                                crate::p4_types::initialization::InitializationId::This,
                            ))
                    } else {
                        false
                    }
                }
                super::calls::Statement::Deferred(id) => {
                    super::object_allocation::verify_deferred(storage, bound, *id, budget)?
                }
                super::calls::Statement::Initialize(actual) => {
                    super::object_allocation::verify(storage, bound, actual, budget)?
                }
                super::calls::Statement::NewArray(actual) => super::fresh_fill::verify_allocation(
                    input, storage, bound, actual, names, budget,
                )?,
                super::calls::Statement::FreshFill(actual) => {
                    super::fresh_fill::verify_fill(storage, bound, actual, budget)?
                }
                super::calls::Statement::EmptyFill {
                    array,
                    result,
                    payload,
                } => {
                    if let Op::EmptyFill(_, id) = bound.op {
                        let proof = storage.empty_fills.get(ix(id.0)?).ok_or(
                            RequestError::InternalInvariant("fill certificate lost storage"),
                        )?;
                        proof.pc == bound.pc
                            && proof.payload == *payload
                            && proof.result == *result
                            && proof.array.0 == array.0
                            && bound.input(0)?.0 == array.0
                            && bound.result.is_none()
                    } else {
                        false
                    }
                }
                super::calls::Statement::TypeCheck(actual) => {
                    super::cast_ops::verify(ssa, bound, actual, names, budget)?
                }
                super::calls::Statement::String {
                    target,
                    reference,
                    units,
                } => {
                    if let Op::String(destination, literal_id) = bound.op {
                        let literal = ssa.ssa.strings.get(ix(literal_id.0)?).ok_or(
                            RequestError::InternalInvariant("literal certificate lost original"),
                        )?;
                        let layout = crate::raw::instruction_layout(
                            ssa.ssa.raw.instructions(),
                            bound.pc,
                            ssa.ssa.raw.version(),
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let operands = crate::input::operands::instruction_operands(
                            ssa.ssa.raw.instructions(),
                            bound.pc,
                            layout,
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let mut registers = operands.registers().iter();
                        bound.result.map(|value| value.0) == Some(target.0)
                            && bound.inputs.is_empty()
                            && ssa.kinds[ix(target.0)?] == Some(Kind::String)
                            && matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode, 0x1a | 0x1b))
                            && registers.next() == Some(destination)
                            && registers.next().is_none()
                            && *reference == literal.reference
                            && matches!(layout.references()[0],Some(original) if original.target == crate::raw::OperandReference::String(*reference))
                            && literal.value.matches_with(units, budget)?
                    } else {
                        false
                    }
                }
                super::calls::Statement::ReceiverCall { target, call } => {
                    if let Op::Call(id, _) = bound.op {
                        target.map(|value| value.0) == bound.result.map(|value| value.0)
                            && super::receiver_ops::verify(ssa, id, call, budget)?
                    } else {
                        false
                    }
                }
                super::calls::Statement::Field(actual) => {
                    super::field_ops::verify(ssa, names, bound, actual, budget)?
                }
                super::calls::Statement::Array(actual) => {
                    super::array_ops::verify(input, ssa, bound, actual, budget)?
                }
                super::calls::Statement::Assignment(actual) => {
                    bound.result.map(|id| id.0) == Some(actual.target.0)
                        && same_expression(ssa, &actual.expression, bound)?
                }
                super::calls::Statement::StaticCall { target, call } => {
                    if let Op::Call(id, _) = bound.op {
                        target.map(|value| value.0) == bound.result.map(|value| value.0)
                            && static_call(ssa, id, call, budget)?
                    } else {
                        false
                    }
                }
            };
            if !matches {
                return Err(RequestError::InternalInvariant(
                    "dispatcher changed operation order or value correspondence",
                ));
            }
            statement = statement
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            if matches!(bound.op, Op::Exception(_)) {
                let ingress = super::exceptional::ingress(storage, case.key, budget)?;
                for copy in &ingress.copies.steps {
                    budget.checkpoint()?;
                    if !matches!(case.statements.get(statement), Some(super::calls::Statement::Assignment(actual))
                        if actual.target == copy.target && matches!(actual.expression, Expression::Local(super::branch::ValueUse::Direct(source)) if Some(source) == copy.source.local()))
                    {
                        return Err(RequestError::InternalInvariant(
                            "handler ingress changed copy or binding position",
                        ));
                    }
                    statement = statement
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                }
            }
        }
        if matches!(block.terminal, Terminal::Throw(_)) {
            snapshot_copies(
                storage,
                case.key,
                block.terminal_pc,
                &case.statements,
                &mut statement,
                budget,
            )?;
        }
        if statement != case.statements.len() {
            return Err(RequestError::InternalInvariant(
                "dispatcher duplicated an operation",
            ));
        }
        let mut outgoing = Vec::new();
        for (id, original) in ssa.ssa.edges.iter().enumerate() {
            budget.checkpoint()?;
            if original.source == case.key {
                push(
                    &mut outgoing,
                    EdgeId(u32::try_from(id).map_err(|_| RequestError::PlatformCapacity)?),
                    budget,
                )?;
            }
        }
        match (&case.transfer, block.terminal) {
            (
                Transfer::Throw(actual) | Transfer::Rethrow { value: actual, .. },
                Terminal::Throw(_),
            ) => {
                let value =
                    ssa.ssa.bound[index]
                        .terminal
                        .ok_or(RequestError::InternalInvariant(
                            "throw certificate lost operand",
                        ))?;
                if let Transfer::Rethrow { handler, .. } = case.transfer
                    && super::exceptional::exception_root(ssa, value, budget)? != Some(handler)
                {
                    return Err(RequestError::InternalInvariant(
                        "rethrow changed catch parameter identity",
                    ));
                }
                if !outgoing.is_empty()
                    || actual.value().0 != value.0
                    || !match actual {
                        super::branch::ValueUse::NullAssignment(_) => {
                            crate::p4_types::zero::proven(&ssa.ssa, value, budget)?
                        }
                        super::branch::ValueUse::Direct(_) => {
                            matches!(
                                ssa.kinds[ix(value.0)?],
                                Some(Kind::Nominal(_) | Kind::Throwable)
                            )
                        }
                        _ => false,
                    }
                {
                    return Err(RequestError::InternalInvariant(
                        "throw changed operand or normal successors",
                    ));
                }
            }
            (Transfer::ReturnVoid | Transfer::InitializerExit, Terminal::ReturnVoid) => {
                let initializer = super::initializers::is_initializer(input, budget)?;
                if matches!(case.transfer, Transfer::InitializerExit) != initializer
                    || initializer && !super::initializers::header_admitted(input, budget)?
                {
                    return Err(RequestError::InternalInvariant(
                        "void completion changed initializer context",
                    ));
                }
                if !outgoing.is_empty()
                    || !matches!(ssa.ssa.result, crate::p2_flow::ResultType::Void)
                    || ssa.ssa.bound[index].terminal.is_some()
                {
                    return Err(RequestError::InternalInvariant(
                        "void return certificate acquired a value or successor",
                    ));
                }
            }
            (
                Transfer::Switch {
                    form,
                    value,
                    cases,
                    default,
                },
                Terminal::Switch { table, .. },
            ) => {
                let original =
                    ssa.ssa
                        .switches
                        .get(ix(table.0)?)
                        .ok_or(RequestError::InternalInvariant(
                            "switch certificate has a foreign table",
                        ))?;
                if *form != crate::p6_regions::switch_form(cases, budget)? {
                    return Err(RequestError::InternalInvariant(
                        "switch density selection differs from its keys",
                    ));
                }
                if cases.len() != original.cases.len()
                    || outgoing.len()
                        != cases
                            .len()
                            .checked_add(1)
                            .ok_or(RequestError::PlatformCapacity)?
                    || ssa.ssa.bound[index].terminal.map(|id| id.0) != Some(value.0)
                    || default.target != original.default
                {
                    return Err(RequestError::InternalInvariant(
                        "switch certificate changed selector or edge coverage",
                    ));
                }
                for (position, ((actual_key, actual), (key, target))) in
                    cases.iter().zip(&original.cases).enumerate()
                {
                    budget.checkpoint()?;
                    if actual_key != key || actual.target != *target {
                        return Err(RequestError::InternalInvariant(
                            "switch certificate changed an original key or target",
                        ));
                    }
                    edge(ssa, body, actual, outgoing[position], &types, budget)?;
                }
                edge(ssa, body, default, outgoing[cases.len()], &types, budget)?;
            }
            (Transfer::Return(actual), Terminal::Return(_)) => {
                let crate::p2_flow::ResultType::Value(result) = ssa.ssa.result else {
                    return Err(RequestError::InternalInvariant(
                        "value return certificate has void descriptor",
                    ));
                };
                if !outgoing.is_empty()
                    || ssa.ssa.bound[index].terminal.map(|id| id.0) != Some(actual.value().0)
                    || {
                        let at = ix(actual.value().0)?;
                        use super::branch::ValueUse;
                        !match actual {
                            ValueUse::ObjectArrayView(_) => false,
                            ValueUse::NullAssignment(_) => {
                                result.field_offset() == 2
                                    && crate::p4_types::zero::proven(
                                        &ssa.ssa,
                                        crate::p3_ssa::ValueId(actual.value().0),
                                        budget,
                                    )?
                            }
                            ValueUse::FloatLiteral { .. } | ValueUse::DoubleLiteral { .. } => {
                                numeric_view(
                                    ssa,
                                    *actual,
                                    ssa.ssa.bound[index].terminal.as_ref(),
                                    result.kind(),
                                )?
                            }
                            ValueUse::UpcastObject(_) => {
                                matches!(result, crate::p2_flow::DeclarationType::Object)
                                    && matches!(
                                        ssa.kinds[at],
                                        Some(
                                            Kind::String
                                                | Kind::Class
                                                | Kind::Throwable
                                                | Kind::Nominal(_)
                                                | Kind::ReferenceArray(_)
                                                | Kind::ProjectedNominal(_)
                                                | Kind::PrimitiveArray(_)
                                                | Kind::ObjectArray
                                        )
                                    )
                            }
                            ValueUse::Direct(_) => {
                                if result.field_offset() == 2 {
                                    ssa.kinds[at]
                                        .is_some_and(crate::p4_types::reference_roots::reference)
                                } else {
                                    matches!(
                                        result,
                                        crate::p2_flow::DeclarationType::Int
                                            | crate::p2_flow::DeclarationType::Float
                                            | crate::p2_flow::DeclarationType::Long
                                            | crate::p2_flow::DeclarationType::Double
                                            | crate::p2_flow::DeclarationType::Boolean
                                    ) && ssa.kinds[at] == Some(result.kind())
                                }
                            }
                            ValueUse::Narrow { kind, .. } => {
                                let range = ssa.ranges[at];
                                let range = if ssa.integral_bits[at] != 0
                                    && ssa.integral_bits[at] & 4 == 0
                                {
                                    range.and_then(crate::p4_types::IntRange::known01)
                                } else {
                                    range
                                };
                                matches!(result,crate::p2_flow::DeclarationType::Narrow(declared) if declared==*kind)
                                    && ssa.kinds[at] == Some(Kind::Int)
                                    && range.is_some_and(|range| range.within(kind.range()))
                            }
                            ValueUse::BooleanWord(_) => {
                                matches!(result, crate::p2_flow::DeclarationType::Int)
                                    && ssa.kinds[at] == Some(Kind::Boolean)
                            }
                            ValueUse::Boolean01(_) => {
                                matches!(result, crate::p2_flow::DeclarationType::Boolean)
                                    && ssa.kinds[at] == Some(Kind::Int)
                                    && ssa.integral_bits[at] != 0
                                    && ssa.integral_bits[at] & 4 == 0
                            }
                        }
                    }
                {
                    return Err(RequestError::InternalInvariant(
                        "dispatcher changed return control",
                    ));
                }
            }
            (Transfer::Jump(actual), Terminal::Jump(target)) => {
                if outgoing.len() != 1 || actual.target != target {
                    return Err(RequestError::InternalInvariant(
                        "dispatcher changed jump control",
                    ));
                }
                edge(ssa, body, actual, outgoing[0], &types, budget)?;
            }
            (
                Transfer::Branch { condition, yes, no },
                Terminal::Test {
                    yes: original_yes,
                    no: original_no,
                    ..
                },
            ) => {
                if outgoing.len() != 2
                    || yes.target != original_yes
                    || no.target != original_no
                    || !super::conditions::verify(
                        ssa,
                        &ssa.ssa.bound[index],
                        block.terminal,
                        condition,
                        budget,
                    )?
                {
                    return Err(RequestError::InternalInvariant(
                        "dispatcher changed branch evaluation or edge identity",
                    ));
                }
                edge(ssa, body, yes, outgoing[0], &types, budget)?;
                edge(ssa, body, no, outgoing[1], &types, budget)?;
            }
            _ => {
                return Err(RequestError::InternalInvariant(
                    "dispatcher changed terminal kind",
                ));
            }
        }
    }
    for covered in seen {
        budget.checkpoint()?;
        if !covered {
            return Err(RequestError::InternalInvariant(
                "dispatcher lost a reachable block",
            ));
        }
    }
    Ok(())
}
