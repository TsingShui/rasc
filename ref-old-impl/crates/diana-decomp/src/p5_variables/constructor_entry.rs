use crate::p2_flow::{MethodFrame, Op};
use crate::p3_ssa::{Definition, Sym, ValueId};
use crate::p4_types::TypedBody;
use crate::p4_types::initialization::{ChainArgument, ChainKind};
use crate::{CodeUnitOffset, RegisterId, RequestError, RunContext};

pub(crate) struct ConstructorEntry {
    _sealed: (),
    pub pc: CodeUnitOffset,
    pub receiver: RegisterId,
    pub reference: crate::MethodId,
    pub kind: ChainKind,
    pub arguments: Vec<ChainArgument>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn original_argument(
    typed: &TypedBody<'_>,
    phis: &[Option<usize>],
    mut value: ValueId,
    expected: ChainArgument,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for _ in &typed.ssa.values {
        budget.checkpoint()?;
        match typed.ssa.values[ix(value.0)?] {
            Definition::Parameter(parameter) if parameter > 0 => {
                return Ok(
                    matches!(expected,ChainArgument::Parameter(index) if index.checked_add(1) == Some(parameter)),
                );
            }
            Definition::OpResult(id) => {
                let op = typed.ssa.operation(id)?;
                match op.op {
                    Op::Move(..) | Op::MoveWide(..) => value = op.input(0)?,
                    Op::Constant(register, literal) => {
                        let raw = &typed.ssa.raw;
                        let layout = crate::raw::instruction_layout(
                            raw.instructions(),
                            op.pc,
                            raw.version(),
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let operands = crate::input::operands::instruction_operands(
                            raw.instructions(),
                            op.pc,
                            layout,
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let bits = match operands.literal() {
                            Some(crate::raw::RawLiteral::Bits32(bits)) => {
                                i32::from_le_bytes(bits.to_le_bytes())
                            }
                            Some(crate::raw::RawLiteral::Signed8(bits)) => i32::from(bits),
                            Some(crate::raw::RawLiteral::Signed16(bits)) => i32::from(bits),
                            _ => return Ok(false),
                        };
                        return Ok(
                            matches!(expected,ChainArgument::Literal(expected) if expected == literal && literal == bits)
                                && matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x12..=0x15))
                                && operands.registers().iter().next() == Some(register),
                        );
                    }
                    Op::ConstantWide(register, literal) => {
                        let raw = &typed.ssa.raw;
                        let layout = crate::raw::instruction_layout(
                            raw.instructions(),
                            op.pc,
                            raw.version(),
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let operands = crate::input::operands::instruction_operands(
                            raw.instructions(),
                            op.pc,
                            layout,
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let bits = match operands.literal() {
                            Some(crate::raw::RawLiteral::Bits64(bits)) => {
                                i64::from_le_bytes(bits.to_le_bytes())
                            }
                            Some(crate::raw::RawLiteral::Bits32(bits)) => {
                                i64::from(i32::from_le_bytes(bits.to_le_bytes()))
                            }
                            Some(crate::raw::RawLiteral::Signed16(bits)) => i64::from(bits),
                            _ => return Ok(false),
                        };
                        return Ok(
                            matches!(expected, ChainArgument::WideLiteral(expected) if expected == literal && literal == bits)
                                && matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode, 0x16..=0x19))
                                && operands.registers().iter().next() == Some(register),
                        );
                    }
                    _ => return Ok(false),
                }
            }
            Definition::Phi => {
                let Some(index) = phis[ix(value.0)?] else {
                    return Ok(false);
                };
                let inputs = &typed.ssa.phis[index].inputs;
                if inputs.len() != 1 {
                    return Ok(false);
                }
                let (crate::p3_ssa::Incoming::EntrySeed, Sym::Value(source)) = inputs[0] else {
                    return Ok(false);
                };
                value = source;
            }
            _ => return Ok(false),
        }
    }
    Ok(false)
}
pub(super) fn prove(
    typed: &TypedBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Option<ConstructorEntry>, RequestError> {
    let Some(chain) = typed.initializations.chain.as_ref() else {
        return Ok(None);
    };
    let broken = || {
        RequestError::InternalInvariant(
            "constructor entry differs from original UninitThis or arguments",
        )
    };
    if !matches!(typed.ssa.frame, MethodFrame::Constructor(_)) {
        return Err(broken());
    }
    let op = typed.ssa.operation(chain.init)?;
    if !matches!(op.op,Op::Initialize(call) if call == chain.call)
        || chain.arguments.len().checked_add(1) != Some(op.inputs.len())
    {
        return Err(broken());
    }
    let site = &typed.ssa.calls[ix(chain.call.0)?];
    let raw = &typed.ssa.raw;
    let layout = crate::raw::instruction_layout(raw.instructions(), op.pc, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x70|0x76))
        || !matches!(layout.references()[0],Some(reference) if reference.target == crate::raw::OperandReference::Method(site.reference))
    {
        return Err(broken());
    }
    let mut phis = Vec::new();
    for _ in &typed.ssa.values {
        budget.checkpoint()?;
        crate::substrate::owned::push(&mut phis, None, budget)?;
    }
    for (index, phi) in typed.ssa.phis.iter().enumerate() {
        budget.checkpoint()?;
        phis[ix(phi.result.0)?] = Some(index);
    }
    let mut arguments = Vec::new();
    for (&value, &expected) in op.inputs.iter().skip(1).zip(&chain.arguments) {
        budget.checkpoint()?;
        if !original_argument(typed, &phis, value, expected, budget)? {
            return Err(broken());
        }
        crate::substrate::owned::push(&mut arguments, expected, budget)?;
    }
    for &id in &typed.ssa.bound[0].ops {
        budget.checkpoint()?;
        let prefix = typed.ssa.operation(id)?;
        if prefix.pc.0 >= op.pc.0 {
            break;
        }
        if let Some(capture) = &typed.capture
            && capture.operation() == prefix.id
        {
            if !matches!(prefix.op, Op::FieldStore(_, _, field, _) if field == capture.field())
                || prefix.pc != crate::CodeUnitOffset(0)
                || capture.initialization() != op.pc
            {
                return Err(broken());
            }
            let layout =
                crate::raw::instruction_layout(raw.instructions(), prefix.pc, raw.version())
                    .map_err(crate::input::MethodError::Decode)?;
            let operands =
                crate::input::operands::instruction_operands(raw.instructions(), prefix.pc, layout)
                    .map_err(crate::input::MethodError::Decode)?;
            let first = raw
                .registers_size()
                .checked_sub(raw.ins_size())
                .ok_or_else(broken)?;
            let mut registers = operands.registers().iter();
            if raw.ins_size() != 2
                || !matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x5b)
                || registers.next() != Some(crate::RegisterId(u32::from(first) + 1))
                || registers.next() != Some(crate::RegisterId(u32::from(first)))
                || registers.next().is_some()
            {
                return Err(broken());
            }
        }
        if matches!(prefix.op, Op::Move(..) | Op::MoveWide(..)) {
            let result = prefix.result.ok_or_else(broken)?;
            let is_this = typed.initializations.deferred.get(ix(result.0)?)
                == Some(&Some(
                    crate::p4_types::initialization::InitializationId::This,
                ));
            let layout =
                crate::raw::instruction_layout(raw.instructions(), prefix.pc, raw.version())
                    .map_err(crate::input::MethodError::Decode)?;
            let kind = typed.kinds[ix(result.0)?].ok_or_else(broken)?;
            if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if if is_this { matches!(spec.opcode,7..=9) } else if matches!(kind, crate::p4_types::Kind::Long | crate::p4_types::Kind::Double) { matches!(spec.opcode,4..=6) } else if matches!(kind, crate::p4_types::Kind::Int | crate::p4_types::Kind::Boolean | crate::p4_types::Kind::Float) { matches!(spec.opcode,1..=3) } else { matches!(spec.opcode,7..=9) })
            {
                return Err(RequestError::InitializationUnavailable { pc: prefix.pc });
            }
        }
    }
    for alias in &chain.aliases {
        budget.checkpoint()?;
        let value = alias.value();
        match typed.ssa.values[ix(value.0)?] {
            Definition::Parameter(0) | Definition::Phi => {}
            Definition::OpResult(id) => {
                let alias = typed.ssa.operation(id)?;
                let layout =
                    crate::raw::instruction_layout(raw.instructions(), alias.pc, raw.version())
                        .map_err(crate::input::MethodError::Decode)?;
                if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,7..=9))
                {
                    return Err(RequestError::InitializationUnavailable { pc: alias.pc });
                }
            }
            _ => return Err(broken()),
        }
    }
    Ok(Some(ConstructorEntry {
        _sealed: (),
        pc: op.pc,
        receiver: *site.registers.first().ok_or_else(broken)?,
        reference: site.reference,
        kind: chain.kind,
        arguments,
    }))
}
