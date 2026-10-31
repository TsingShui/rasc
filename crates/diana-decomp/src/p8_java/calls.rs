use super::branch::{Assignment, LocalId, ValueUse};
use crate::p4_calls::CallFormal;
use crate::pipeline::branch::{CallId, Kind, Op, VerifiedSsa};
use crate::substrate::owned::{TextBuffer, reserve};
use crate::{CodeUnitOffset, MethodBudget, MethodId, RequestError};

/// Original invocation kind retained by verified source binding.
#[derive(Clone, Copy, Debug)]
pub enum JavaInvokeKind {
    Direct,
    Static,
    Virtual,
    Interface,
}
/// Borrowed original call identity in an owned verified method AST.
pub struct JavaCallSite<'a> {
    pub reference: MethodId,
    pub receiver: Option<crate::RegisterId>,
    pub owner: &'a [u16],
    pub name: &'a [u16],
    pub prototype: &'a [u16],
    pub invoke: CodeUnitOffset,
    pub result: Option<CodeUnitOffset>,
    pub kind: JavaInvokeKind,
}
pub(crate) struct StaticCall {
    _sealed: (),
    pub reference: MethodId,
    pub original_owner: Vec<u16>,
    pub original_name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub invoke: CodeUnitOffset,
    pub result_site: Option<CodeUnitOffset>,
    pub qualifier: String,
    pub method: String,
    pub arguments: Vec<ValueUse>,
}
pub(crate) enum Statement {
    Assignment(Assignment),
    StaticCall {
        target: Option<LocalId>,
        call: StaticCall,
    },
}
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
fn copy(units: &[u16], budget: &mut MethodBudget<'_, '_>) -> Result<Vec<u16>, RequestError> {
    let mut out = Vec::new();
    reserve(&mut out, units.len(), budget)?;
    for &unit in units {
        budget.charge_work(1)?;
        out.push(unit);
    }
    Ok(out)
}
fn text(units: &[u16], budget: &mut MethodBudget<'_, '_>) -> Result<Option<String>, RequestError> {
    if !super::declarations::identifier(units, budget)? {
        return Ok(None);
    }
    let mut output = TextBuffer::new();
    for &unit in units {
        budget.charge_work(1)?;
        output.character(
            char::from_u32(u32::from(unit)).ok_or(RequestError::InternalInvariant(
                "checked identifier lost its scalar",
            ))?,
            budget,
        )?;
    }
    Ok(Some(output.finish()))
}
pub(super) fn argument(
    ssa: &VerifiedSsa<'_>,
    value: LocalId,
    formal: CallFormal,
) -> Result<Option<ValueUse>, RequestError> {
    let index = ix(value.0)?;
    if let CallFormal::Reference(kind) = formal {
        return Ok(
            if matches!(
                ssa.values[index],
                crate::pipeline::branch::Definition::Constant(0)
            ) {
                Some(ValueUse::Null { value, kind })
            } else {
                match (ssa.kinds[index], kind) {
                    (Some(Kind::Object), crate::p5_types::BuiltinReference::Object)
                    | (Some(Kind::String), crate::p5_types::BuiltinReference::String) => {
                        Some(ValueUse::Direct(value))
                    }
                    (Some(Kind::String), crate::p5_types::BuiltinReference::Object) => {
                        Some(ValueUse::UpcastObject(value))
                    }
                    _ => None,
                }
            },
        );
    }
    Ok(match (ssa.kinds[index], formal) {
        (Some(Kind::Int), CallFormal::Int) | (Some(Kind::Boolean), CallFormal::Boolean) => {
            Some(ValueUse::Direct(value))
        }
        (Some(Kind::Boolean), CallFormal::Int) => Some(ValueUse::BooleanWord(value)),
        (Some(Kind::Int), CallFormal::Boolean)
            if ssa.integral_bits[index] != 0 && ssa.integral_bits[index] & 4 == 0 =>
        {
            Some(ValueUse::Boolean01(value))
        }
        (Some(Kind::Int), CallFormal::Narrow(kind)) => {
            let range = ssa.ranges[index];
            let range = if ssa.integral_bits[index] != 0 && ssa.integral_bits[index] & 4 == 0 {
                range.and_then(crate::p5_types::IntRange::known01)
            } else {
                range
            };
            if range.is_some_and(|range| range.within(kind.range())) {
                Some(ValueUse::Narrow { value, kind })
            } else {
                None
            }
        }
        _ => None,
    })
}
pub(super) fn type_name(
    owner: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<String>, RequestError> {
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    let mut output = TextBuffer::new();
    for (index, segment) in owner[1..owner.len() - 1]
        .split(|&unit| unit == 47)
        .enumerate()
    {
        budget.charge_work(1)?;
        let Some(segment) = text(segment, budget)? else {
            return Ok(None);
        };
        if index != 0 {
            output.push(".", budget)?;
        }
        output.push(&segment, budget)?;
    }
    Ok(Some(output.finish()))
}
pub(super) fn identifier_text(
    units: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<String>, RequestError> {
    text(units, budget)
}
fn plan(
    ssa: &VerifiedSsa<'_>,
    id: CallId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<StaticCall>, RequestError> {
    let site = ssa
        .calls
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "owned call lost original site",
        ))?;
    let bound = ssa.bound_calls[ix(id.0)?]
        .as_ref()
        .ok_or(RequestError::InternalInvariant("owned call lost arguments"))?;
    let owner = &site.contract.owner;
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    let mut qualifier = TextBuffer::new();
    for (index, segment) in owner[1..owner.len() - 1]
        .split(|&unit| unit == 47)
        .enumerate()
    {
        budget.charge_work(1)?;
        let Some(segment) = text(segment, budget)? else {
            return Ok(None);
        };
        if index != 0 {
            qualifier.push(".", budget)?;
        }
        qualifier.push(&segment, budget)?;
    }
    let Some(method) = text(&site.contract.name, budget)? else {
        return Ok(None);
    };
    if bound.arguments.len() != site.contract.formals.len() {
        return Err(RequestError::InternalInvariant("owned call changed arity"));
    }
    let mut arguments = Vec::new();
    for (&value, &formal) in bound.arguments.iter().zip(&site.contract.formals) {
        budget.charge_work(1)?;
        let Some(view) = argument(ssa, LocalId(value.0), formal)? else {
            return Ok(None);
        };
        push(&mut arguments, view, budget)?;
    }
    Ok(Some(StaticCall {
        _sealed: (),
        reference: site.contract.reference,
        original_owner: copy(&site.contract.owner, budget)?,
        original_name: copy(&site.contract.name, budget)?,
        prototype: copy(&site.contract.prototype, budget)?,
        invoke: site.pc,
        result_site: site.result_pc,
        qualifier: qualifier.finish(),
        method,
        arguments,
    }))
}
pub(super) fn operations(
    ssa: &VerifiedSsa<'_>,
    block: crate::pipeline::branch::BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Statement>>, RequestError> {
    let mut output = Vec::new();
    for op in &ssa.bound[ix(block.0)?].ops {
        budget.charge_work(1)?;
        if let Op::Call(id, _) = op.op {
            let Some(call) = plan(ssa, id, budget)? else {
                return Ok(None);
            };
            push(
                &mut output,
                Statement::StaticCall {
                    target: op.result.map(|value| LocalId(value.0)),
                    call,
                },
                budget,
            )?;
        } else if let Some(assignment) = super::branch::operation(op)? {
            push(&mut output, Statement::Assignment(assignment), budget)?;
        }
    }
    Ok(Some(output))
}
