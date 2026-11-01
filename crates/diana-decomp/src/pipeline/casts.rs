use crate::input::{DexString, MethodInput, RawMethodView, RawPrintable};
use crate::p5_types::BuiltinReference;
use crate::raw::{LayoutKind, OperandReference, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError, TypeId};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CastValueId(pub u32);
pub(crate) struct Cast {
    pub pc: CodeUnitOffset,
    pub register: RegisterId,
    pub reference: TypeId,
    pub kind: BuiltinReference,
    pub source: CastValueId,
    pub result: CastValueId,
}
pub(crate) struct CastPlan<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<BuiltinReference>,
    pub casts: Vec<Cast>,
    pub returned: CodeUnitOffset,
}
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
fn builtin(
    name: DexString<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<BuiltinReference>, RequestError> {
    for kind in [BuiltinReference::Object, BuiltinReference::String] {
        if name.matches_with(kind.descriptor(), budget)? {
            return Ok(Some(kind));
        }
    }
    Ok(None)
}
pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<CastPlan<'a>>, RequestError> {
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
            .matches_with(&[86], budget)?
    {
        return Ok(None);
    }
    let mut parameters = Vec::new();
    let mut supported = true;
    input
        .declaration()
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            match builtin(name, budget)? {
                Some(kind) => push(&mut parameters, kind, budget)?,
                None => supported = false,
            }
            Ok::<(), RequestError>(())
        })?;
    if !supported || parameters.len() != usize::from(raw.ins_size()) {
        return Ok(None);
    }
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(None);
    }
    let start = usize::from(raw.registers_size())
        .checked_sub(parameters.len())
        .ok_or(RequestError::InternalInvariant(
            "checked cast parameters exceed registers",
        ))?;
    for (i, slot) in slots[start..].iter_mut().enumerate() {
        budget.charge_work(1)?;
        *slot = Some(CastValueId(
            u32::try_from(i).map_err(|_| RequestError::PlatformCapacity)?,
        ));
    }
    let mut casts = Vec::new();
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
                    "cast operation lost checked register",
                ))
        };
        match spec.opcode {
            0 => {}
            0x07..=0x09 => {
                let from = reg(1)?;
                let value = slots
                    [usize::try_from(from.0).map_err(|_| RequestError::PlatformCapacity)?]
                .ok_or(RequestError::UndefinedRegister { register: from, pc })?;
                slots[usize::try_from(reg(0)?.0).map_err(|_| RequestError::PlatformCapacity)?] =
                    Some(value);
            }
            0x1f => {
                let register = reg(0)?;
                let source = slots
                    [usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)?]
                .ok_or(RequestError::UndefinedRegister { register, pc })?;
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "checked cast lost type reference",
                    ));
                };
                let OperandReference::Type(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "cast reference changed domain",
                    ));
                };
                let Some(kind) = builtin(input.type_name(reference, budget)?, budget)? else {
                    return Ok(None);
                };
                let result = CastValueId(
                    u32::try_from(
                        parameters
                            .len()
                            .checked_add(casts.len())
                            .ok_or(RequestError::PlatformCapacity)?,
                    )
                    .map_err(|_| RequestError::PlatformCapacity)?,
                );
                push(
                    &mut casts,
                    Cast {
                        pc,
                        register,
                        reference,
                        kind,
                        source,
                        result,
                    },
                    budget,
                )?;
                slots[usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)?] =
                    Some(result);
            }
            0x0e => {
                if casts.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(CastPlan {
                    _sealed: (),
                    raw,
                    parameters,
                    casts,
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
