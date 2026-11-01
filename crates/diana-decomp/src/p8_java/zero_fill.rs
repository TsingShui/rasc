use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::zero_fill::{ZeroFill, ZeroFillPlan};
use crate::raw::{LayoutKind, instruction_layout};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{CodeUnitOffset, EnvironmentId, MethodBudget, RequestError, VerifiedJavaMethod};
pub(crate) struct ZeroFillBody {
    pub fills: Vec<ZeroFill>,
}
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "zero-fill certificate differs from original component, aliases or effects",
    ))
}
fn replay(
    input: &MethodInput<'_, '_>,
    plan: &ZeroFillPlan<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return bad();
    };
    let raw = checked.layout().original();
    if !std::ptr::eq(raw.instructions(), plan.raw.instructions())
        || raw.ins_size() != 1
        || input.access_flags() & 8 == 0
        || !raw.try_byte_range().as_range().is_empty()
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return bad();
    }
    let mut parameters = 0usize;
    let mut valid = true;
    input
        .declaration()
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            parameters = parameters
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            valid &= name.matches_with(plan.kind.descriptor(), budget)?;
            Ok::<(), RequestError>(())
        })?;
    if parameters != 1 || !valid || plan.fills.is_empty() {
        return bad();
    }
    let mut aliases = Vec::new();
    reserve(&mut aliases, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        aliases.push(false);
    }
    let Some(parameter) = aliases.last_mut() else {
        return bad();
    };
    *parameter = true;
    let mut pc = CodeUnitOffset(0);
    let mut cursor = 0usize;
    loop {
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
                    "fill replay lost checked register",
                ))
        };
        let ix =
            |r: crate::RegisterId| usize::try_from(r.0).map_err(|_| RequestError::PlatformCapacity);
        match spec.opcode {
            0 => {}
            0x07..=0x09 => {
                let from = reg(1)?;
                if !aliases[ix(from)?] {
                    return bad();
                }
                aliases[ix(reg(0)?)?] = true;
            }
            0x26 => {
                let Some(fill) = plan.fills.get(cursor) else {
                    return bad();
                };
                let register = reg(0)?;
                let Some(delta) = operands.offset() else {
                    return bad();
                };
                if fill.pc != pc
                    || fill.register != register
                    || !aliases[ix(register)?]
                    || pc.0.checked_add_signed(delta.0) != Some(fill.payload.0)
                {
                    return bad();
                }
                let data = instruction_layout(raw.instructions(), fill.payload, raw.version())
                    .map_err(crate::input::MethodError::Decode)?;
                if !matches!(data.kind(),LayoutKind::ArrayData{element_width,element_count:0} if element_width.0==plan.kind.width())
                {
                    return bad();
                }
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x0e => {
                if pc != plan.returned || cursor != plan.fills.len() {
                    return bad();
                }
                return Ok(());
            }
            _ => return bad(),
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
}
pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    plan: ZeroFillPlan<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    replay(input, &plan, budget)?;
    let RawPrintable::Method(raw) = input.raw() else {
        return bad();
    };
    let flags = input.access_flags();
    if flags & !0x081f != 0
        || (flags & 7).count_ones() > 1
        || raw.annotations().method_range().is_some()
        || raw.annotations().parameter_range().is_some()
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
        (16, "final "),
        (0x800, "strictfp "),
    ] {
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    header.push("void ", budget)?;
    header.push(&name, budget)?;
    header.push("(", budget)?;
    header.push(plan.kind.spelling(), budget)?;
    header.push(" p0)", budget)?;
    Ok(Some(VerifiedJavaMethod::zero_fill(
        header.finish(),
        ZeroFillBody { fills: plan.fills },
        environment,
    )))
}
