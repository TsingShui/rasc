use super::branch::{Assignment, Expression, LocalId, Right};
use super::dispatcher::{Dispatcher, Edge, Transfer};
use crate::p6_variables::{Copy, CopyPlan, LocalType, Temporary};
use crate::pipeline::branch::{BlockId, Definition, EdgeId, Kind, Op, Sym, Terminal, TypedBody};
use crate::substrate::owned::reserve;
use crate::{MethodBudget, RequestError};
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn push<T>(
    items: &mut Vec<T>,
    item: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(items, 1, budget)?;
    items.push(item);
    Ok(())
}
use crate::p6_variables::normal::local_type as ty;
fn same_expression(actual: &Expression, bound: &crate::pipeline::branch::BoundOp) -> bool {
    use crate::p2_flow::normal::Operand;
    let left = bound.inputs.first();
    let right = bound.inputs.get(1);
    match (actual, bound.op) {
        (Expression::Compare(a, compare, b), Op::Compare(_, _, original, _)) => {
            left.is_some_and(|value| value.0 == a.0)
                && *compare == original
                && right.is_some_and(|value| value.0 == b.0)
        }
        (Expression::Constant(actual), Op::Constant(_, original)) => *actual == original,
        (Expression::Local(actual), Op::Move(_, _)) => left.is_some_and(|id| actual.0 == id.0),
        (Expression::Binary(actual, kind, rhs), Op::Binary(_, _, original, operand)) => {
            left.is_some_and(|id| actual.0 == id.0)
                && *kind == original
                && match (rhs, operand) {
                    (Right::Local(actual), Operand::Register(_)) => {
                        right.is_some_and(|id| actual.0 == id.0)
                    }
                    (Right::Literal(actual), Operand::Literal(original)) => *actual == original,
                    _ => false,
                }
        }
        _ => false,
    }
}
fn static_call(
    ssa: &TypedBody<'_>,
    id: crate::pipeline::branch::CallId,
    actual: &super::calls::StaticCall,
    budget: &mut MethodBudget<'_, '_>,
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
        || actual.arguments.len() != contract.formals.len()
    {
        return Ok(false);
    }
    let owner = &contract.owner;
    if owner.len() < 3 {
        return Ok(false);
    }
    let mut source = actual.qualifier.encode_utf16();
    for &unit in &owner[1..owner.len() - 1] {
        budget.charge_work(1)?;
        if source.next() != Some(if unit == 47 { 46 } else { unit }) {
            return Ok(false);
        }
    }
    if source.next().is_some() {
        return Ok(false);
    }
    let mut source = actual.method.encode_utf16();
    for &unit in &contract.name {
        budget.charge_work(1)?;
        if source.next() != Some(unit) {
            return Ok(false);
        }
    }
    if source.next().is_some() {
        return Ok(false);
    }
    use super::branch::ValueUse;
    use crate::p4_calls::CallFormal;
    for ((&view, &value), &formal) in actual
        .arguments
        .iter()
        .zip(&bound.inputs)
        .zip(&contract.formals)
    {
        budget.charge_work(1)?;
        let at = ix(value.0)?;
        if view.value().0 != value.0 {
            return Ok(false);
        }
        let valid = match (view, formal, ssa.kinds[at]) {
            (ValueUse::Null { kind, .. }, CallFormal::Reference(expected), _) => {
                kind == expected && ssa.ssa.is_zero(value)?
            }
            (
                ValueUse::Direct(_),
                CallFormal::Reference(crate::p5_types::BuiltinReference::Object),
                Some(Kind::Object),
            )
            | (
                ValueUse::Direct(_),
                CallFormal::Reference(crate::p5_types::BuiltinReference::String),
                Some(Kind::String),
            )
            | (
                ValueUse::UpcastObject(_),
                CallFormal::Reference(crate::p5_types::BuiltinReference::Object),
                Some(Kind::String | Kind::IntArray | Kind::ObjectArray | Kind::Nominal(_)),
            ) => true,
            (ValueUse::Direct(_), CallFormal::Int, Some(Kind::Int))
            | (ValueUse::Direct(_), CallFormal::Boolean, Some(Kind::Boolean))
            | (ValueUse::BooleanWord(_), CallFormal::Int, Some(Kind::Boolean)) => true,
            (ValueUse::Boolean01(_), CallFormal::Boolean, Some(Kind::Int)) => {
                ssa.integral_bits[at] != 0 && ssa.integral_bits[at] & 4 == 0
            }
            (ValueUse::Narrow { kind, .. }, CallFormal::Narrow(formal), Some(Kind::Int))
                if kind == formal =>
            {
                let range = ssa.ranges[at];
                let range = if ssa.integral_bits[at] != 0 && ssa.integral_bits[at] & 4 == 0 {
                    range.and_then(crate::p5_types::IntRange::known01)
                } else {
                    range
                };
                range.is_some_and(|range| range.within(kind.range()))
            }
            _ => false,
        };
        if !valid {
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
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let mut parallel = Vec::new();
    for phi in &ssa.ssa.phis {
        budget.charge_work(1)?;
        if phi.block == target {
            let mut source = None;
            for &(edge, sym) in &phi.inputs {
                budget.charge_work(1)?;
                if edge == incoming {
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
            push(
                &mut parallel,
                Copy {
                    target: LocalId(phi.result.0),
                    source: LocalId(source.0),
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
        budget.charge_work(1)?;
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
        budget.charge_work(1)?;
        let Expression::Local(source) = statement.expression else {
            return Err(RequestError::InternalInvariant(
                "edge copy evaluates a non-local expression",
            ));
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
    crate::p6_variables::verify(&parallel, types, &plan, budget)
}
fn edge(
    ssa: &TypedBody<'_>,
    body: &Dispatcher,
    actual: &Edge,
    id: EdgeId,
    types: &[LocalType],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let original = ssa
        .ssa
        .edges
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "structure certificate has a foreign edge",
        ))?;
    if actual.id != id || actual.target != original.target {
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

pub(super) fn verify(
    ssa: &TypedBody<'_>,
    body: &Dispatcher,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    if body.locals.len() < ssa.ssa.values.len() || body.cases.len() != ssa.ssa.blocks.len() {
        return Err(RequestError::InternalInvariant(
            "dispatcher lost locals or block coverage",
        ));
    }
    let mut types = Vec::new();
    for (index, local) in body.locals.iter().enumerate() {
        budget.charge_work(1)?;
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
                Definition::Parameter(parameter) => Some(parameter),
                _ => None,
            };
            if local.parameter != parameter {
                return Err(RequestError::InternalInvariant(
                    "dispatcher fabricated an entry definition",
                ));
            }
            push(&mut types, ty(local.kind)?, budget)?;
        } else if local.parameter.is_some() {
            return Err(RequestError::InternalInvariant(
                "copy temporary became a parameter",
            ));
        }
    }
    copies(ssa, body, None, BlockId(0), &body.entry, &types, budget)?;
    let mut seen = Vec::new();
    reserve(&mut seen, ssa.ssa.blocks.len(), budget)?;
    for _ in &ssa.ssa.blocks {
        budget.charge_work(1)?;
        seen.push(false);
    }
    for case in &body.cases {
        budget.charge_work(1)?;
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
            budget.charge_work(1)?;
            let bound = ssa.ssa.operation(id)?;
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
                super::calls::Statement::ReceiverCall { target, call } => {
                    if let Op::Call(id, _) = bound.op {
                        target.map(|value| value.0) == bound.result.map(|value| value.0)
                            && super::receiver_ops::verify(ssa, id, call, budget)?
                    } else {
                        false
                    }
                }
                super::calls::Statement::Field(actual) => {
                    super::field_ops::verify(ssa, bound, actual, budget)?
                }
                super::calls::Statement::Array(actual) => {
                    super::array_ops::verify(ssa, bound, actual, budget)?
                }
                super::calls::Statement::Assignment(actual) => {
                    bound.result.map(|id| id.0) == Some(actual.target.0)
                        && same_expression(&actual.expression, bound)
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
        }
        if statement != case.statements.len() {
            return Err(RequestError::InternalInvariant(
                "dispatcher duplicated an operation",
            ));
        }
        let mut outgoing = Vec::new();
        for (id, original) in ssa.ssa.edges.iter().enumerate() {
            budget.charge_work(1)?;
            if original.source == case.key {
                push(
                    &mut outgoing,
                    EdgeId(u32::try_from(id).map_err(|_| RequestError::PlatformCapacity)?),
                    budget,
                )?;
            }
        }
        match (&case.transfer, block.terminal) {
            (Transfer::ReturnVoid, Terminal::ReturnVoid) => {
                if !outgoing.is_empty()
                    || !matches!(ssa.ssa.result, crate::pipeline::branch::ResultType::Void)
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
                if *form != crate::p7_regions::switch_form(cases, budget)? {
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
                    budget.charge_work(1)?;
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
                let crate::pipeline::branch::ResultType::Value(result) = ssa.ssa.result else {
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
                            ValueUse::Null { .. } => false,
                            ValueUse::UpcastObject(_) => {
                                matches!(result, crate::pipeline::branch::DeclarationType::Object)
                                    && matches!(
                                        ssa.kinds[at],
                                        Some(Kind::String | Kind::Nominal(_))
                                    )
                            }
                            ValueUse::Direct(_) => {
                                matches!(
                                    result,
                                    crate::pipeline::branch::DeclarationType::Int
                                        | crate::pipeline::branch::DeclarationType::Boolean
                                        | crate::pipeline::branch::DeclarationType::Object
                                        | crate::pipeline::branch::DeclarationType::String
                                        | crate::pipeline::branch::DeclarationType::Nominal(_)
                                ) && ssa.kinds[at] == Some(result.kind())
                            }
                            ValueUse::Narrow { kind, .. } => {
                                let range = ssa.ranges[at];
                                let range = if ssa.integral_bits[at] != 0
                                    && ssa.integral_bits[at] & 4 == 0
                                {
                                    range.and_then(crate::p5_types::IntRange::known01)
                                } else {
                                    range
                                };
                                matches!(result,crate::pipeline::branch::DeclarationType::Narrow(declared) if declared==*kind)
                                    && ssa.kinds[at] == Some(Kind::Int)
                                    && range.is_some_and(|range| range.within(kind.range()))
                            }
                            ValueUse::BooleanWord(_) => {
                                matches!(result, crate::pipeline::branch::DeclarationType::Int)
                                    && ssa.kinds[at] == Some(Kind::Boolean)
                            }
                            ValueUse::Boolean01(_) => {
                                matches!(result, crate::pipeline::branch::DeclarationType::Boolean)
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
                Transfer::Branch {
                    left,
                    right,
                    kind,
                    opcode,
                    yes,
                    no,
                },
                Terminal::Test {
                    opcode: original,
                    yes: original_yes,
                    no: original_no,
                    ..
                },
            ) => {
                if outgoing.len() != 2
                    || *opcode != original
                    || yes.target != original_yes
                    || no.target != original_no
                    || ssa.ssa.bound[index].terminal.map(|id| id.0) != Some(left.0)
                    || ssa.ssa.bound[index].other.map(|id| id.0) != right.map(|id| id.0)
                    || ssa.kinds[ix(left.0)?] != Some(*kind)
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
        budget.charge_work(1)?;
        if !covered {
            return Err(RequestError::InternalInvariant(
                "dispatcher lost a reachable block",
            ));
        }
    }
    Ok(())
}
