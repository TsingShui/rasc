use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::raw::{LayoutKind, OperandReference, instruction_layout};
use crate::substrate::owned::{copy_units, reserve};
use crate::{CodeUnitOffset, DeclarationCatalog, MethodBudget, RegisterId, RequestError};

pub(crate) struct ReceiverPlan<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub parameter: Vec<u16>,
    pub contract: crate::p4_calls::ReceiverContract,
    pub invoke: CodeUnitOffset,
    pub receiver: RegisterId,
    pub result: CodeUnitOffset,
    pub returned: CodeUnitOffset,
}
#[derive(Clone, Copy)]
enum Alias {
    Undefined,
    Parameter,
}
fn ix(register: RegisterId) -> Result<usize, RequestError> {
    usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
}
pub(crate) fn plan<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ReceiverPlan<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 8 == 0
        || !raw.try_byte_range().as_range().is_empty()
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[73], budget)?
    {
        return Ok(None);
    }
    let mut count = 0usize;
    let mut parameter = None;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            if count == 1 {
                parameter = Some(copy_units(name, budget)?);
            }
            Ok(())
        },
    )?;
    let Some(parameter) = parameter else {
        return Ok(None);
    };
    if count != 1
        || parameter.first() != Some(&76)
        || parameter.last() != Some(&59)
        || raw.ins_size() != 1
    {
        return Ok(None);
    }
    // Only the initialized input parameter seeds aliases; allocations and
    // default/null values cannot enter this closed receiver producer.
    let mut aliases = Vec::new();
    reserve(&mut aliases, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        aliases.push(Alias::Undefined);
    }
    let parameter_register =
        usize::from(raw.registers_size())
            .checked_sub(1)
            .ok_or(RequestError::InternalInvariant(
                "receiver parameter has no input word",
            ))?;
    aliases[parameter_register] = Alias::Parameter;
    let mut pc = CodeUnitOffset(0);
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        if !checked.layout().is_reachable(pc) {
            return Ok(None);
        }
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return Ok(None);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |index| {
            operands
                .registers()
                .iter()
                .nth(index)
                .ok_or(RequestError::InternalInvariant(
                    "checked receiver operand disappeared",
                ))
        };
        let next = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
        match spec.opcode {
            0 => {}
            7..=9 => {
                let source = reg(1)?;
                if !matches!(aliases[ix(source)?], Alias::Parameter) {
                    return Err(RequestError::UndefinedRegister {
                        register: source,
                        pc,
                    });
                }
                aliases[ix(reg(0)?)?] = Alias::Parameter;
            }
            0x6e | 0x72 | 0x74 | 0x78 => {
                if operands.registers().iter().count() != 1 {
                    return Ok(None);
                }
                let receiver = reg(0)?;
                if !matches!(aliases[ix(receiver)?], Alias::Parameter) {
                    return Err(RequestError::UndefinedRegister {
                        register: receiver,
                        pc,
                    });
                }
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "receiver call has no reference",
                    ));
                };
                let OperandReference::Method(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "receiver call has wrong reference kind",
                    ));
                };
                let kind = if matches!(spec.opcode, 0x72 | 0x78) {
                    crate::p4_calls::ReceiverKind::Interface
                } else {
                    crate::p4_calls::ReceiverKind::Virtual
                };
                let result = instruction_layout(raw.instructions(), next, raw.version())
                    .map_err(crate::input::MethodError::Decode)?;
                if !matches!(result.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x0a) {
                    return Ok(None);
                }
                let result_operands =
                    crate::input::operands::instruction_operands(raw.instructions(), next, result)
                        .map_err(crate::input::MethodError::Decode)?;
                let result_register = result_operands.registers().iter().next().ok_or(
                    RequestError::InternalInvariant("receiver move-result lost destination"),
                )?;
                let mut end = next
                    .checked_advance(result.width())
                    .ok_or(RequestError::PlatformCapacity)?;
                while end.0 < raw.code_units().0 {
                    budget.charge_work(1)?;
                    let terminal = instruction_layout(raw.instructions(), end, raw.version())
                        .map_err(crate::input::MethodError::Decode)?;
                    match terminal.kind() {
                        LayoutKind::Instruction(spec) if spec.opcode == 0 => {
                            end = end
                                .checked_advance(terminal.width())
                                .ok_or(RequestError::PlatformCapacity)?
                        }
                        LayoutKind::Instruction(spec) if spec.opcode == 0x0f => {
                            let terminal_operands = crate::input::operands::instruction_operands(
                                raw.instructions(),
                                end,
                                terminal,
                            )
                            .map_err(crate::input::MethodError::Decode)?;
                            if terminal_operands.registers().iter().next() != Some(result_register)
                            {
                                return Ok(None);
                            }
                            let Some(contract) = crate::p4_calls::resolve_receiver(
                                input, catalog, reference, kind, &parameter, budget,
                            )?
                            else {
                                return Err(RequestError::BindingEvidenceMissing { reference, pc });
                            };
                            return Ok(Some(ReceiverPlan {
                                _sealed: (),
                                raw,
                                parameter,
                                contract,
                                invoke: pc,
                                receiver,
                                result: next,
                                returned: end,
                            }));
                        }
                        _ => return Ok(None),
                    }
                }
                return Ok(None);
            }
            _ => return Ok(None),
        }
        pc = next;
    }
    Ok(None)
}
