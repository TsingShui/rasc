use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, DeclarationCatalog, MethodBudget, RegisterId, RequestError};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ValueId(pub u32);
pub(crate) enum ValueType {
    Int,
    Reference(Vec<u16>),
}
pub(crate) enum Target {
    Constructor(crate::p4_calls::allocation::ConstructorContract),
    Static(crate::p4_calls::StaticContract),
    Receiver(crate::p4_calls::ReceiverContract),
}
impl Target {
    pub(crate) fn reference(&self) -> crate::MethodId {
        match self {
            Self::Constructor(c) => c.reference,
            Self::Static(c) => c.reference,
            Self::Receiver(c) => c.reference,
        }
    }
    pub(crate) fn owner(&self) -> &[u16] {
        match self {
            Self::Constructor(c) => &c.owner,
            Self::Static(c) => &c.owner,
            Self::Receiver(c) => &c.owner,
        }
    }
    pub(crate) fn name(&self) -> &[u16] {
        match self {
            Self::Constructor(_) => &[60, 105, 110, 105, 116, 62],
            Self::Static(c) => &c.name,
            Self::Receiver(c) => &c.name,
        }
    }
    pub(crate) fn prototype(&self) -> &[u16] {
        match self {
            Self::Constructor(c) => &c.prototype,
            Self::Static(c) => &c.prototype,
            Self::Receiver(c) => &c.prototype,
        }
    }
}
pub(crate) enum Operation {
    Construct {
        allocation: crate::p4_calls::allocation::AdjacentAllocation,
        arguments: Vec<ValueId>,
        value: ValueId,
    },
    Constant {
        pc: CodeUnitOffset,
        value: ValueId,
        literal: i32,
    },
    Call {
        pc: CodeUnitOffset,
        target: Target,
        receiver: Option<(ValueId, RegisterId)>,
        arguments: Vec<ValueId>,
        result: Option<(ValueId, CodeUnitOffset)>,
    },
}
pub(crate) struct Chain<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub operations: Vec<Operation>,
    pub types: Vec<ValueType>,
    pub returned: ValueId,
    pub declared_result: ValueType,
    pub return_pc: CodeUnitOffset,
}
fn push<T>(
    out: &mut Vec<T>,
    value: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(out, 1, budget)?;
    out.push(value);
    Ok(())
}
pub(crate) fn index(value: ValueId) -> Result<usize, RequestError> {
    usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)
}
fn slot(register: RegisterId) -> Result<usize, RequestError> {
    usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
}
fn read(
    slots: &[Option<ValueId>],
    register: RegisterId,
    pc: CodeUnitOffset,
) -> Result<ValueId, RequestError> {
    slots[slot(register)?].ok_or(RequestError::UndefinedRegister { register, pc })
}
fn define(
    types: &mut Vec<ValueType>,
    kind: ValueType,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ValueId, RequestError> {
    let id = ValueId(u32::try_from(types.len()).map_err(|_| RequestError::PlatformCapacity)?);
    push(types, kind, budget)?;
    Ok(id)
}
pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Chain<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 8 == 0
        || raw.ins_size() != 0
        || !raw.try_byte_range().as_range().is_empty()
    {
        return Ok(None);
    }
    let declared_result = if input
        .declaration()
        .prototype
        .return_type
        .matches_with(&[73], budget)?
    {
        ValueType::Int
    } else {
        let descriptor =
            crate::substrate::owned::copy_units(input.declaration().prototype.return_type, budget)?;
        if !crate::p4_calls::source_type(input, catalog, &descriptor, budget)? {
            return Ok(None);
        }
        ValueType::Reference(descriptor)
    };
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(None);
    }
    let mut operations = Vec::new();
    let mut types = Vec::new();
    let mut pc = CodeUnitOffset(0);
    let mut calls = 0usize;
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        if !checked.layout().is_reachable(pc) {
            return Ok(None);
        }
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return Ok(None);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "call chain lost checked register",
                ))
        };
        let mut next = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
        match spec.opcode {
            0 => {}
            0x12..=0x15 => {
                let literal = match operands.literal() {
                    Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => {
                        return Err(RequestError::InternalInvariant(
                            "call chain constant lost bits",
                        ));
                    }
                };
                let value = define(&mut types, ValueType::Int, budget)?;
                slots[slot(reg(0)?)?] = Some(value);
                push(
                    &mut operations,
                    Operation::Constant { pc, value, literal },
                    budget,
                )?;
            }
            0x22 => {
                let allocation =
                    crate::p4_calls::allocation::pair(input, &raw, pc, catalog, budget)?;
                let mut arguments = Vec::new();
                for &register in &allocation.arguments {
                    budget.charge_work(1)?;
                    let value = read(&slots, register, allocation.init)?;
                    if !matches!(types[index(value)?], ValueType::Int) {
                        return Err(RequestError::InitializationUnavailable {
                            pc: allocation.init,
                        });
                    }
                    push(&mut arguments, value, budget)?;
                }
                let mut descriptor = Vec::new();
                reserve(&mut descriptor, allocation.contract.owner.len(), budget)?;
                for &unit in &allocation.contract.owner {
                    budget.charge_work(1)?;
                    descriptor.push(unit);
                }
                let value = define(&mut types, ValueType::Reference(descriptor), budget)?;
                // Only normal init completion makes these aliases readable Refs.
                for &alias in &allocation.aliases {
                    budget.charge_work(1)?;
                    slots[slot(alias)?] = Some(value);
                }
                next = allocation.resume;
                calls = calls.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                push(
                    &mut operations,
                    Operation::Construct {
                        allocation,
                        arguments,
                        value,
                    },
                    budget,
                )?;
            }
            0x70 | 0x76 => {
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "direct call lost reference",
                    ));
                };
                let OperandReference::Method(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "direct call changed reference kind",
                    ));
                };
                if input
                    .method_reference(reference, budget)?
                    .declaration
                    .name
                    .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
                {
                    return Err(RequestError::InitializationUnavailable { pc });
                }
                return Ok(None);
            }
            0x6e | 0x72 | 0x74 | 0x78 | 0x71 | 0x77 => {
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant("call chain lost reference"));
                };
                let OperandReference::Method(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "call chain changed reference kind",
                    ));
                };
                let mut words = operands.registers().iter();
                let mut receiver = None;
                let (target, result_type) = if matches!(spec.opcode, 0x71 | 0x77) {
                    let Some(contract) =
                        crate::p4_calls::resolve_static(input, catalog, reference, budget)?
                    else {
                        return Err(RequestError::BindingEvidenceMissing { reference, pc });
                    };
                    if contract
                        .formals
                        .iter()
                        .any(|formal| !matches!(formal, crate::p4_calls::CallFormal::Int))
                    {
                        return Ok(None);
                    }
                    let kind = match contract.result {
                        crate::p4_calls::StaticResult::Int => Some(ValueType::Int),
                        crate::p4_calls::StaticResult::Void => None,
                        crate::p4_calls::StaticResult::Nominal => {
                            let Some(end) = contract.prototype.iter().position(|&unit| unit == 41)
                            else {
                                return Err(RequestError::InternalInvariant(
                                    "sealed prototype has no result",
                                ));
                            };
                            let mut descriptor = Vec::new();
                            for &unit in &contract.prototype[end + 1..] {
                                budget.charge_work(1)?;
                                push(&mut descriptor, unit, budget)?;
                            }
                            Some(ValueType::Reference(descriptor))
                        }
                        crate::p4_calls::StaticResult::Reference(_)
                        | crate::p4_calls::StaticResult::Boolean => return Ok(None),
                    };
                    (Target::Static(contract), kind)
                } else {
                    let register = words
                        .next()
                        .ok_or(RequestError::InternalInvariant("receiver call lost word"))?;
                    let value = read(&slots, register, pc)?;
                    let ValueType::Reference(descriptor) = &types[index(value)?] else {
                        return Ok(None);
                    };
                    let kind = if matches!(spec.opcode, 0x72 | 0x78) {
                        crate::p4_calls::ReceiverKind::Interface
                    } else {
                        crate::p4_calls::ReceiverKind::Virtual
                    };
                    let Some(contract) = crate::p4_calls::resolve_receiver(
                        input, catalog, reference, kind, descriptor, false, budget,
                    )?
                    else {
                        return Err(RequestError::BindingEvidenceMissing { reference, pc });
                    };
                    receiver = Some((value, register));
                    (Target::Receiver(contract), Some(ValueType::Int))
                };
                let mut arguments = Vec::new();
                for register in words {
                    budget.charge_work(1)?;
                    let value = read(&slots, register, pc)?;
                    if !matches!(types[index(value)?], ValueType::Int) {
                        return Ok(None);
                    }
                    push(&mut arguments, value, budget)?;
                }
                let arity = match &target {
                    Target::Static(c) => c.formals.len(),
                    Target::Constructor(_) => {
                        return Err(RequestError::InternalInvariant(
                            "constructor entered ordinary invoke",
                        ));
                    }
                    Target::Receiver(c) => c.int_arity,
                };
                if arguments.len() != arity {
                    return Err(RequestError::InternalInvariant(
                        "call chain changed checked arity",
                    ));
                }
                let mut result = None;
                if next.0 < raw.code_units().0 {
                    let following = instruction_layout(raw.instructions(), next, raw.version())
                        .map_err(crate::input::MethodError::Decode)?;
                    if matches!(following.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,0x0a|0x0c))
                    {
                        let Some(kind) = result_type else {
                            return Err(RequestError::InternalInvariant(
                                "void chain call has result",
                            ));
                        };
                        let expected = match kind {
                            ValueType::Int => 0x0a,
                            ValueType::Reference(_) => 0x0c,
                        };
                        if !matches!(following.kind(),LayoutKind::Instruction(spec) if spec.opcode==expected)
                        {
                            return Err(RequestError::InternalInvariant(
                                "checked chain result changed kind",
                            ));
                        }
                        let output = crate::input::operands::instruction_operands(
                            raw.instructions(),
                            next,
                            following,
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let destination = output.registers().iter().next().ok_or(
                            RequestError::InternalInvariant("chain result lost destination"),
                        )?;
                        let value = define(&mut types, kind, budget)?;
                        slots[slot(destination)?] = Some(value);
                        result = Some((value, next));
                        next = next
                            .checked_advance(following.width())
                            .ok_or(RequestError::PlatformCapacity)?;
                    }
                }
                calls = calls.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                push(
                    &mut operations,
                    Operation::Call {
                        pc,
                        target,
                        receiver,
                        arguments,
                        result,
                    },
                    budget,
                )?;
            }
            0x0f | 0x11 => {
                let returned = read(&slots, reg(0)?, pc)?;
                if calls == 0 {
                    return Ok(None);
                }
                match (&types[index(returned)?], &declared_result) {
                    (ValueType::Int, ValueType::Int) if spec.opcode == 0x0f => {}
                    (ValueType::Reference(from), ValueType::Reference(to))
                        if spec.opcode == 0x11
                            && crate::p4_calls::source_subtype(
                                input, catalog, from, to, budget,
                            )? => {}
                    _ => return Ok(None),
                }
                return Ok(Some(Chain {
                    _sealed: (),
                    raw,
                    operations,
                    types,
                    returned,
                    declared_result,
                    return_pc: pc,
                }));
            }
            _ => return Ok(None),
        }
        pc = next;
    }
    Ok(None)
}
