use crate::input::{MethodInput, RawPrintable};
use crate::p5_types::BuiltinReference;
use crate::pipeline::casts::{Cast, CastPlan, CastValueId};
use crate::raw::{LayoutKind, OperandReference, instruction_layout};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{
    CodeUnitOffset, DeclarationCatalog, EnvironmentId, MethodBudget, RequestError,
    VerifiedJavaMethod,
};
pub(crate) struct CastBody {
    pub parameter_count: usize,
    pub casts: Vec<Cast>,
}
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "cast definitions or effects differ from original instructions",
    ))
}
fn replay(
    input: &MethodInput<'_, '_>,
    plan: &CastPlan<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return bad();
    };
    let raw = checked.layout().original();
    if !std::ptr::eq(raw.instructions(), plan.raw.instructions())
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
    let mut parameter = 0usize;
    let mut valid = true;
    input
        .declaration()
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            let Some(kind) = plan.parameters.get(parameter) else {
                valid = false;
                return Ok::<(), RequestError>(());
            };
            if !name.matches_with(kind.descriptor(), budget)? {
                valid = false;
            }
            parameter = parameter
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        })?;
    if !valid
        || parameter != plan.parameters.len()
        || parameter != usize::from(raw.ins_size())
        || plan.casts.is_empty()
    {
        return bad();
    }
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(None);
    }
    let start = usize::from(raw.registers_size())
        .checked_sub(parameter)
        .ok_or(RequestError::InternalInvariant(
            "cast replay parameter range differs",
        ))?;
    for (i, slot) in slots[start..].iter_mut().enumerate() {
        budget.charge_work(1)?;
        *slot = Some(CastValueId(
            u32::try_from(i).map_err(|_| RequestError::PlatformCapacity)?,
        ));
    }
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
                .ok_or(RequestError::InternalInvariant("cast replay lost register"))
        };
        match spec.opcode {
            0 => {}
            0x07..=0x09 => {
                let from = reg(1)?;
                let Some(value) =
                    slots[usize::try_from(from.0).map_err(|_| RequestError::PlatformCapacity)?]
                else {
                    return bad();
                };
                slots[usize::try_from(reg(0)?.0).map_err(|_| RequestError::PlatformCapacity)?] =
                    Some(value);
            }
            0x1f => {
                let Some(cast) = plan.casts.get(cursor) else {
                    return bad();
                };
                let register = reg(0)?;
                if pc != cast.pc
                    || register != cast.register
                    || slots
                        [usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)?]
                        != Some(cast.source)
                    || !matches!(layout.references()[0],Some(reference) if reference.target==OperandReference::Type(cast.reference))
                    || !input
                        .type_name(cast.reference, budget)?
                        .matches_with(cast.kind.descriptor(), budget)?
                {
                    return bad();
                }
                let next = u32::try_from(
                    parameter
                        .checked_add(cursor)
                        .ok_or(RequestError::PlatformCapacity)?,
                )
                .map_err(|_| RequestError::PlatformCapacity)?;
                if cast.result.0 != next {
                    return bad();
                }
                slots[usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)?] =
                    Some(cast.result);
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x0e => {
                if pc != plan.returned || cursor != plan.casts.len() {
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
    plan: CastPlan<'_>,
    catalog: &DeclarationCatalog,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    replay(input, &plan, budget)?;
    let RawPrintable::Method(raw) = input.raw() else {
        return bad();
    };
    let flags = input.access_flags();
    if flags & !0x081f != 0
        || raw.annotations().method_range().is_some()
        || raw.annotations().parameter_range().is_some()
        || !crate::p4_calls::qualifier_available(input, catalog, &[106, 97, 118, 97], budget)?
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
    for (i, kind) in plan.parameters.iter().enumerate() {
        budget.charge_work(1)?;
        if i != 0 {
            header.push(", ", budget)?;
        }
        header.push(
            match kind {
                BuiltinReference::Object => "java.lang.Object",
                BuiltinReference::String => "java.lang.String",
            },
            budget,
        )?;
        header.push(" p", budget)?;
        header.unsigned(
            u64::try_from(i).map_err(|_| RequestError::PlatformCapacity)?,
            budget,
        )?;
    }
    header.push(")", budget)?;
    Ok(Some(VerifiedJavaMethod::casts(
        header.finish(),
        CastBody {
            parameter_count: plan.parameters.len(),
            casts: plan.casts,
        },
        environment,
    )))
}
