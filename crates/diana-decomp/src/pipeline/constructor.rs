use super::receiver::{PrefixOp, PrefixValueId};
use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::{copy_units, reserve};
use crate::{CodeUnitOffset, DeclarationCatalog, MethodBudget, RegisterId, RequestError};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Argument {
    Parameter(u32),
    Literal(i32),
}
#[derive(Clone, Copy)]
enum Slot {
    Undefined,
    UninitThis,
    InitializedThis,
    Int(Argument),
    String(PrefixValueId),
}
pub(crate) struct ConstructorPlan<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub parameters: u32,
    pub chain: crate::p4_calls::allocation::ConstructorContract,
    pub chain_pc: CodeUnitOffset,
    pub receiver: RegisterId,
    pub arguments: Vec<Argument>,
    pub body: Vec<PrefixOp>,
    pub returned: CodeUnitOffset,
}
fn ix(register: RegisterId) -> Result<usize, RequestError> {
    usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
}
fn unavailable<T>(pc: CodeUnitOffset) -> Result<T, RequestError> {
    Err(RequestError::InitializationUnavailable { pc })
}
pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ConstructorPlan<'a>>, RequestError> {
    if input.access_flags() & 0x10008 != 0x10000
        || !input
            .declaration()
            .name
            .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
    {
        return Ok(None);
    }
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if !raw.try_byte_range().as_range().is_empty() {
        return unavailable(CodeUnitOffset(0));
    }
    let mut parameters = 0u32;
    let mut supported = true;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            if !name.matches_with(&[73], budget)? {
                supported = false;
            }
            parameters = parameters
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        },
    )?;
    if !supported
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return unavailable(CodeUnitOffset(0));
    }
    if parameters.checked_add(1) != Some(u32::from(raw.ins_size())) {
        return Err(RequestError::InternalInvariant(
            "constructor input words changed",
        ));
    }
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(Slot::Undefined);
    }
    let entry = usize::from(raw.registers_size())
        .checked_sub(usize::from(raw.ins_size()))
        .ok_or(RequestError::InternalInvariant(
            "constructor has no this word",
        ))?;
    slots[entry] = Slot::UninitThis;
    for parameter in 0..parameters {
        budget.charge_work(1)?;
        let at = entry
            .checked_add(1)
            .and_then(|v| v.checked_add(usize::try_from(parameter).ok()?))
            .ok_or(RequestError::PlatformCapacity)?;
        slots[at] = Slot::Int(Argument::Parameter(parameter));
    }
    let mut pc = CodeUnitOffset(0);
    let (chain, chain_pc, receiver, arguments) = loop {
        budget.charge_work(1)?;
        if pc.0 >= raw.code_units().0 || !checked.layout().is_reachable(pc) {
            return unavailable(pc);
        }
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return unavailable(pc);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "constructor prologue lost checked operand",
                ))
        };
        let next = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
        match spec.opcode {
            0 => {}
            1..=3 => {
                let Slot::Int(argument) = slots[ix(reg(1)?)?] else {
                    return unavailable(pc);
                };
                slots[ix(reg(0)?)?] = Slot::Int(argument);
            }
            7..=9 => {
                if !matches!(slots[ix(reg(1)?)?], Slot::UninitThis) {
                    return unavailable(pc);
                }
                slots[ix(reg(0)?)?] = Slot::UninitThis;
            }
            0x12..=0x15 => {
                let literal = match operands.literal() {
                    Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => {
                        return Err(RequestError::InternalInvariant(
                            "constructor literal lost bits",
                        ));
                    }
                };
                slots[ix(reg(0)?)?] = Slot::Int(Argument::Literal(literal));
            }
            0x70 | 0x76 => {
                let mut words = operands.registers().iter();
                let receiver = words.next().ok_or(RequestError::InternalInvariant(
                    "constructor chain lost this",
                ))?;
                if !matches!(slots[ix(receiver)?], Slot::UninitThis) {
                    return unavailable(pc);
                }
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "constructor chain lost reference",
                    ));
                };
                let OperandReference::Method(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "constructor chain changed reference kind",
                    ));
                };
                let owner = copy_units(input.method_reference(reference, budget)?.owner, budget)?;
                let chain = crate::p4_calls::allocation::resolve(
                    input,
                    catalog,
                    reference,
                    &owner,
                    crate::p4_calls::allocation::ConstructorUse::Super,
                    pc,
                    budget,
                )?;
                let mut arguments = Vec::new();
                for register in words {
                    budget.charge_work(1)?;
                    let Slot::Int(argument) = slots[ix(register)?] else {
                        return unavailable(pc);
                    };
                    reserve(&mut arguments, 1, budget)?;
                    arguments.push(argument);
                }
                if arguments.len() != chain.int_arity {
                    return Err(RequestError::InternalInvariant(
                        "constructor chain changed arity",
                    ));
                }
                let origin = pc;
                pc = next;
                break (chain, origin, receiver, arguments);
            }
            _ => return unavailable(pc),
        }
        pc = next;
    };
    for slot in &mut slots {
        budget.charge_work(1)?;
        if matches!(slot, Slot::UninitThis) {
            *slot = Slot::InitializedThis;
        }
    }
    let mut body = Vec::new();
    let mut next_value = 0u32;
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        if !checked.layout().is_reachable(pc) {
            return unavailable(pc);
        }
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return unavailable(pc);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "constructor body lost checked operand",
                ))
        };
        match spec.opcode {
            0 => {}
            7..=9 => {
                let value = slots[ix(reg(1)?)?];
                if !matches!(value, Slot::InitializedThis | Slot::String(_)) {
                    return unavailable(pc);
                }
                slots[ix(reg(0)?)?] = value;
            }
            0x1a | 0x1b => {
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "constructor string lost reference",
                    ));
                };
                let OperandReference::String(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "constructor string changed reference kind",
                    ));
                };
                let value = PrefixValueId(next_value);
                next_value = next_value
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                slots[ix(reg(0)?)?] = Slot::String(value);
                reserve(&mut body, 1, budget)?;
                body.push(PrefixOp::String {
                    value,
                    reference,
                    pc,
                });
            }
            0x71 | 0x77 => {
                if operands.registers().iter().count() != 1 {
                    return unavailable(pc);
                }
                let Slot::String(argument) = slots[ix(reg(0)?)?] else {
                    return unavailable(pc);
                };
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "constructor effect lost reference",
                    ));
                };
                let OperandReference::Method(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "constructor effect changed reference kind",
                    ));
                };
                let Some(contract) =
                    crate::p4_calls::resolve_static(input, catalog, reference, budget)?
                else {
                    return Err(RequestError::BindingEvidenceMissing { reference, pc });
                };
                if contract.result != crate::p4_calls::StaticResult::Void
                    || !matches!(
                        contract.formals.as_slice(),
                        [crate::p4_calls::CallFormal::Reference(
                            crate::p5_types::BuiltinReference::String
                        )]
                    )
                {
                    return unavailable(pc);
                }
                reserve(&mut body, 1, budget)?;
                body.push(PrefixOp::Effect {
                    contract,
                    argument,
                    pc,
                });
            }
            0x0e => {
                return Ok(Some(ConstructorPlan {
                    _sealed: (),
                    raw,
                    parameters,
                    chain,
                    chain_pc,
                    receiver,
                    arguments,
                    body,
                    returned: pc,
                }));
            }
            _ => return unavailable(pc),
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    unavailable(pc)
}
