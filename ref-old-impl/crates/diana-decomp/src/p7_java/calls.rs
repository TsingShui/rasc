use super::branch::{Assignment, LocalId, ValueUse};
use crate::p2_flow::{CallId, Op};
use crate::p4_types::TypedBody;
use crate::substrate::owned::{TextBuffer, reserve};
use crate::{CodeUnitOffset, MethodId, RequestError, RunContext};

/// Original invocation kind retained by the readable reconstruction.
#[derive(Clone, Copy, Debug)]
pub enum JavaInvokeKind {
    Super,
    Direct,
    Static,
    Virtual,
    Interface,
}
/// Borrowed original call identity in an owned readable method AST.
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
    Value(ValueUse),
    Super(LocalId),
}
impl CallReceiver {
    pub(crate) fn value(self) -> LocalId {
        match self {
            Self::Value(value) => value.value(),
            Self::Super(value) => value,
        }
    }
}
pub(crate) struct ReceiverCall {
    pub contract: crate::p4_types::declarations::ReceiverContract,
    pub receiver: CallReceiver,
    pub original_receiver: crate::RegisterId,
    pub invoke: CodeUnitOffset,
    pub result_site: Option<CodeUnitOffset>,
    pub method: String,
    pub arguments: Vec<ValueUse>,
}
pub(crate) enum Statement {
    Monitor {
        operation: crate::p2_flow::OpId,
        kind: crate::p2_flow::MonitorKind,
        lock: ValueUse,
    },
    Class {
        target: LocalId,
        reference: crate::TypeId,
        source: String,
    },
    Snapshot(crate::p5_variables::exceptional::Snapshot),
    ExceptionBind {
        handler: crate::p2_flow::BlockId,
        target: LocalId,
    },
    Deferred(crate::p4_types::initialization::InitializationId),
    Initialize(super::object_allocation::Initialization),
    ConstructorChain(crate::MethodId),
    NewArray(super::fresh_fill::Allocation),
    FreshFill(super::fresh_fill::Fill),
    EmptyFill {
        array: LocalId,
        result: LocalId,
        payload: CodeUnitOffset,
    },
    TypeCheck(super::cast_ops::Statement),
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
fn push<T>(items: &mut Vec<T>, item: T, budget: &mut RunContext<'_>) -> Result<(), RequestError> {
    reserve(items, 1, budget)?;
    items.push(item);
    Ok(())
}
fn copy(units: &[u16], budget: &mut RunContext<'_>) -> Result<Vec<u16>, RequestError> {
    let mut out = Vec::new();
    reserve(&mut out, units.len(), budget)?;
    for &unit in units {
        budget.checkpoint()?;
        out.push(unit);
    }
    Ok(out)
}
fn text(
    units: &[u16],
    budget: &mut impl crate::pipeline::control::InputControl,
) -> Result<Option<String>, RequestError> {
    if !super::declarations::identifier(units, budget)? {
        return Ok(None);
    }
    let mut output = TextBuffer::new();
    for scalar in char::decode_utf16(units.iter().copied()) {
        budget.checkpoint()?;
        output.character(
            scalar.map_err(|_| {
                RequestError::InternalInvariant("checked identifier lost its scalar")
            })?,
            budget,
        )?;
    }
    Ok(Some(output.finish()))
}
pub(super) fn type_name(
    owner: &[u16],
    budget: &mut impl crate::pipeline::control::InputControl,
) -> Result<Option<String>, RequestError> {
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return Ok(None);
    }
    let mut output = TextBuffer::new();
    for (index, segment) in owner[1..owner.len() - 1]
        .split(|&unit| unit == 47)
        .enumerate()
    {
        budget.checkpoint()?;
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
    budget: &mut RunContext<'_>,
) -> Result<Option<String>, RequestError> {
    text(units, budget)
}
pub(super) struct SourceCalls {
    pub receivers: Vec<Option<crate::p4_types::declarations::ReceiverContract>>,
    pub constructors: Vec<Option<crate::p4_types::declarations::allocation::ConstructorContract>>,
    pub throws: crate::p4_types::bindings::SourceThrows,
}
pub(super) fn validate_bindings(
    input: &crate::input::MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    ssa: &TypedBody<'_>,
    uncertainties: &mut Vec<crate::OutputReason>,
    budget: &mut RunContext<'_>,
) -> Result<Option<SourceCalls>, RequestError> {
    let Some(throws) = &ssa.source_throws else {
        return Ok(None);
    };
    let throws = throws.retained(budget)?;
    let mut receivers = Vec::new();
    let mut constructors = Vec::new();
    for (index, site) in ssa.ssa.calls.iter().enumerate() {
        budget.checkpoint()?;
        let original = input.method_reference(site.reference, budget)?;
        let owner = crate::substrate::owned::copy_units(original.owner, budget)?;
        if catalog.class(&owner, budget)?.is_none() {
            super::method::note(
                uncertainties,
                "call_declaration_unknown",
                Some(site.pc),
                budget,
            )?;
        }
        let id = CallId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let invocation = crate::p4_types::declarations::throws::Invocation {
            reference: site.reference,
            pc: site.pc,
        };
        let bound = ssa.ssa.call(id)?;
        let uses = ssa
            .call_uses
            .get(index)
            .ok_or(RequestError::InternalInvariant(
                "call lost final use conversions",
            ))?;
        if site.target == crate::p2_flow::CallTarget::Constructor {
            let initialization = ssa
                .initializations
                .calls
                .get(index)
                .and_then(|id| *id)
                .ok_or(RequestError::InternalInvariant(
                    "constructor lost initialization state",
                ))?;
            let (object, use_kind) = match initialization {
                crate::p4_types::initialization::InitializationId::Allocation(_) => {
                    let plan = ssa
                        .initializations
                        .plans
                        .get(initialization.index()?)
                        .ok_or(RequestError::InternalInvariant(
                            "constructor lost allocation plan",
                        ))?;
                    (
                        plan.object,
                        crate::p4_types::declarations::allocation::ConstructorUse::Allocation,
                    )
                }
                crate::p4_types::initialization::InitializationId::This => {
                    let chain = ssa.initializations.chain.as_ref().ok_or(
                        RequestError::InternalInvariant("constructor lost UninitThis plan"),
                    )?;
                    let use_kind = match chain.kind {
                        crate::p4_types::initialization::ChainKind::This => {
                            crate::p4_types::declarations::allocation::ConstructorUse::This
                        }
                        crate::p4_types::initialization::ChainKind::Super => {
                            crate::p4_types::declarations::allocation::ConstructorUse::Super
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
            if !matches!(receiver, crate::p4_types::calls::TypedReceiver::Constructor { value, initialization: actual } if *value == bound.input(0)? && *actual == initialization)
            {
                return Err(RequestError::InternalInvariant(
                    "constructor receiver became readable before init",
                ));
            }
            let owner =
                crate::substrate::owned::copy_units(input.type_name(object, budget)?, budget)?;
            let Some(crate::p4_types::bindings::CallEvidence::Constructor { declarations, .. }) =
                ssa.bindings.get(index)
            else {
                return Err(RequestError::InternalInvariant(
                    "constructor lost P4 declaration product",
                ));
            };
            let contract = declarations.bind(site.reference, site.pc, &owner, use_kind, budget)?;
            if uses.arguments().len() != contract.arity {
                return Err(RequestError::InternalInvariant("constructor changed arity"));
            }
            push(&mut constructors, Some(contract), budget)?;
            push(&mut receivers, None, budget)?;
            continue;
        }
        push(&mut constructors, None, budget)?;
        let (receiver, arguments) = match &uses.receiver {
            Some(receiver) => {
                let value = receiver.value();
                if site.kind == crate::p2_flow::InvokeKind::Static
                    || bound.inputs.first() != Some(&value)
                {
                    return Err(RequestError::InternalInvariant(
                        "receiver conversion changed original definition",
                    ));
                }
                let descriptor = match receiver {
                    crate::p4_types::calls::TypedReceiver::Super { .. } => {
                        crate::substrate::owned::copy_units(input.owner(), budget)?
                    }
                    crate::p4_types::calls::TypedReceiver::Value(usage) => {
                        if usage.conversion == crate::p4_types::UseConversion::NullAssignment {
                            crate::substrate::owned::copy_units(
                                input.type_descriptor(site.owner, budget)?,
                                budget,
                            )?
                        } else {
                            ssa.kinds[ix(value.0)?]
                                .ok_or(RequestError::InternalInvariant(
                                    "receiver has no storage kind",
                                ))?
                                .reference_descriptor(input, budget)?
                        }
                    }
                    crate::p4_types::calls::TypedReceiver::Constructor { .. } => {
                        return Err(RequestError::InternalInvariant(
                            "ordinary call used constructor receiver",
                        ));
                    }
                };
                if (site.kind == crate::p2_flow::InvokeKind::Super)
                    != matches!(
                        receiver,
                        crate::p4_types::calls::TypedReceiver::Super { .. }
                    )
                {
                    return Err(RequestError::InternalInvariant(
                        "super receiver lost this origin",
                    ));
                }
                let Some(crate::p4_types::bindings::CallEvidence::Receiver { declaration, .. }) =
                    ssa.bindings.get(index)
                else {
                    return Err(RequestError::InternalInvariant(
                        "receiver lost P4 declaration product",
                    ));
                };
                let declaration =
                    declaration
                        .as_ref()
                        .ok_or(RequestError::BindingEvidenceMissing {
                            reference: site.reference,
                            pc: site.pc,
                        })?;
                let contract = declaration
                    .bind_source(input, catalog, &descriptor, budget)?
                    .ok_or(RequestError::BindingEvidenceMissing {
                        reference: site.reference,
                        pc: site.pc,
                    })?;
                (Some(contract), &bound.inputs[1..])
            }
            None => {
                if site.kind != crate::p2_flow::InvokeKind::Static {
                    return Err(RequestError::InternalInvariant(
                        "receiver call lost typed receiver",
                    ));
                }
                let contract = ssa.contract(id)?;
                if !crate::p4_types::bindings::binding_available(
                    input,
                    catalog,
                    contract,
                    &invocation,
                    budget,
                )? {
                    super::method::note(
                        uncertainties,
                        "call_source_binding",
                        Some(site.pc),
                        budget,
                    )?;
                }
                (None, bound.inputs.as_slice())
            }
        };
        let views = uses.arguments();
        if views.len() != arguments.len() {
            return Err(RequestError::InternalInvariant(
                "call conversions changed arity",
            ));
        }
        for (&value, typed_use) in arguments.iter().zip(views) {
            budget.checkpoint()?;
            if value != typed_use.value {
                return Err(RequestError::InternalInvariant(
                    "call conversion changed definition or order",
                ));
            }
        }
        push(&mut receivers, receiver, budget)?;
    }
    let result = SourceCalls {
        receivers,
        constructors,
        throws,
    };
    Ok(Some(result))
}
fn plan(
    ssa: &TypedBody<'_>,
    id: CallId,
    budget: &mut RunContext<'_>,
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
        budget.checkpoint()?;
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
    if bound.inputs.len() != site.formals.len() {
        return Err(RequestError::InternalInvariant("owned call changed arity"));
    }
    let uses = ssa
        .call_uses
        .get(ix(id.0)?)
        .ok_or(RequestError::InternalInvariant("call lost typed uses"))?;
    let mut arguments = Vec::new();
    for typed_use in uses.arguments() {
        budget.checkpoint()?;
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
    storage: &crate::p5_variables::StorageBody<'_>,
    block: crate::p2_flow::BlockId,
    bindings: &mut SourceCalls,
    names: &super::types::TypeNames,
    placements: &[super::exceptional::Placement<'_>],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<Statement>>, RequestError> {
    let ssa = &storage.typed;
    let mut output = Vec::new();
    for &id in &ssa.ssa.bound[ix(block.0)?].ops {
        budget.checkpoint()?;
        let op = ssa.ssa.operation(id)?;
        if matches!(&ssa.capture, Some(capture) if capture.operation() == id) {
            continue;
        }
        super::exceptional::snapshots(placements, block, op.pc, &mut output, budget)?;
        if matches!(op.op, Op::Monitor(..)) {
            push(&mut output, super::monitor_ops::lower(ssa, id, op)?, budget)?;
            continue;
        }
        if let Op::Class(_, reference) = op.op {
            let mut source = TextBuffer::new();
            source.push(
                names.name(crate::p4_types::Kind::Nominal(reference), budget)?,
                budget,
            )?;
            let target = LocalId(
                op.result
                    .ok_or(RequestError::InternalInvariant("class literal lost result"))?
                    .0,
            );
            push(
                &mut output,
                Statement::Class {
                    target,
                    reference,
                    source: source.finish(),
                },
                budget,
            )?;
            continue;
        }
        if let Op::Exception(_) = op.op {
            let target = LocalId(
                op.result
                    .ok_or(RequestError::InternalInvariant("catch lost value"))?
                    .0,
            );
            push(
                &mut output,
                Statement::ExceptionBind {
                    handler: block,
                    target,
                },
                budget,
            )?;
            let ingress = super::exceptional::placed_ingress(placements, block, budget)?;
            for copy in &ingress.copies.steps {
                budget.checkpoint()?;
                push(
                    &mut output,
                    Statement::Assignment(Assignment {
                        target: copy.target,
                        expression: super::branch::copy_expression(copy.source),
                    }),
                    budget,
                )?;
            }
            continue;
        }
        if let Op::Initialize(call) = op.op {
            if ssa.initializations.calls.get(ix(call.0)?)
                == Some(&Some(
                    crate::p4_types::initialization::InitializationId::This,
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
        } else if matches!(op.op, Op::NewArray(..) | Op::FilledArray(..)) {
            push(
                &mut output,
                Statement::NewArray(super::fresh_fill::allocate(storage, op, names, budget)?),
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
        } else if matches!(op.op, Op::Cast(..) | Op::InstanceOf(..)) {
            push(
                &mut output,
                Statement::TypeCheck(super::cast_ops::lower(ssa, op, names, budget)?),
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
        } else if matches!(
            op.op,
            Op::FieldLoad(..) | Op::FieldStore(..) | Op::StaticLoad(..) | Op::StaticStore(..)
        ) {
            let Some(statement) = super::field_ops::lower(ssa, names, op, budget)? else {
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
                for argument in ssa.call_uses[index].arguments() {
                    budget.checkpoint()?;
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
                            receiver: match ssa.call_uses[index]
                                .receiver
                                .as_ref()
                                .ok_or(RequestError::InternalInvariant("call lost receiver use"))?
                            {
                                crate::p4_types::calls::TypedReceiver::Super { .. } => {
                                    CallReceiver::Super(LocalId(receiver.0))
                                }
                                crate::p4_types::calls::TypedReceiver::Value(usage) => {
                                    CallReceiver::Value(super::values::lower(
                                        LocalId(usage.value.0),
                                        usage.conversion,
                                    ))
                                }
                                crate::p4_types::calls::TypedReceiver::Constructor { .. } => {
                                    return Err(RequestError::InternalInvariant(
                                        "ordinary call lost receiver category",
                                    ));
                                }
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
        } else if let Some(assignment) = super::branch::operation(ssa, op, budget)? {
            push(&mut output, Statement::Assignment(assignment), budget)?;
        }
    }
    if matches!(
        ssa.ssa.blocks[ix(block.0)?].terminal,
        crate::p2_flow::Terminal::Throw(_)
    ) {
        super::exceptional::snapshots(
            placements,
            block,
            ssa.ssa.blocks[ix(block.0)?].terminal_pc,
            &mut output,
            budget,
        )?;
    }
    Ok(Some(output))
}
