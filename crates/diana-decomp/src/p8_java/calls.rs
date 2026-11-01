use super::branch::{Assignment, LocalId, ValueUse};
use crate::pipeline::branch::{CallId, Op, TypedBody};
use crate::substrate::owned::{TextBuffer, reserve};
use crate::{CodeUnitOffset, MethodBudget, MethodId, RequestError};

/// Original invocation kind retained by verified source binding.
#[derive(Clone, Copy, Debug)]
pub enum JavaInvokeKind {
    Super,
    Direct,
    Static,
    Virtual,
    Interface,
}
/// Borrowed original call identity in an owned verified method AST.
pub struct JavaCallSite<'a> {
    pub reference: MethodId,
    pub receiver: Option<crate::RegisterId>,
    pub allocation: Option<CodeUnitOffset>,
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
    Field(super::field_ops::Statement),
    Array(super::array_ops::Statement),
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
pub(super) fn empty_static(
    contract: crate::p4_calls::StaticContract,
    invoke: CodeUnitOffset,
    result_site: Option<CodeUnitOffset>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<StaticCall>, RequestError> {
    if !contract.formals.is_empty()
        || !contract.declared_throws.is_empty()
        || !matches!(
            contract.result,
            crate::p4_calls::StaticResult::Int | crate::p4_calls::StaticResult::Void
        )
        || contract.result == crate::p4_calls::StaticResult::Void && result_site.is_some()
    {
        return Err(RequestError::InternalInvariant(
            "empty static call lost its checked signature",
        ));
    }
    let Some(qualifier) = type_name(&contract.owner, budget)? else {
        return Ok(None);
    };
    let Some(method) = identifier_text(&contract.name, budget)? else {
        return Ok(None);
    };
    Ok(Some(StaticCall {
        _sealed: (),
        reference: contract.reference,
        original_owner: contract.owner,
        original_name: contract.name,
        prototype: contract.prototype,
        invoke,
        result_site,
        qualifier,
        method,
        arguments: Vec::new(),
    }))
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
pub(super) fn validate_bindings(
    input: &crate::input::MethodInput<'_, '_>,
    catalog: &crate::DeclarationCatalog,
    ssa: &TypedBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for (index, site) in ssa.ssa.calls.iter().enumerate() {
        budget.charge_work(1)?;
        let id = CallId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let contract = ssa.contract(id)?;
        let bound = ssa.ssa.call(id)?;
        if bound.inputs.len() != contract.formals.len() {
            return Err(RequestError::InternalInvariant(
                "call constraints changed original word arity",
            ));
        }
        let uses = ssa
            .call_uses
            .get(index)
            .ok_or(RequestError::InternalInvariant(
                "call lost final use conversions",
            ))?;
        if uses.arguments.len() != bound.inputs.len() {
            return Err(RequestError::InternalInvariant(
                "call conversions changed arity",
            ));
        }
        for (&value, typed_use) in bound.inputs.iter().zip(&uses.arguments) {
            budget.charge_work(1)?;
            if value != typed_use.value {
                return Err(RequestError::InternalInvariant(
                    "call conversion changed definition or order",
                ));
            }
        }
        if !crate::p4_calls::normal::binding_available(input, catalog, contract, budget)? {
            return Err(RequestError::BindingEvidenceMissing {
                reference: site.reference,
                pc: site.pc,
            });
        }
    }
    Ok(true)
}
fn plan(
    ssa: &TypedBody<'_>,
    id: CallId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<StaticCall>, RequestError> {
    let site = ssa
        .ssa
        .calls
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant(
            "owned call lost original site",
        ))?;
    let bound = ssa.ssa.call(id)?;
    let contract = ssa.contract(id)?;
    let owner = &contract.owner;
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
    let Some(method) = text(&contract.name, budget)? else {
        return Ok(None);
    };
    if bound.inputs.len() != contract.formals.len() {
        return Err(RequestError::InternalInvariant("owned call changed arity"));
    }
    let uses = ssa
        .call_uses
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant("call lost typed uses"))?;
    let mut arguments = Vec::new();
    for typed_use in &uses.arguments {
        budget.charge_work(1)?;
        push(
            &mut arguments,
            super::values::lower(LocalId(typed_use.value.0), typed_use.conversion),
            budget,
        )?;
    }
    Ok(Some(StaticCall {
        _sealed: (),
        reference: contract.reference,
        original_owner: copy(&contract.owner, budget)?,
        original_name: copy(&contract.name, budget)?,
        prototype: copy(&contract.prototype, budget)?,
        invoke: site.pc,
        result_site: site.result_pc,
        qualifier: qualifier.finish(),
        method,
        arguments,
    }))
}
pub(super) fn operations(
    ssa: &TypedBody<'_>,
    block: crate::pipeline::branch::BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Statement>>, RequestError> {
    let mut output = Vec::new();
    for &id in &ssa.ssa.bound[ix(block.0)?].ops {
        budget.charge_work(1)?;
        let op = ssa.ssa.operation(id)?;
        if matches!(op.op, Op::FieldLoad(..) | Op::FieldStore(..)) {
            let Some(statement) = super::field_ops::lower(ssa, op, budget)? else {
                return Ok(None);
            };
            push(&mut output, Statement::Field(statement), budget)?;
        } else if matches!(
            op.op,
            Op::ArrayLength(..) | Op::ArrayLoad(..) | Op::ArrayStore(..)
        ) {
            push(
                &mut output,
                Statement::Array(super::array_ops::lower(ssa, op)?),
                budget,
            )?;
        } else if let Op::Call(id, _) = op.op {
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
