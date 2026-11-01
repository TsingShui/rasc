use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::p5_types::PrimitiveArray;
use crate::raw::{LayoutKind, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};
pub(crate) struct ZeroFill {
    pub pc: CodeUnitOffset,
    pub register: RegisterId,
    pub payload: CodeUnitOffset,
}
pub(crate) struct ZeroFillPlan<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub kind: PrimitiveArray,
    pub fills: Vec<ZeroFill>,
    pub returned: CodeUnitOffset,
}
fn bad_type(pc: CodeUnitOffset) -> RequestError {
    crate::input::MethodError::Malformed {
        pc,
        problem: crate::input::MethodProblem::InvalidArrayPayloadType,
    }
    .into()
}
pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ZeroFillPlan<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 8 == 0
        || raw.ins_size() != 1
        || !raw.try_byte_range().as_range().is_empty()
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return Ok(None);
    }
    let mut kind = None;
    let mut known_reference = false;
    let mut count = 0usize;
    input
        .declaration()
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            kind = PrimitiveArray::parse(name, budget)?;
            if kind.is_none() {
                let mut reader = name.reader()?;
                known_reference = reader.next(budget)? == Some(91)
                    && matches!(reader.next(budget)?, Some(76 | 91));
            }
            Ok::<(), RequestError>(())
        })?;
    if count != 1 || kind.is_none() && !known_reference {
        return Ok(None);
    }
    let mut aliases = Vec::new();
    reserve(&mut aliases, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        aliases.push(false);
    }
    let parameter = aliases.last_mut().ok_or(RequestError::InternalInvariant(
        "checked fill parameter lost register",
    ))?;
    *parameter = true;
    let mut fills = Vec::new();
    let mut pc = CodeUnitOffset(0);
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
                    "fill lost checked register",
                ))
        };
        let ix = |r: RegisterId| usize::try_from(r.0).map_err(|_| RequestError::PlatformCapacity);
        match spec.opcode {
            0 => {}
            0x07..=0x09 => {
                let from = reg(1)?;
                if !aliases[ix(from)?] {
                    return Err(RequestError::UndefinedRegister { register: from, pc });
                }
                aliases[ix(reg(0)?)?] = true;
            }
            0x26 => {
                let register = reg(0)?;
                if !aliases[ix(register)?] {
                    return Err(RequestError::UndefinedRegister { register, pc });
                }
                let delta = operands.offset().ok_or(RequestError::InternalInvariant(
                    "fill lost original payload offset",
                ))?;
                let payload = CodeUnitOffset(pc.0.checked_add_signed(delta.0).ok_or(
                    RequestError::InternalInvariant("checked fill payload target overflowed"),
                )?);
                let data = instruction_layout(raw.instructions(), payload, raw.version())
                    .map_err(crate::input::MethodError::Decode)?;
                let LayoutKind::ArrayData {
                    element_width,
                    element_count,
                } = data.kind()
                else {
                    return Err(RequestError::InternalInvariant("fill payload changed kind"));
                };
                let Some(component) = kind else {
                    return Err(bad_type(pc));
                };
                if component.width() != element_width.0 {
                    return Err(bad_type(pc));
                }
                if element_count != 0 {
                    return Ok(None);
                }
                reserve(&mut fills, 1, budget)?;
                fills.push(ZeroFill {
                    pc,
                    register,
                    payload,
                });
            }
            0x0e => {
                let Some(kind) = kind else {
                    return Ok(None);
                };
                if fills.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(ZeroFillPlan {
                    _sealed: (),
                    raw,
                    kind,
                    fills,
                    returned: pc,
                }));
            }
            _ => return Ok(None),
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(None)
}
