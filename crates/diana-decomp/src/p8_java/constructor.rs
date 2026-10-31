use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::constructor::{Argument, ConstructorPlan};
use crate::pipeline::receiver::PrefixOp;
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{
    CodeUnitOffset, DeclarationCatalog, EnvironmentId, MethodBudget, RegisterId, RequestError,
    VerifiedJavaMethod,
};

pub(crate) struct ConstructorBody {
    pub kind: crate::pipeline::constructor::ChainKind,
    pub chain: crate::p4_calls::allocation::ConstructorContract,
    pub chain_pc: CodeUnitOffset,
    pub receiver: RegisterId,
    pub arguments: Vec<Argument>,
    pub body: Vec<super::receiver::PrefixStatement>,
    pub returned: CodeUnitOffset,
}
#[derive(Clone, Copy)]
enum Root {
    Undefined,
    UninitThis,
    InitializedThis,
    Argument(Argument),
    String(u32),
}
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Before,
    After,
}
fn bad<T>() -> Result<T, RequestError> {
    Err(RequestError::InternalInvariant(
        "constructor differs from original prologue/effect roots",
    ))
}
fn ix(register: RegisterId) -> Result<usize, RequestError> {
    usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)
}
fn replay(
    input: &MethodInput<'_, '_>,
    plan: &ConstructorPlan<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let raw = &plan.raw;
    if plan.parameters.checked_add(1) != Some(u32::from(raw.ins_size()))
        || !input
            .declaration()
            .name
            .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return bad();
    }
    let mut actual_parameters = 0u32;
    let mut all_int = true;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            all_int &= name.matches_with(&[73], budget)?;
            actual_parameters = actual_parameters
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        },
    )?;
    if !all_int || actual_parameters != plan.parameters {
        return bad();
    }
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(Root::Undefined);
    }
    let entry = slots.len().checked_sub(usize::from(raw.ins_size())).ok_or(
        RequestError::InternalInvariant("constructor replay lost this word"),
    )?;
    slots[entry] = Root::UninitThis;
    for parameter in 0..plan.parameters {
        budget.charge_work(1)?;
        let at = entry
            .checked_add(1)
            .and_then(|v| v.checked_add(usize::try_from(parameter).ok()?))
            .ok_or(RequestError::PlatformCapacity)?;
        slots[at] = Root::Argument(Argument::Parameter(parameter));
    }
    let mut pc = CodeUnitOffset(0);
    let mut stage = Stage::Before;
    let mut body = 0usize;
    let mut next_string = 0u32;
    while pc.0 <= plan.returned.0 {
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
                    "constructor replay lost operand",
                ))
        };
        if pc == plan.chain_pc {
            if stage != Stage::Before
                || !matches!(spec.opcode, 0x70 | 0x76)
                || reg(0)? != plan.receiver
                || !matches!(slots[ix(reg(0)?)?], Root::UninitThis)
                || !matches!(layout.references()[0],Some(reference) if matches!(reference.target,OperandReference::Method(id) if id==plan.chain.reference))
            {
                return bad();
            }
            let method = input.method_reference(plan.chain.reference, budget)?;
            if !method.owner.matches_with(&plan.chain.owner, budget)?
                || !method
                    .declaration
                    .name
                    .matches_with(&[60, 105, 110, 105, 116, 62], budget)?
                || !method
                    .declaration
                    .prototype
                    .matches_with(&plan.chain.prototype, budget)?
            {
                return bad();
            }
            let mut words = operands.registers().iter().skip(1);
            if plan.arguments.len() != plan.chain.int_arity {
                return bad();
            }
            for &argument in &plan.arguments {
                budget.charge_work(1)?;
                let Some(register) = words.next() else {
                    return bad();
                };
                if !matches!(slots[ix(register)?],Root::Argument(actual) if actual==argument) {
                    return bad();
                }
            }
            if words.next().is_some() {
                return bad();
            }
            for slot in &mut slots {
                budget.charge_work(1)?;
                if matches!(slot, Root::UninitThis) {
                    *slot = Root::InitializedThis;
                }
            }
            stage = Stage::After;
        } else if stage == Stage::Before {
            match spec.opcode {
                0 => {}
                1..=3 => {
                    let Root::Argument(argument) = slots[ix(reg(1)?)?] else {
                        return bad();
                    };
                    slots[ix(reg(0)?)?] = Root::Argument(argument);
                }
                7..=9 => {
                    if !matches!(slots[ix(reg(1)?)?], Root::UninitThis) {
                        return bad();
                    }
                    slots[ix(reg(0)?)?] = Root::UninitThis;
                }
                0x12..=0x15 => {
                    let value = match operands.literal() {
                        Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                        Some(RawLiteral::Signed8(v)) => i32::from(v),
                        Some(RawLiteral::Signed16(v)) => i32::from(v),
                        _ => return bad(),
                    };
                    slots[ix(reg(0)?)?] = Root::Argument(Argument::Literal(value));
                }
                _ => return bad(),
            }
        } else if pc == plan.returned {
            if spec.opcode != 0x0e || body != plan.body.len() {
                return bad();
            }
            return Ok(());
        } else {
            match spec.opcode {
                0 => {}
                7..=9 => {
                    let value = slots[ix(reg(1)?)?];
                    if !matches!(value, Root::InitializedThis | Root::String(_)) {
                        return bad();
                    }
                    slots[ix(reg(0)?)?] = value;
                }
                0x1a | 0x1b => {
                    let Some(PrefixOp::String {
                        value,
                        reference,
                        pc: origin,
                    }) = plan.body.get(body)
                    else {
                        return bad();
                    };
                    if *origin != pc
                        || value.0 != next_string
                        || !matches!(layout.references()[0],Some(item) if matches!(item.target,OperandReference::String(id) if id==*reference))
                    {
                        return bad();
                    }
                    slots[ix(reg(0)?)?] = Root::String(next_string);
                    next_string = next_string
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                    body = body.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                }
                0x71 | 0x77 => {
                    let Some(PrefixOp::Effect {
                        contract,
                        argument,
                        pc: origin,
                    }) = plan.body.get(body)
                    else {
                        return bad();
                    };
                    if *origin != pc
                        || operands.registers().iter().count() != 1
                        || !matches!(slots[ix(reg(0)?)?],Root::String(value) if value==argument.0)
                        || !matches!(layout.references()[0],Some(item) if matches!(item.target,OperandReference::Method(id) if id==contract.reference))
                    {
                        return bad();
                    }
                    body = body.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                }
                _ => return bad(),
            }
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    bad()
}
pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    plan: ConstructorPlan<'_>,
    catalog: &DeclarationCatalog,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    super::declarations::check_target(input, budget)?;
    let RawPrintable::Method(raw) = input.raw() else {
        return bad();
    };
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        plan.raw.instructions(),
    ) {
        return bad();
    }
    replay(input, &plan, budget)?;
    let caller = copy_units(input.owner(), budget)?;
    let same_owner = crate::p0_knowledge::same_units(&caller, &plan.chain.owner, budget)?;
    if same_owner != (plan.kind == crate::pipeline::constructor::ChainKind::This) {
        return bad();
    }
    if plan.kind == crate::pipeline::constructor::ChainKind::This {
        crate::p4_calls::allocation::prove_this_chain(
            input,
            catalog,
            plan.chain.reference,
            plan.chain_pc,
            budget,
        )?;
    }
    let flags = input.access_flags();
    if flags & 0x10000 == 0
        || flags & !0x10007 != 0
        || (flags & 7).count_ones() > 1
        || input.owner_access_flags() & 0x6200 != 0
        || raw.annotations().method_range().is_some()
        || raw.annotations().parameter_range().is_some()
    {
        return Ok(None);
    }
    if !plan.body.is_empty()
        && !crate::p4_calls::qualifier_available(input, catalog, &[106, 97, 118, 97], budget)?
    {
        return Ok(None);
    }
    if !crate::p4_calls::compatible_throws(
        input,
        catalog,
        Some(&plan.chain.declared_throws),
        budget,
    )? {
        return Ok(None);
    }
    let owner = copy_units(input.owner(), budget)?;
    if owner.len() < 3 || owner.first() != Some(&76) || owner.last() != Some(&59) {
        return bad();
    }
    let mut start = 1usize;
    for (i, &unit) in owner.iter().enumerate() {
        budget.charge_work(1)?;
        if unit == 47 {
            start = i.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        }
    }
    let Some(name) = super::calls::identifier_text(&owner[start..owner.len() - 1], budget)? else {
        return Ok(None);
    };
    let mut header = TextBuffer::new();
    for (bit, text) in [(1, "public "), (2, "private "), (4, "protected ")] {
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    header.push(&name, budget)?;
    header.push("(", budget)?;
    for parameter in 0..plan.parameters {
        budget.charge_work(1)?;
        if parameter != 0 {
            header.push(", ", budget)?;
        }
        header.push("int p", budget)?;
        header.unsigned(u64::from(parameter), budget)?;
    }
    header.push(")", budget)?;
    let Some(body) = super::receiver::owned_prefix(input, plan.body, budget)? else {
        return Ok(None);
    };
    for statement in &body {
        budget.charge_work(1)?;
        let pc = match statement {
            super::receiver::PrefixStatement::String { pc, .. }
            | super::receiver::PrefixStatement::Effect { pc, .. } => *pc,
        };
        if pc.0 <= plan.chain_pc.0 || pc.0 >= plan.returned.0 {
            return bad();
        }
    }
    Ok(Some(VerifiedJavaMethod::constructor(
        header.finish(),
        ConstructorBody {
            kind: plan.kind,
            chain: plan.chain,
            chain_pc: plan.chain_pc,
            receiver: plan.receiver,
            arguments: plan.arguments,
            body,
            returned: plan.returned,
        },
        environment,
    )))
}
