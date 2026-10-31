use crate::input::MethodInput;
use crate::p4_calls::allocation::AdjacentAllocation;
use crate::pipeline::call_chain::{Chain, ValueId, ValueType, index};
use crate::raw::{LayoutKind, OperandReference, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{MethodBudget, RegisterId, RequestError};
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "allocation storage differs from original new/init/aliases",
    ))
}
fn slot(register: RegisterId) -> Result<usize, RequestError> {
    usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
}
fn member(
    aliases: &[RegisterId],
    register: RegisterId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for &alias in aliases {
        budget.charge_work(1)?;
        if register == alias {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn verify(
    input: &MethodInput<'_, '_>,
    chain: &Chain<'_>,
    pair: &AdjacentAllocation,
    arguments: &[ValueId],
    value: ValueId,
    slots: &mut [Option<ValueId>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let raw = &chain.raw;
    let new = instruction_layout(raw.instructions(), pair.id.0, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(new.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x22) {
        return bad();
    }
    let Some(reference) = new.references()[0] else {
        return bad();
    };
    let OperandReference::Type(ty) = reference.target else {
        return bad();
    };
    if !input
        .type_name(ty, budget)?
        .matches_with(&pair.contract.owner, budget)?
    {
        return bad();
    }
    let Some(ValueType::Reference(descriptor)) = chain.types.get(index(value)?) else {
        return bad();
    };
    if !crate::p0_knowledge::same_units(descriptor, &pair.contract.owner, budget)? {
        return bad();
    }
    let original = crate::input::operands::instruction_operands(raw.instructions(), pair.id.0, new)
        .map_err(crate::input::MethodError::Decode)?;
    let Some(destination) = original.registers().iter().next() else {
        return bad();
    };
    let mut aliases = Vec::new();
    reserve(&mut aliases, 1, budget)?;
    aliases.push(destination);
    let mut pc = pair
        .id
        .0
        .checked_advance(new.width())
        .ok_or(RequestError::PlatformCapacity)?;
    while pc.0 < pair.init.0 {
        budget.charge_work(1)?;
        let op = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        if !matches!(op.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,7..=9)) {
            return bad();
        }
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, op)
            .map_err(crate::input::MethodError::Decode)?;
        let mut words = operands.registers().iter();
        let Some(destination) = words.next() else {
            return bad();
        };
        let Some(source) = words.next() else {
            return bad();
        };
        if !member(&aliases, source, budget)? {
            return bad();
        }
        if !member(&aliases, destination, budget)? {
            reserve(&mut aliases, 1, budget)?;
            aliases.push(destination);
        }
        pc = pc
            .checked_advance(op.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    if pc != pair.init
        || aliases.len() != pair.aliases.len()
        || arguments.len() != pair.contract.int_arity
    {
        return bad();
    }
    for (&actual, &expected) in aliases.iter().zip(&pair.aliases) {
        budget.charge_work(1)?;
        if actual != expected {
            return bad();
        }
    }
    let init = instruction_layout(raw.instructions(), pc, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(init.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,0x70|0x76))
        || pc.checked_advance(init.width()) != Some(pair.resume)
    {
        return bad();
    }
    if !matches!(init.references()[0],Some(reference) if matches!(reference.target,OperandReference::Method(id) if id==pair.contract.reference))
    {
        return bad();
    }
    let method = input.method_reference(pair.contract.reference, budget)?;
    if !method.owner.matches_with(&pair.contract.owner, budget)?
        || !method
            .declaration
            .name
            .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
        || !method
            .declaration
            .prototype
            .matches_with(&pair.contract.prototype, budget)?
    {
        return bad();
    }
    let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, init)
        .map_err(crate::input::MethodError::Decode)?;
    let mut words = operands.registers().iter();
    let Some(receiver) = words.next() else {
        return bad();
    };
    if receiver != pair.receiver
        || !member(&aliases, receiver, budget)?
        || pair.arguments.len() != arguments.len()
    {
        return bad();
    }
    for (&argument, &expected_register) in arguments.iter().zip(&pair.arguments) {
        budget.charge_work(1)?;
        let Some(register) = words.next() else {
            return bad();
        };
        if register != expected_register
            || member(&aliases, register, budget)?
            || slots[slot(register)?] != Some(argument)
            || !matches!(chain.types.get(index(argument)?), Some(ValueType::Int))
        {
            return bad();
        }
    }
    if words.next().is_some() {
        return bad();
    }
    for alias in aliases {
        budget.charge_work(1)?;
        slots[slot(alias)?] = Some(value);
    }
    Ok(())
}
