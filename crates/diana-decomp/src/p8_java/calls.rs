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
#[derive(Clone, Copy)]
pub(crate) enum CallReceiver {
    Value(LocalId),
    Super(LocalId),
}
impl CallReceiver {
    pub(crate) fn value(self) -> LocalId {
        match self {
            Self::Value(value) | Self::Super(value) => value,
        }
    }
}
pub(crate) struct ReceiverCall {
    pub contract: crate::p4_calls::ReceiverContract,
    pub receiver: CallReceiver,
    pub original_receiver: crate::RegisterId,
    pub invoke: CodeUnitOffset,
    pub result_site: Option<CodeUnitOffset>,
    pub method: String,
    pub arguments: Vec<ValueUse>,
}
pub(crate) enum Statement {
    Deferred(crate::p5_types::initialization::InitializationId),
    Initialize(super::object_allocation::Initialization),
    ConstructorChain(crate::MethodId),
    NewArray(super::fresh_fill::Allocation),
    FreshFill(super::fresh_fill::Fill),
    EmptyFill {
        array: LocalId,
        result: LocalId,
        payload: CodeUnitOffset,
    },
    Cast(super::cast_ops::Statement),
    String {
        target: LocalId,
        reference: crate::StringId,
        units: Vec<u16>,
    },
    ReceiverCall {
        target: Option<LocalId>,
        call: ReceiverCall,
    },
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
pub(super) struct SourceCalls {
    pub receivers: Vec<Option<crate::p4_calls::ReceiverContract>>,
    pub constructors: Vec<Option<crate::p4_calls::allocation::ConstructorContract>>,
    pub throws: crate::p4_calls::normal::SourceThrows,
}
pub(super) fn validate_bindings(
    input: &crate::input::MethodInput<'_, '_>,
    catalog: &crate::DeclarationCatalog,
    ssa: &TypedBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<SourceCalls>, RequestError> {
    let Some(throws) = crate::p4_calls::source_throws(input, catalog, budget)? else {
        return Ok(None);
    };
    let mut receivers = Vec::new();
    let mut constructors = Vec::new();
    for (index, site) in ssa.ssa.calls.iter().enumerate() {
        budget.charge_work(1)?;
        let id = CallId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let bound = ssa.ssa.call(id)?;
        let uses = ssa
            .call_uses
            .get(index)
            .ok_or(RequestError::InternalInvariant(
                "call lost final use conversions",
            ))?;
        if site.target == crate::p2_flow::normal::CallTarget::Constructor {
            let initialization = ssa
                .initializations
                .calls
                .get(index)
                .and_then(|id| *id)
                .ok_or(RequestError::InternalInvariant(
                    "constructor lost initialization state",
                ))?;
            let (object, use_kind) = match initialization {
                crate::p5_types::initialization::InitializationId::Allocation(_) => {
                    let plan = ssa
                        .initializations
                        .plans
                        .get(initialization.index()?)
                        .ok_or(RequestError::InternalInvariant(
                            "constructor lost allocation plan",
                        ))?;
                    (
                        plan.object,
                        crate::p4_calls::allocation::ConstructorUse::Allocation,
                    )
                }
                crate::p5_types::initialization::InitializationId::This => {
                    let chain = ssa.initializations.chain.as_ref().ok_or(
                        RequestError::InternalInvariant("constructor lost UninitThis plan"),
                    )?;
                    let use_kind = match chain.kind {
                        crate::p5_types::initialization::ChainKind::This => {
                            crate::p4_calls::allocation::ConstructorUse::This
                        }
                        crate::p5_types::initialization::ChainKind::Super => {
                            crate::p4_calls::allocation::ConstructorUse::Super
                        }
                    };
                    (site.owner, use_kind)
                }
            };
            let receiver = uses
                .receiver
                .as_ref()
                .ok_or(RequestError::InternalInvariant(
                    "constructor lost receiver use",
                ))?;
            if receiver.identity != crate::p5_types::calls::ReceiverIdentity::Uninitialized
                || bound.input(0)? != receiver.value
            {
                return Err(RequestError::InternalInvariant(
                    "constructor receiver became readable before init",
                ));
            }
            let owner =
                crate::substrate::owned::copy_units(input.type_name(object, budget)?, budget)?;
            let contract = crate::p4_calls::allocation::resolve(
                input,
                catalog,
                site.reference,
                &owner,
                use_kind,
                site.pc,
                budget,
            )?;
            if matches!(use_kind, crate::p4_calls::allocation::ConstructorUse::This) {
                crate::p4_calls::allocation::prove_this_chain(
                    input,
                    catalog,
                    site.reference,
                    site.pc,
                    budget,
                )?;
            }
            if !throws.covers(catalog, &contract.declared_throws, budget)? {
                return Ok(None);
            }
            if uses.arguments(site.reference, site.pc)?.len() != contract.int_arity {
                return Err(RequestError::InternalInvariant("constructor changed arity"));
            }
            push(&mut constructors, Some(contract), budget)?;
            push(&mut receivers, None, budget)?;
            continue;
        }
        push(&mut constructors, None, budget)?;
        let (receiver, arguments) = match &uses.receiver {
            Some(receiver) => {
                if site.kind == crate::p2_flow::normal::InvokeKind::Static
                    || bound.inputs.first() != Some(&receiver.value)
                    || ssa.kinds[ix(receiver.value.0)?]
                        != Some(crate::p5_types::normal::Kind::Nominal(
                            receiver.original_type,
                        ))
                {
                    return Err(RequestError::InternalInvariant(
                        "receiver conversion changed original definition",
                    ));
                }
                let descriptor = crate::substrate::owned::copy_units(
                    input.type_descriptor(receiver.original_type, budget)?,
                    budget,
                )?;
                if site.kind == crate::p2_flow::normal::InvokeKind::Super
                    && receiver.identity != crate::p5_types::calls::ReceiverIdentity::ImplicitThis
                {
                    return Err(RequestError::InternalInvariant(
                        "super receiver lost this origin",
                    ));
                }
                let kind = match site.kind {
                    crate::p2_flow::normal::InvokeKind::Direct => {
                        crate::p4_calls::ReceiverKind::Direct
                    }
                    crate::p2_flow::normal::InvokeKind::Super => {
                        crate::p4_calls::ReceiverKind::Super
                    }
                    crate::p2_flow::normal::InvokeKind::Virtual => {
                        crate::p4_calls::ReceiverKind::Virtual
                    }
                    crate::p2_flow::normal::InvokeKind::Interface => {
                        crate::p4_calls::ReceiverKind::Interface
                    }
                    crate::p2_flow::normal::InvokeKind::Static => {
                        return Err(RequestError::InternalInvariant(
                            "static call acquired receiver",
                        ));
                    }
                };
                let contract = crate::p4_calls::resolve_receiver(
                    input,
                    catalog,
                    site.reference,
                    kind,
                    &descriptor,
                    false,
                    budget,
                )?
                .ok_or(RequestError::BindingEvidenceMissing {
                    reference: site.reference,
                    pc: site.pc,
                })?;
                if !throws.covers(catalog, &contract.declared_throws, budget)? {
                    return Ok(None);
                }
                (Some(contract), &bound.inputs[1..])
            }
            None => {
                if site.kind != crate::p2_flow::normal::InvokeKind::Static {
                    return Err(RequestError::InternalInvariant(
                        "receiver call lost typed receiver",
                    ));
                }
                let contract = ssa.contract(id)?;
                if !crate::p4_calls::normal::binding_available(input, catalog, contract, budget)? {
                    return Err(RequestError::BindingEvidenceMissing {
                        reference: site.reference,
                        pc: site.pc,
                    });
                }
                (None, bound.inputs.as_slice())
            }
        };
        let views = uses.arguments(site.reference, site.pc)?;
        if views.len() != arguments.len() {
            return Err(RequestError::InternalInvariant(
                "call conversions changed arity",
            ));
        }
        for (&value, typed_use) in arguments.iter().zip(views) {
            budget.charge_work(1)?;
            if value != typed_use.value {
                return Err(RequestError::InternalInvariant(
                    "call conversion changed definition or order",
                ));
            }
        }
        push(&mut receivers, receiver, budget)?;
    }
    Ok(Some(SourceCalls {
        receivers,
        constructors,
        throws,
    }))
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
    for typed_use in uses.arguments(site.reference, site.pc)? {
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
    storage: &crate::p6_variables::normal::StorageBody<'_>,
    block: crate::pipeline::branch::BlockId,
    bindings: &mut SourceCalls,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Statement>>, RequestError> {
    let ssa = &storage.typed;
    let mut output = Vec::new();
    for &id in &ssa.ssa.bound[ix(block.0)?].ops {
        budget.charge_work(1)?;
        let op = ssa.ssa.operation(id)?;
        if let Op::Initialize(call) = op.op {
            if ssa.initializations.calls.get(ix(call.0)?)
                == Some(&Some(
                    crate::p5_types::initialization::InitializationId::This,
                ))
            {
                push(
                    &mut output,
                    Statement::ConstructorChain(ssa.ssa.calls[ix(call.0)?].reference),
                    budget,
                )?;
                continue;
            }
            let contract = bindings
                .constructors
                .get_mut(ix(call.0)?)
                .and_then(Option::take)
                .ok_or(RequestError::InternalInvariant(
                    "constructor lost source binding",
                ))?;
            let Some(initialization) =
                super::object_allocation::lower(storage, op, contract, budget)?
            else {
                return Ok(None);
            };
            push(&mut output, Statement::Initialize(initialization), budget)?;
        } else if let Some(result) = op.result
            && let Some(Some(id)) = ssa.initializations.deferred.get(ix(result.0)?)
        {
            push(&mut output, Statement::Deferred(*id), budget)?;
        } else if matches!(op.op, Op::NewArray(..)) {
            push(
                &mut output,
                Statement::NewArray(super::fresh_fill::allocate(ssa, op)?),
                budget,
            )?;
        } else if matches!(op.op, Op::FreshFill(..)) {
            push(
                &mut output,
                Statement::FreshFill(super::fresh_fill::fill(storage, op, budget)?),
                budget,
            )?;
        } else if let Op::EmptyFill(_, id) = op.op {
            let proof =
                storage
                    .empty_fills
                    .get(ix(id.0)?)
                    .ok_or(RequestError::InternalInvariant(
                        "empty fill lost LengthOnly storage",
                    ))?;
            push(
                &mut output,
                Statement::EmptyFill {
                    array: LocalId(proof.array.0),
                    result: proof.result,
                    payload: proof.payload,
                },
                budget,
            )?;
        } else if matches!(op.op, Op::Cast(..)) {
            push(
                &mut output,
                Statement::Cast(super::cast_ops::lower(ssa, op)?),
                budget,
            )?;
        } else if let Op::String(_, literal) = op.op {
            let original =
                ssa.ssa
                    .strings
                    .get(ix(literal.0)?)
                    .ok_or(RequestError::InternalInvariant(
                        "string lost original value",
                    ))?;
            let units = crate::substrate::owned::copy_units(original.value, budget)?;
            super::declarations::check_string_literal(&units, budget)?;
            let target = LocalId(
                op.result
                    .ok_or(RequestError::InternalInvariant("string lost result"))?
                    .0,
            );
            push(
                &mut output,
                Statement::String {
                    target,
                    reference: original.reference,
                    units,
                },
                budget,
            )?;
        } else if matches!(op.op, Op::FieldLoad(..) | Op::FieldStore(..)) {
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
            let index = ix(id.0)?;
            if let Some(contract) = bindings
                .receivers
                .get_mut(index)
                .ok_or(RequestError::InternalInvariant("call lost binding slot"))?
                .take()
            {
                let bound = ssa.ssa.call(id)?;
                let site = &ssa.ssa.calls[index];
                let Some(method) = identifier_text(&contract.name, budget)? else {
                    return Ok(None);
                };
                let mut arguments = Vec::new();
                for argument in ssa.call_uses[index].arguments(site.reference, site.pc)? {
                    budget.charge_work(1)?;
                    push(
                        &mut arguments,
                        super::values::lower(LocalId(argument.value.0), argument.conversion),
                        budget,
                    )?;
                }
                let receiver = bound.input(0)?;
                let original_receiver =
                    *site
                        .registers
                        .first()
                        .ok_or(RequestError::InternalInvariant(
                            "call lost original receiver register",
                        ))?;
                push(
                    &mut output,
                    Statement::ReceiverCall {
                        target: op.result.map(|value| LocalId(value.0)),
                        call: ReceiverCall {
                            receiver: if matches!(
                                contract.kind,
                                crate::p4_calls::ReceiverKind::Super
                            ) {
                                CallReceiver::Super(LocalId(receiver.0))
                            } else {
                                CallReceiver::Value(LocalId(receiver.0))
                            },
                            contract,
                            original_receiver,
                            invoke: site.pc,
                            result_site: site.result_pc,
                            method,
                            arguments,
                        },
                    },
                    budget,
                )?;
                continue;
            }
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
        } else if let Some(assignment) = super::branch::operation(ssa, op)? {
            push(&mut output, Statement::Assignment(assignment), budget)?;
        }
    }
    Ok(Some(output))
}
