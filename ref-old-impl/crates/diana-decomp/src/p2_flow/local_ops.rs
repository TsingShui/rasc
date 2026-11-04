use super::normal::{CallId, CallSite, CallTarget, InvokeKind, InvokeResult, Op, ValueWidth};
use crate::input::ProgramIndex;
use crate::raw::{InsnLayout, OpcodeSpec, OperandReference, RawLiteral, RawOperands};
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RequestError, RunContext};

pub(super) fn local(spec: OpcodeSpec, operands: RawOperands) -> Result<Option<Op>, RequestError> {
    let reg = |at| {
        operands
            .registers()
            .iter()
            .nth(at)
            .ok_or(RequestError::InternalInvariant(
                "local operation lost operand",
            ))
    };
    Ok(Some(match spec.opcode {
        0 => Op::Nop,
        1..=3 | 7..=9 => Op::Move(reg(0)?, reg(1)?),
        4..=6 => Op::MoveWide(reg(0)?, reg(1)?),
        0x12..=0x15 => Op::Constant(
            reg(0)?,
            match operands.literal() {
                Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                Some(RawLiteral::Signed8(v)) => i32::from(v),
                Some(RawLiteral::Signed16(v)) => i32::from(v),
                _ => return Err(RequestError::InternalInvariant("scalar constant absent")),
            },
        ),
        0x16..=0x19 => Op::ConstantWide(
            reg(0)?,
            match operands.literal() {
                Some(RawLiteral::Bits64(v)) => i64::from_le_bytes(v.to_le_bytes()),
                Some(RawLiteral::Signed16(v)) => i64::from(v),
                Some(RawLiteral::Bits32(v)) => i64::from(i32::from_le_bytes(v.to_le_bytes())),
                _ => {
                    return Err(RequestError::InternalInvariant(
                        "wide literal lost raw bits",
                    ));
                }
            },
        ),
        _ => return Ok(None),
    }))
}
pub(super) fn call(
    index: &ProgramIndex<'_>,
    pc: CodeUnitOffset,
    kind: InvokeKind,
    reference: crate::MethodId,
    result: InvokeResult,
    operands: RawOperands,
    budget: &mut RunContext<'_>,
) -> Result<CallSite, RequestError> {
    let method = index.method_reference(reference, budget)?;
    let mut widths = Vec::new();
    if kind != InvokeKind::Static {
        push(&mut widths, ValueWidth::One, budget)?;
    }
    method
        .declaration
        .prototype
        .visit_parameter_types(budget, |descriptor, budget| {
            let mut units = descriptor.reader()?;
            push(
                &mut widths,
                if matches!(units.next(budget)?, Some(74 | 68)) {
                    ValueWidth::Two
                } else {
                    ValueWidth::One
                },
                budget,
            )
        })?;
    let mut registers = Vec::new();
    for register in operands.registers().iter() {
        budget.checkpoint()?;
        push(&mut registers, register, budget)?;
    }
    let mut arguments = Vec::new();
    let mut word = 0usize;
    for width in &widths {
        budget.checkpoint()?;
        let low = *registers.get(word).ok_or(RequestError::InternalInvariant(
            "invoke lost argument words",
        ))?;
        push(&mut arguments, low, budget)?;
        if *width == ValueWidth::Two
            && word
                .checked_add(1)
                .and_then(|at| registers.get(at))
                .map(|high| high.0)
                != low.0.checked_add(1)
        {
            return Err(RequestError::InternalInvariant(
                "invoke wide words lost adjacency",
            ));
        }
        word = word
            .checked_add(if *width == ValueWidth::Two { 2 } else { 1 })
            .ok_or(RequestError::PlatformCapacity)?;
    }
    if word != registers.len() {
        return Err(RequestError::InternalInvariant("invoke has surplus words"));
    }
    let constructor = method.declaration.name.matches_with(
        &const { crate::substrate::owned::ascii_units(*b"<init>") },
        budget,
    )?;
    let input_formals =
        super::call_facts::from_reference(index, &method.declaration.prototype, budget)?;
    Ok(CallSite {
        formals: input_formals,
        target: if constructor {
            CallTarget::Constructor
        } else {
            CallTarget::Method
        },
        owner: method.owner_type,
        kind,
        reference,
        result,
        registers,
        arguments,
        widths,
        pc,
        result_pc: None,
    })
}

pub(super) fn initialization(
    index: &ProgramIndex<'_>,
    pc: CodeUnitOffset,
    spec: OpcodeSpec,
    layout: InsnLayout,
    operands: RawOperands,
    calls: &mut Vec<CallSite>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Op>, RequestError> {
    if !matches!(spec.opcode, 0x70 | 0x76) {
        return Ok(None);
    }
    let Some(OperandReference::Method(reference)) = layout.references()[0].map(|r| r.target) else {
        return Err(RequestError::InternalInvariant(
            "direct invoke lost method reference",
        ));
    };
    let method = index.method_reference(reference, budget)?;
    if !method.declaration.name.matches_with(
        &const { crate::substrate::owned::ascii_units(*b"<init>") },
        budget,
    )? {
        return Ok(None);
    }
    if !method
        .declaration
        .prototype
        .return_type
        .matches_with(&[86], budget)?
    {
        return Err(RequestError::InitializationUnavailable { pc });
    }
    crate::p1::binding::check_producer(index, pc, spec, layout, operands, budget)?;
    if let Some(local) = index.local_method(reference, budget)?
        && local.access_flags & crate::substrate::flags::STATIC != 0
    {
        return Err(crate::input::MethodError::Malformed {
            pc,
            problem: crate::input::MethodProblem::InvalidInvokeKind,
        }
        .into());
    }
    let site = call(
        index,
        pc,
        InvokeKind::Direct,
        reference,
        InvokeResult::Void,
        operands,
        budget,
    )?;
    let id = CallId(u32::try_from(calls.len()).map_err(|_| RequestError::PlatformCapacity)?);
    push(calls, site, budget)?;
    Ok(Some(Op::Initialize(id)))
}
