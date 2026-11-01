use crate::input::{MethodInput, RawPrintable};
use crate::p5_types::PrimitiveArray;
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};
#[derive(Clone, Copy)]
enum ArrayComponent {
    Primitive(PrimitiveArray),
    Reference,
}
struct AdjacentArray {
    register: RegisterId,
    length: Option<i32>,
    kind: ArrayComponent,
}
pub(crate) fn check(
    input: &MethodInput<'_, '_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(());
    };
    let raw = checked.layout().original();
    let mut pc = CodeUnitOffset(0);
    let mut constant = None;
    let mut array: Option<AdjacentArray> = None;
    if input.access_flags() & 8 != 0 && raw.ins_size() == 1 {
        let mut count = 0usize;
        let mut component = None;
        input
            .declaration()
            .prototype
            .visit_parameter_types(budget, |name, budget| {
                count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                if let Some(kind) = PrimitiveArray::parse(name, budget)? {
                    component = Some(ArrayComponent::Primitive(kind));
                } else {
                    let mut reader = name.reader()?;
                    if reader.next(budget)? == Some(91)
                        && matches!(reader.next(budget)?, Some(76 | 91))
                    {
                        component = Some(ArrayComponent::Reference);
                    }
                }
                Ok::<(), RequestError>(())
            })?;
        if count == 1
            && let Some(kind) = component
        {
            array = Some(AdjacentArray {
                register: RegisterId(
                    u32::from(raw.registers_size())
                        .checked_sub(1)
                        .ok_or(RequestError::InternalInvariant("array parameter lost slot"))?,
                ),
                length: None,
                kind,
            });
        }
    }
    let mut branched = !raw.try_byte_range().as_range().is_empty();
    let mut scan = CodeUnitOffset(0);
    while scan.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), scan, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        if matches!(layout.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,0x28..=0x2c|0x32..=0x3d))
        {
            branched = true;
        }
        scan = scan
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    let mut initialized = Vec::new();
    crate::substrate::owned::reserve(&mut initialized, usize::from(raw.registers_size()), budget)?;
    let parameter_start =
        raw.registers_size()
            .checked_sub(raw.ins_size())
            .ok_or(RequestError::InternalInvariant(
                "fill parameters exceed checked frame",
            ))?;
    for register in 0..raw.registers_size() {
        budget.charge_work(1)?;
        initialized.push(register >= parameter_start);
    }
    let mut definitions_known = !branched;
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        if let LayoutKind::Instruction(spec) = layout.kind() {
            if checked.layout().is_reachable(pc) {
                let operands =
                    crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
                        .map_err(crate::input::MethodError::Decode)?;
                let reg = |i| {
                    operands
                        .registers()
                        .iter()
                        .nth(i)
                        .ok_or(RequestError::InternalInvariant(
                            "fill shape lost checked register",
                        ))
                };
                if definitions_known {
                    let read = match spec.opcode {
                        0x23 | 0x07..=0x09 => Some(reg(1)?),
                        0x26 => Some(reg(0)?),
                        _ => None,
                    };
                    if let Some(register) = read
                        && !initialized[usize::try_from(register.0)
                            .map_err(|_| RequestError::PlatformCapacity)?]
                    {
                        return Err(RequestError::UndefinedRegister { register, pc });
                    }
                    match spec.opcode {
                        0 => {}
                        0x12..=0x15 | 0x23 | 0x07..=0x09 => {
                            initialized[usize::try_from(reg(0)?.0)
                                .map_err(|_| RequestError::PlatformCapacity)?] = true;
                        }
                        0x26 => {}
                        _ => definitions_known = false,
                    }
                }
                match spec.opcode {
                    0 => {}
                    0x12..=0x15 => {
                        let register = reg(0)?;
                        if array
                            .as_ref()
                            .is_some_and(|array| array.register == register)
                        {
                            array = None;
                        }
                        let value = match operands.literal() {
                            Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                            Some(RawLiteral::Signed8(v)) => i32::from(v),
                            Some(RawLiteral::Signed16(v)) => i32::from(v),
                            _ => {
                                return Err(RequestError::InternalInvariant(
                                    "numeric const lost original bits",
                                ));
                            }
                        };
                        constant = Some((register, value));
                    }
                    0x23 => {
                        let Some(reference) = layout.references()[0] else {
                            return Err(RequestError::InternalInvariant("new-array lost type"));
                        };
                        let OperandReference::Type(reference) = reference.target else {
                            return Err(RequestError::InternalInvariant(
                                "new-array changed reference domain",
                            ));
                        };
                        let length_register = reg(1)?;
                        array = Some(AdjacentArray {
                            register: reg(0)?,
                            length: constant
                                .filter(|(register, _)| *register == length_register)
                                .map(|(_, value)| value),
                            kind: PrimitiveArray::parse(
                                input.type_name(reference, budget)?,
                                budget,
                            )?
                            .map_or(ArrayComponent::Reference, ArrayComponent::Primitive),
                        });
                        constant = None;
                    }
                    0x07..=0x09 => {
                        let target = reg(0)?;
                        let source = reg(1)?;
                        if let Some(fresh) = &mut array {
                            if fresh.register == source {
                                fresh.register = target;
                            } else if fresh.register == target {
                                array = None;
                            }
                        }
                        constant = None;
                    }
                    0x26 => {
                        let Some(delta) = operands.offset() else {
                            return Err(RequestError::InternalInvariant(
                                "fill shape lost payload delta",
                            ));
                        };
                        let payload = CodeUnitOffset(pc.0.checked_add_signed(delta.0).ok_or(
                            RequestError::InternalInvariant("checked fill target overflowed"),
                        )?);
                        let data = instruction_layout(raw.instructions(), payload, raw.version())
                            .map_err(crate::input::MethodError::Decode)?;
                        let LayoutKind::ArrayData {
                            element_width,
                            element_count,
                        } = data.kind()
                        else {
                            return Err(RequestError::InternalInvariant("fill shape lost payload"));
                        };
                        let register = reg(0)?;
                        if !branched
                            && let Some(known) =
                                array.as_ref().filter(|array| array.register == register)
                        {
                            let ArrayComponent::Primitive(kind) = known.kind else {
                                return Err(crate::input::MethodError::Malformed {
                                    pc,
                                    problem: crate::input::MethodProblem::InvalidArrayPayloadType,
                                }
                                .into());
                            };
                            if kind.width() != element_width.0 {
                                return Err(crate::input::MethodError::Malformed {
                                    pc,
                                    problem: crate::input::MethodProblem::InvalidArrayPayloadType,
                                }
                                .into());
                            }
                        }
                        if element_count != 0 {
                            if branched {
                                return Err(RequestError::ArrayFillShape { pc });
                            }
                            let register = reg(0)?;
                            let Some(fresh) =
                                array.as_ref().filter(|array| array.register == register)
                            else {
                                return Err(RequestError::ArrayFillShape { pc });
                            };
                            if !fresh.length.is_some_and(|length| {
                                length >= 0
                                    && u32::try_from(length)
                                        .is_ok_and(|length| length >= element_count)
                            }) {
                                return Err(RequestError::ArrayFillShape { pc });
                            }
                        }
                        array = None;
                        constant = None;
                    }
                    _ => {
                        array = None;
                        constant = None;
                    }
                }
            } else {
                array = None;
                constant = None;
            }
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(())
}
