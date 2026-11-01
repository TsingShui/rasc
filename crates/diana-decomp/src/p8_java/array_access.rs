use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::array_access::{
    AccessPlan, ElementUse, Kind, Operation, Return, ValueId, index,
};
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{
    CodeUnitOffset, DeclarationCatalog, EnvironmentId, MethodBudget, RequestError,
    VerifiedJavaMethod,
};
pub(crate) enum Statement {
    Constant {
        value: ValueId,
        literal: i32,
    },
    Call {
        result: Option<ValueId>,
        call: super::calls::StaticCall,
    },
    Load {
        array: ValueId,
        index: ValueId,
        result: ValueId,
    },
    Store {
        array: ValueId,
        index: ValueId,
        value: ElementUse,
    },
    Length {
        array: ValueId,
        result: ValueId,
    },
}
pub(crate) struct Body {
    pub parameter_count: usize,
    pub types: Vec<Kind>,
    pub statements: Vec<Statement>,
    pub returned: Return,
}
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "array access definitions/order/types differ from original code",
    ))
}
fn definition(
    plan: &AccessPlan<'_>,
    id: ValueId,
    kind: Kind,
    zero: bool,
    next: &mut u32,
) -> Result<(), RequestError> {
    if id.0 != *next
        || plan.types.get(index(id)?) != Some(&kind)
        || plan.zero.get(index(id)?) != Some(&zero)
    {
        return bad();
    }
    *next = next.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    Ok(())
}
fn replay(
    input: &MethodInput<'_, '_>,
    plan: &AccessPlan<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return bad();
    };
    let raw = checked.layout().original();
    if !std::ptr::eq(raw.instructions(), plan.raw.instructions())
        || input.access_flags() & 8 == 0
        || !raw.try_byte_range().as_range().is_empty()
        || plan.types.len() != plan.zero.len()
    {
        return bad();
    }
    let mut count = 0usize;
    let mut valid = true;
    input
        .declaration()
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            let Some(kind) = plan.parameters.get(count) else {
                valid = false;
                return Ok::<(), RequestError>(());
            };
            valid &= name.matches_with(kind.descriptor(), budget)?;
            count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        })?;
    if !valid || count != plan.parameters.len() || count != usize::from(raw.ins_size()) {
        return bad();
    }
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(None);
    }
    let start = slots
        .len()
        .checked_sub(count)
        .ok_or(RequestError::InternalInvariant(
            "array replay parameters exceed frame",
        ))?;
    let mut next_value = 0u32;
    for (i, &kind) in plan.parameters.iter().enumerate() {
        budget.charge_work(1)?;
        let id = ValueId(next_value);
        definition(plan, id, kind, false, &mut next_value)?;
        slots[start + i] = Some(id);
    }
    let mut pc = CodeUnitOffset(0);
    let mut cursor = 0usize;
    let mut accessed = false;
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
                    "array replay lost checked register",
                ))
        };
        let slot =
            |r: crate::RegisterId| usize::try_from(r.0).map_err(|_| RequestError::PlatformCapacity);
        let read = |r: crate::RegisterId| {
            slots[slot(r)?].ok_or(RequestError::InternalInvariant(
                "array replay has undefined original use",
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
                }) = plan.operations.get(cursor)
                else {
                    return bad();
                };
                let original = match operands.literal() {
                    Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => return bad(),
                };
                if *origin != pc || *literal != original {
                    return bad();
                }
                definition(plan, *value, Kind::Int, original == 0, &mut next_value)?;
                slots[slot(reg(0)?)?] = Some(*value);
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x01..=0x03 | 0x07..=0x09 => {
                let source = read(reg(1)?)?;
                let kind = plan.types[index(source)?];
                if matches!(spec.opcode, 0x07..=0x09)
                    && !kind.reference()
                    && !plan.zero[index(source)?]
                    || matches!(spec.opcode, 0x01..=0x03) && kind != Kind::Int
                {
                    return bad();
                }
                slots[slot(reg(0)?)?] = Some(source);
            }
            0x71 | 0x77 => {
                let Some(Operation::Call {
                    pc: origin,
                    contract,
                    result,
                }) = plan.operations.get(cursor)
                else {
                    return bad();
                };
                if *origin != pc
                    || !operands.registers().is_empty()
                    || !contract.formals.is_empty()
                    || !contract.declared_throws.is_empty()
                    || !matches!(layout.references()[0],Some(reference) if reference.target==OperandReference::Method(contract.reference))
                {
                    return bad();
                }
                let original = input.method_reference(contract.reference, budget)?;
                if !original.owner.matches_with(&contract.owner, budget)?
                    || !original
                        .declaration
                        .name
                        .matches_with(&contract.name, budget)?
                    || crate::substrate::owned::copy_proto(&original.declaration.prototype, budget)?
                        != contract.prototype
                {
                    return bad();
                }
                let expected = match contract.result {
                    crate::p4_calls::StaticResult::Int => &[73][..],
                    crate::p4_calls::StaticResult::Void => &[86][..],
                    _ => return bad(),
                };
                if !original
                    .declaration
                    .prototype
                    .return_type
                    .matches_with(expected, budget)?
                {
                    return bad();
                }
                if let Some((value, result_pc)) = result {
                    if *result_pc != next || contract.result != crate::p4_calls::StaticResult::Int {
                        return bad();
                    }
                    let following = instruction_layout(raw.instructions(), next, raw.version())
                        .map_err(crate::input::MethodError::Decode)?;
                    if !matches!(following.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x0a)
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
                    definition(plan, *value, Kind::Int, false, &mut next_value)?;
                    slots[slot(destination)?] = Some(*value);
                    next = next
                        .checked_advance(following.width())
                        .ok_or(RequestError::PlatformCapacity)?;
                }
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x44..=0x51 => {
                let array = read(reg(1)?)?;
                let at = read(reg(2)?)?;
                let element = match plan.types[index(array)?] {
                    Kind::IntArray => Kind::Int,
                    Kind::ObjectArray => Kind::Object,
                    _ => return bad(),
                };
                if plan.types[index(at)?] != Kind::Int {
                    return bad();
                }
                let load = spec.opcode <= 0x4a;
                let expected = match (element, load) {
                    (Kind::Int, true) => 0x44,
                    (Kind::Int, false) => 0x4b,
                    (Kind::Object, true) => 0x46,
                    (Kind::Object, false) => 0x4d,
                    _ => return bad(),
                };
                if spec.opcode != expected {
                    return bad();
                }
                if load {
                    let Some(Operation::Load {
                        pc: origin,
                        array: a,
                        index: i,
                        result,
                    }) = plan.operations.get(cursor)
                    else {
                        return bad();
                    };
                    if *origin != pc || *a != array || *i != at {
                        return bad();
                    }
                    definition(plan, *result, element, false, &mut next_value)?;
                    slots[slot(reg(0)?)?] = Some(*result);
                } else {
                    let Some(Operation::Store {
                        pc: origin,
                        array: a,
                        index: i,
                        value,
                    }) = plan.operations.get(cursor)
                    else {
                        return bad();
                    };
                    if *origin != pc || *a != array || *i != at || value.id() != read(reg(0)?)? {
                        return bad();
                    }
                    let v = index(value.id())?;
                    match value {
                        ElementUse::Null(_) => {
                            if element != Kind::Object || !plan.zero[v] {
                                return bad();
                            }
                        }
                        ElementUse::Value(_) => {
                            if element == Kind::Int && plan.types[v] != Kind::Int
                                || element == Kind::Object && !plan.types[v].reference()
                            {
                                return bad();
                            }
                        }
                    }
                }
                accessed = true;
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x21 => {
                let array = read(reg(1)?)?;
                let Some(Operation::Length {
                    pc: origin,
                    array: a,
                    result,
                }) = plan.operations.get(cursor)
                else {
                    return bad();
                };
                if *origin != pc
                    || *a != array
                    || !matches!(
                        plan.types[index(array)?],
                        Kind::IntArray | Kind::ObjectArray
                    )
                {
                    return bad();
                }
                definition(plan, *result, Kind::Int, false, &mut next_value)?;
                slots[slot(reg(0)?)?] = Some(*result);
                accessed = true;
                cursor = cursor
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            0x0e | 0x0f => {
                if !accessed
                    || plan.return_pc != pc
                    || cursor != plan.operations.len()
                    || usize::try_from(next_value).map_err(|_| RequestError::PlatformCapacity)?
                        != plan.types.len()
                {
                    return bad();
                }
                match plan.returned {
                    Return::Void => {
                        if spec.opcode != 0x0e
                            || !input
                                .declaration()
                                .prototype
                                .return_type
                                .matches_with(&[86], budget)?
                        {
                            return bad();
                        }
                    }
                    Return::Int(value) => {
                        if spec.opcode != 0x0f
                            || value != read(reg(0)?)?
                            || plan.types[index(value)?] != Kind::Int
                            || !input
                                .declaration()
                                .prototype
                                .return_type
                                .matches_with(&[73], budget)?
                        {
                            return bad();
                        }
                    }
                }
                return Ok(());
            }
            _ => return bad(),
        }
        pc = next;
    }
}
pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    plan: AccessPlan<'_>,
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
        || (flags & 7).count_ones() > 1
        || raw.annotations().method_range().is_some()
        || raw.annotations().parameter_range().is_some()
    {
        return Ok(None);
    }
    if plan
        .parameters
        .iter()
        .any(|kind| matches!(kind, Kind::Object | Kind::String | Kind::ObjectArray))
        && !crate::p4_calls::qualifier_available(input, catalog, &[106, 97, 118, 97], budget)?
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
    header.push(
        match plan.returned {
            Return::Void => "void ",
            Return::Int(_) => "int ",
        },
        budget,
    )?;
    header.push(&name, budget)?;
    header.push("(", budget)?;
    for (i, kind) in plan.parameters.iter().enumerate() {
        budget.charge_work(1)?;
        if i != 0 {
            header.push(", ", budget)?;
        }
        header.push(kind.spelling(), budget)?;
        header.push(" p", budget)?;
        header.unsigned(
            u64::try_from(i).map_err(|_| RequestError::PlatformCapacity)?,
            budget,
        )?;
    }
    header.push(")", budget)?;
    let mut statements = Vec::new();
    for op in plan.operations {
        budget.charge_work(1)?;
        let statement = match op {
            Operation::Constant { value, literal, .. } => Statement::Constant { value, literal },
            Operation::Load {
                array,
                index,
                result,
                ..
            } => Statement::Load {
                array,
                index,
                result,
            },
            Operation::Store {
                array,
                index,
                value,
                ..
            } => Statement::Store {
                array,
                index,
                value,
            },
            Operation::Length { array, result, .. } => Statement::Length { array, result },
            Operation::Call {
                pc,
                contract,
                result,
            } => {
                let Some(call) =
                    super::calls::empty_static(contract, pc, result.map(|(_, pc)| pc), budget)?
                else {
                    return Ok(None);
                };
                Statement::Call {
                    result: result.map(|(value, _)| value),
                    call,
                }
            }
        };
        reserve(&mut statements, 1, budget)?;
        statements.push(statement);
    }
    Ok(Some(VerifiedJavaMethod::array_access(
        header.finish(),
        Body {
            parameter_count: plan.parameters.len(),
            types: plan.types,
            statements,
            returned: plan.returned,
        },
        environment,
    )))
}
