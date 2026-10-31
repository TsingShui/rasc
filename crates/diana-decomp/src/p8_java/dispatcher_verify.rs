use super::branch::{Assignment, Expression, LocalId, Right};
use super::dispatcher::{Dispatcher, Edge, Transfer};
use crate::p6_variables::{Copy, CopyPlan, LocalType, LocalTypeId, Temporary};
use crate::pipeline::branch::{
    BlockId, BoundOperand, Definition, EdgeId, Kind, Op, Sym, Terminal, VerifiedSsa,
};
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
fn ty(kind: Kind) -> Result<LocalType, RequestError> {
    LocalType::new(
        LocalTypeId(match kind {
            Kind::Int => 0,
            Kind::Boolean => 1,
            Kind::Object => 2,
            Kind::Float => 3,
        }),
        1,
    )
}
fn same_expression(
    actual: &Expression,
    op: Op,
    left: Option<crate::pipeline::branch::ValueId>,
    right: Option<BoundOperand>,
) -> bool {
    match (actual, op) {
        (Expression::Compare(a, compare, b), Op::Compare(_, _, original, _)) => {
            left.is_some_and(|value| value.0 == a.0)
                && *compare == original
                && matches!(right,Some(BoundOperand::Value(value)) if value.0==b.0)
        }
        (Expression::Constant(actual), Op::Constant(_, original)) => *actual == original,
        (Expression::Local(actual), Op::Move(_, _)) => left.is_some_and(|id| actual.0 == id.0),
        (Expression::Binary(actual, kind, rhs), Op::Binary(_, _, original, _)) => {
            left.is_some_and(|id| actual.0 == id.0)
                && *kind == original
                && match (rhs, right) {
                    (Right::Local(actual), Some(BoundOperand::Value(original))) => {
                        actual.0 == original.0
                    }
                    (Right::Literal(actual), Some(BoundOperand::Literal(original))) => {
                        *actual == original
                    }
                    _ => false,
                }
        }
        _ => false,
    }
}
fn static_call(
    ssa: &VerifiedSsa<'_>,
    id: crate::pipeline::branch::CallId,
    actual: &super::calls::StaticCall,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let site = ssa
        .calls
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "call certificate has foreign site",
        ))?;
    let bound = ssa.bound_calls[ix(id.0)?]
        .as_ref()
        .ok_or(RequestError::InternalInvariant(
            "call certificate lost argument bindings",
        ))?;
    let contract = &site.contract;
    if actual.reference != contract.reference
        || actual.original_owner != contract.owner
        || actual.original_name != contract.name
        || actual.prototype != contract.prototype
        || actual.invoke != site.pc
        || actual.result_site != site.result_pc
        || actual.arguments.len() != bound.arguments.len()
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
    use crate::p4_calls::IntegralFormal;
    for ((&view, &value), &formal) in actual
        .arguments
        .iter()
        .zip(&bound.arguments)
        .zip(&contract.formals)
    {
        budget.charge_work(1)?;
        let at = ix(value.0)?;
        if view.value().0 != value.0 {
            return Ok(false);
        }
        let valid = match (view, formal, ssa.kinds[at]) {
            (ValueUse::Direct(_), IntegralFormal::Int, Some(Kind::Int))
            | (ValueUse::Direct(_), IntegralFormal::Boolean, Some(Kind::Boolean))
            | (ValueUse::BooleanWord(_), IntegralFormal::Int, Some(Kind::Boolean)) => true,
            (ValueUse::Boolean01(_), IntegralFormal::Boolean, Some(Kind::Int)) => {
                ssa.integral_bits[at] != 0 && ssa.integral_bits[at] & 4 == 0
            }
            (ValueUse::Narrow { kind, .. }, IntegralFormal::Narrow(formal), Some(Kind::Int))
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
    ssa: &VerifiedSsa<'_>,
    body: &Dispatcher,
    incoming: Option<EdgeId>,
    target: BlockId,
    actual: &[Assignment],
    types: &[LocalType],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let mut parallel = Vec::new();
    for phi in &ssa.phis {
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
    ssa: &VerifiedSsa<'_>,
    body: &Dispatcher,
    actual: &Edge,
    id: EdgeId,
    types: &[LocalType],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let original = ssa
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
    ssa: &VerifiedSsa<'_>,
    body: &Dispatcher,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    if body.locals.len() < ssa.values.len() || body.cases.len() != ssa.blocks.len() {
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
        if index < ssa.values.len() {
            if ssa.kinds[index] != Some(local.kind) {
                return Err(RequestError::InternalInvariant(
                    "dispatcher changed a solved value type",
                ));
            }
            let parameter = match ssa.values[index] {
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
    reserve(&mut seen, ssa.blocks.len(), budget)?;
    for _ in &ssa.blocks {
        budget.charge_work(1)?;
        seen.push(false);
    }
    for case in &body.cases {
        budget.charge_work(1)?;
        let index = ix(case.key.0)?;
        let block = ssa
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
        for bound in &ssa.bound[index].ops {
            budget.charge_work(1)?;
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
                super::calls::Statement::Assignment(actual) => {
                    bound.result.map(|id| id.0) == Some(actual.target.0)
                        && same_expression(&actual.expression, bound.op, bound.source, bound.other)
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
        for (id, original) in ssa.edges.iter().enumerate() {
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
            (
                Transfer::Switch {
                    value,
                    cases,
                    default,
                },
                Terminal::Switch { table, .. },
            ) => {
                let original =
                    ssa.switches
                        .get(ix(table.0)?)
                        .ok_or(RequestError::InternalInvariant(
                            "switch certificate has a foreign table",
                        ))?;
                if cases.len() != original.cases.len()
                    || outgoing.len()
                        != cases
                            .len()
                            .checked_add(1)
                            .ok_or(RequestError::PlatformCapacity)?
                    || ssa.bound[index].terminal.map(|id| id.0) != Some(value.0)
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
                if !outgoing.is_empty()
                    || ssa.bound[index].terminal.map(|id| id.0) != Some(actual.value().0)
                    || {
                        let at = ix(actual.value().0)?;
                        use super::branch::ValueUse;
                        !match actual {
                            ValueUse::Direct(_) => {
                                matches!(
                                    ssa.result,
                                    crate::pipeline::branch::DeclarationType::Int
                                        | crate::pipeline::branch::DeclarationType::Boolean
                                ) && ssa.kinds[at] == Some(ssa.result.kind())
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
                                matches!(ssa.result,crate::pipeline::branch::DeclarationType::Narrow(declared) if declared==*kind)
                                    && ssa.kinds[at] == Some(Kind::Int)
                                    && range.is_some_and(|range| range.within(kind.range()))
                            }
                            ValueUse::BooleanWord(_) => {
                                matches!(ssa.result, crate::pipeline::branch::DeclarationType::Int)
                                    && ssa.kinds[at] == Some(Kind::Boolean)
                            }
                            ValueUse::Boolean01(_) => {
                                matches!(
                                    ssa.result,
                                    crate::pipeline::branch::DeclarationType::Boolean
                                ) && ssa.kinds[at] == Some(Kind::Int)
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
                    || ssa.bound[index].terminal.map(|id| id.0) != Some(left.0)
                    || ssa.bound[index].other.map(|id| id.0) != right.map(|id| id.0)
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
