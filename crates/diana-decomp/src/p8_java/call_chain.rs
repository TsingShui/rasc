use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::call_chain::{Chain, Operation, Target, ValueId, ValueType, index};
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{CodeUnitOffset, EnvironmentId, MethodBudget, RequestError, VerifiedJavaMethod};

pub(crate) struct ResultLocal {
    pub value: ValueId,
    pub pc: CodeUnitOffset,
    pub source_type: String,
}
pub(crate) enum Statement {
    Constant { value: ValueId, literal: i32 },
    Call(CallId),
}
#[derive(Clone, Copy)]
pub(crate) struct CallId(pub u32);
pub(crate) enum CallForm {
    Invoke {
        receiver: Option<(ValueId, crate::RegisterId)>,
        result: Option<ResultLocal>,
        upcast: Option<String>,
    },
    Construct {
        binding: crate::p6_variables::allocation::DeferredAllocation,
        source_type: String,
    },
}
pub(crate) struct Call {
    pub pc: CodeUnitOffset,
    pub target: Target,
    pub form: CallForm,
    pub arguments: Vec<ValueId>,
    pub qualifier: Option<String>,
    pub method: String,
}
pub(crate) struct Body {
    pub calls: Vec<Call>,
    pub statements: Vec<Statement>,
    pub returned: ValueId,
}
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "call chain differs from original operations/definitions",
    ))
}
fn register(register: crate::RegisterId) -> Result<usize, RequestError> {
    usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
}
fn replay(
    input: &MethodInput<'_, '_>,
    catalog: &crate::DeclarationCatalog,
    chain: &Chain<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let raw = &chain.raw;
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(None);
    }
    let mut pc = CodeUnitOffset(0);
    let mut cursor = 0usize;
    let mut next_value = 0u32;
    while pc.0 <= chain.return_pc.0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return bad();
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "chain replay lost checked register",
                ))
        };
        let mut next = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
        match spec.opcode {
            0 => {}
            0x12..=0x15 => {
                let Some(Operation::Constant {
                    pc: origin,
                    value,
                    literal,
                }) = chain.operations.get(cursor)
                else {
                    return bad();
                };
                let original = match operands.literal() {
                    Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => return bad(),
                };
                if pc != *origin
                    || *literal != original
                    || value.0 != next_value
                    || !matches!(chain.types.get(index(*value)?), Some(ValueType::Int))
                {
                    return bad();
                }
                slots[register(reg(0)?)?] = Some(*value);
                next_value = next_value
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x22 => {
                let Some(Operation::Construct {
                    allocation,
                    arguments,
                    value,
                }) = chain.operations.get(cursor)
                else {
                    return bad();
                };
                if allocation.id.0 != pc || value.0 != next_value {
                    return bad();
                }
                super::allocation_verify::verify(
                    input, chain, allocation, arguments, *value, &mut slots, budget,
                )?;
                next = allocation.resume;
                next_value = next_value
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x6e | 0x72 | 0x74 | 0x78 | 0x71 | 0x77 => {
                let Some(Operation::Call {
                    pc: origin,
                    target,
                    receiver,
                    arguments,
                    result,
                }) = chain.operations.get(cursor)
                else {
                    return bad();
                };
                if *origin != pc
                    || !matches!(layout.references()[0],Some(reference) if matches!(reference.target,OperandReference::Method(id) if id==target.reference()))
                {
                    return bad();
                }
                let mut words = operands.registers().iter();
                match target {
                    Target::Constructor(_) => return bad(),
                    Target::Static(contract) => {
                        if !matches!(spec.opcode, 0x71 | 0x77)
                            || receiver.is_some()
                            || contract.formals.len() != arguments.len()
                        {
                            return bad();
                        }
                        for formal in &contract.formals {
                            budget.charge_work(1)?;
                            if !matches!(formal, crate::p4_calls::CallFormal::Int) {
                                return bad();
                            }
                        }
                    }
                    Target::Receiver(contract) => {
                        let kind = match contract.kind {
                            crate::p4_calls::ReceiverKind::Virtual => {
                                matches!(spec.opcode, 0x6e | 0x74)
                            }
                            crate::p4_calls::ReceiverKind::Interface => {
                                matches!(spec.opcode, 0x72 | 0x78)
                            }
                            _ => false,
                        };
                        let Some((value, receiver_register)) = receiver else {
                            return bad();
                        };
                        if !kind
                            || words.next() != Some(*receiver_register)
                            || slots[register(*receiver_register)?] != Some(*value)
                            || contract.int_arity != arguments.len()
                        {
                            return bad();
                        }
                        let Some(ValueType::Reference(descriptor)) =
                            chain.types.get(index(*value)?)
                        else {
                            return bad();
                        };
                        if !crate::p4_calls::source_subtype(
                            input,
                            catalog,
                            descriptor,
                            &contract.owner,
                            budget,
                        )? {
                            return bad();
                        }
                    }
                }
                for &argument in arguments {
                    budget.charge_work(1)?;
                    let Some(word) = words.next() else {
                        return bad();
                    };
                    if slots[register(word)?] != Some(argument)
                        || !matches!(chain.types.get(index(argument)?), Some(ValueType::Int))
                    {
                        return bad();
                    }
                }
                if words.next().is_some() {
                    return bad();
                }
                if let Some((value, origin)) = result {
                    if *origin != next || value.0 != next_value {
                        return bad();
                    }
                    let Some(kind) = chain.types.get(index(*value)?) else {
                        return bad();
                    };
                    match (target, kind) {
                        (Target::Static(c), ValueType::Int)
                            if c.result == crate::p4_calls::StaticResult::Int => {}
                        (Target::Receiver(_), ValueType::Int) => {}
                        (Target::Static(c), ValueType::Reference(descriptor))
                            if c.result == crate::p4_calls::StaticResult::Nominal =>
                        {
                            let Some(end) = c.prototype.iter().position(|&unit| unit == 41) else {
                                return bad();
                            };
                            if !crate::p0_knowledge::same_units(
                                &c.prototype[end + 1..],
                                descriptor,
                                budget,
                            )? {
                                return bad();
                            }
                        }
                        _ => return bad(),
                    }
                    let following = instruction_layout(raw.instructions(), next, raw.version())
                        .map_err(crate::input::MethodError::Decode)?;
                    let expected = match kind {
                        ValueType::Int => 0x0a,
                        ValueType::Reference(_) => 0x0c,
                    };
                    if !matches!(following.kind(),LayoutKind::Instruction(spec) if spec.opcode==expected)
                    {
                        return bad();
                    }
                    let output = crate::input::operands::instruction_operands(
                        raw.instructions(),
                        next,
                        following,
                    )
                    .map_err(crate::input::MethodError::Decode)?;
                    let Some(destination) = output.registers().iter().next() else {
                        return bad();
                    };
                    slots[register(destination)?] = Some(*value);
                    next_value = next_value
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                    next = next
                        .checked_advance(following.width())
                        .ok_or(RequestError::PlatformCapacity)?;
                }
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x0f | 0x11 => {
                if pc != chain.return_pc
                    || slots[register(reg(0)?)?] != Some(chain.returned)
                    || cursor != chain.operations.len()
                    || usize::try_from(next_value).map_err(|_| RequestError::PlatformCapacity)?
                        != chain.types.len()
                {
                    return bad();
                }
                match (&chain.types[index(chain.returned)?], &chain.declared_result) {
                    (ValueType::Int, ValueType::Int) if spec.opcode == 0x0f => {}
                    (ValueType::Reference(from), ValueType::Reference(to))
                        if spec.opcode == 0x11
                            && crate::p4_calls::source_subtype(
                                input, catalog, from, to, budget,
                            )? => {}
                    _ => return bad(),
                }
                return Ok(());
            }
            _ => return bad(),
        }
        pc = next;
    }
    bad()
}
pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    chain: Chain<'_>,
    catalog: &crate::DeclarationCatalog,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    super::declarations::check_target(input, budget)?;
    replay(input, catalog, &chain, budget)?;
    for operation in &chain.operations {
        budget.charge_work(1)?;
        if let Operation::Call { target, .. } = operation {
            let declared = match target {
                Target::Constructor(contract) => &contract.declared_throws,
                Target::Static(contract) => &contract.declared_throws,
                Target::Receiver(contract) => &contract.declared_throws,
            };
            if !crate::p4_calls::compatible_throws(input, catalog, Some(declared), budget)? {
                return Ok(None);
            }
        } else if let Operation::Construct { allocation, .. } = operation
            && !crate::p4_calls::compatible_throws(
                input,
                catalog,
                Some(&allocation.contract.declared_throws),
                budget,
            )?
        {
            return Ok(None);
        }
    }
    let RawPrintable::Method(raw) = input.raw() else {
        return bad();
    };
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        chain.raw.instructions(),
    ) {
        return bad();
    }
    if raw.annotations().parameter_range().is_some() {
        return Ok(None);
    }
    let flags = input.access_flags();
    if flags & 8 == 0
        || flags & !0x81f != 0
        || (flags & 7).count_ones() > 1
        || input.owner_access_flags() & 0x200 != 0
    {
        return Ok(None);
    }
    let name = copy_units(input.declaration().name, budget)?;
    let Some(name) = super::calls::identifier_text(&name, budget)? else {
        return Ok(None);
    };
    let mut header = TextBuffer::new();
    for (bit, text) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (0x10, "final "),
        (0x800, "strictfp "),
    ] {
        budget.charge_work(1)?;
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    match &chain.declared_result {
        ValueType::Int => header.push("int", budget)?,
        ValueType::Reference(descriptor) => {
            if !crate::p4_calls::source_type(input, catalog, descriptor, budget)? {
                return Ok(None);
            }
            let Some(name) = super::calls::type_name(descriptor, budget)? else {
                return Ok(None);
            };
            header.push(&name, budget)?;
        }
    }
    header.push(" ", budget)?;
    header.push(&name, budget)?;
    header.push("()", budget)?;
    let Some(throws) = crate::p4_calls::source_throws(input, catalog, budget)? else {
        return Ok(None);
    };
    for (index, descriptor) in throws.descriptors.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(name) = super::calls::type_name(descriptor, budget)? else {
            return Ok(None);
        };
        header.push(if index == 0 { " throws " } else { ", " }, budget)?;
        header.push(&name, budget)?;
    }
    let mut statements = Vec::new();
    let mut calls = Vec::new();
    for operation in chain.operations {
        budget.charge_work(1)?;
        let statement = match operation {
            Operation::Constant { value, literal, .. } => Statement::Constant { value, literal },
            Operation::Construct {
                allocation,
                arguments,
                value,
            } => {
                let binding = crate::p6_variables::allocation::bind(&allocation, value)?;
                let Some(source_type) =
                    super::calls::type_name(&allocation.contract.owner, budget)?
                else {
                    return Ok(None);
                };
                let Some(qualifier) = super::calls::type_name(&allocation.contract.owner, budget)?
                else {
                    return Ok(None);
                };
                let id =
                    CallId(u32::try_from(calls.len()).map_err(|_| RequestError::PlatformCapacity)?);
                reserve(&mut calls, 1, budget)?;
                calls.push(Call {
                    pc: allocation.init,
                    target: Target::Constructor(allocation.contract),
                    form: CallForm::Construct {
                        binding,
                        source_type,
                    },
                    arguments,
                    qualifier: Some(qualifier),
                    method: String::new(),
                });
                Statement::Call(id)
            }
            Operation::Call {
                pc,
                target,
                receiver,
                arguments,
                result,
            } => {
                let Some(method) = super::calls::identifier_text(target.name(), budget)? else {
                    return Ok(None);
                };
                let qualifier = match target {
                    Target::Static(_) => {
                        let Some(text) = super::calls::type_name(target.owner(), budget)? else {
                            return Ok(None);
                        };
                        Some(text)
                    }
                    Target::Receiver(_) => None,
                    Target::Constructor(_) => return bad(),
                };
                let result = if let Some((value, pc)) = result {
                    let source_type = match &chain.types[index(value)?] {
                        ValueType::Int => {
                            let mut text = TextBuffer::new();
                            text.push("int", budget)?;
                            text.finish()
                        }
                        ValueType::Reference(descriptor) => {
                            let Some(text) = super::calls::type_name(descriptor, budget)? else {
                                return Ok(None);
                            };
                            text
                        }
                    };
                    Some(ResultLocal {
                        value,
                        pc,
                        source_type,
                    })
                } else {
                    None
                };
                let upcast = if let Some((receiver, _)) = receiver {
                    let ValueType::Reference(descriptor) = &chain.types[index(receiver)?] else {
                        return bad();
                    };
                    if !crate::p0_knowledge::same_units(descriptor, target.owner(), budget)? {
                        let Some(name) = super::calls::type_name(target.owner(), budget)? else {
                            return Ok(None);
                        };
                        Some(name)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let id =
                    CallId(u32::try_from(calls.len()).map_err(|_| RequestError::PlatformCapacity)?);
                reserve(&mut calls, 1, budget)?;
                calls.push(Call {
                    pc,
                    target,
                    form: CallForm::Invoke {
                        receiver,
                        result,
                        upcast,
                    },
                    arguments,
                    qualifier,
                    method,
                });
                Statement::Call(id)
            }
        };
        reserve(&mut statements, 1, budget)?;
        statements.push(statement);
    }
    Ok(Some(VerifiedJavaMethod::call_chain(
        header.finish(),
        Body {
            calls,
            statements,
            returned: chain.returned,
        },
        environment,
    )))
}
