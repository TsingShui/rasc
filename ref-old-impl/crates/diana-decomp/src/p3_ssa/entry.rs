use super::normal::{self, BoundOp, Definition, SsaBody, Sym, ValueId};
use crate::p2_flow::entry::{EntryWindow, ParameterFamily};
use crate::p2_flow::{Op, OpId, ValueWidth, destination};
use crate::substrate::owned::push;
use crate::{RegisterId, RequestError, RunContext};

crate::substrate::checked::checked_product!(EntrySsa, EntrySsaData);
pub(crate) struct EntrySsaData<'a> {
    _sealed: (),
    pub flow: EntryWindow<'a>,
    pub(super) values: Vec<Definition>,
    pub(super) operations: Vec<BoundOp>,
}
pub(crate) enum Graph<'g, 'a> {
    Method {
        ssa: &'g SsaBody<'a>,
        phis: &'g [Option<usize>],
    },
    Entry(&'g EntrySsa<'a>),
}
impl Graph<'_, '_> {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Method { ssa, .. } => ssa.values.len(),
            Self::Entry(ssa) => ssa.values.len(),
        }
    }
    pub(crate) fn definition(&self, value: ValueId) -> Result<Definition, RequestError> {
        let values = match self {
            Self::Method { ssa, .. } => &ssa.values,
            Self::Entry(ssa) => &ssa.values,
        };
        values.get(ix(value.0)?).copied().ok_or_else(broken)
    }
    pub(crate) fn operation(&self, id: OpId) -> Result<&BoundOp, RequestError> {
        match self {
            Self::Method { ssa, .. } => ssa.operation(id),
            Self::Entry(ssa) => ssa.operations.get(ix(id.0)?).ok_or_else(broken),
        }
    }
    pub(crate) fn seed(&self, value: ValueId) -> Result<Option<ValueId>, RequestError> {
        match self {
            Self::Method { ssa, phis } => {
                let Some(index) = phis.get(ix(value.0)?).copied().flatten() else {
                    return Ok(None);
                };
                Ok(
                    match ssa.phis.get(index).ok_or_else(broken)?.inputs.as_slice() {
                        [(normal::Incoming::EntrySeed, Sym::Value(source))] => Some(*source),
                        _ => None,
                    },
                )
            }
            Self::Entry(_) => Ok(None),
        }
    }
    pub(crate) fn parameter(&self, index: u32) -> Result<ParameterFamily, RequestError> {
        let index = ix(index)?;
        match self {
            Self::Entry(ssa) => ssa.flow.parameters.get(index).copied().ok_or_else(broken),
            Self::Method { ssa, .. } => {
                let kind = *ssa.parameters.get(index).ok_or_else(broken)?;
                Ok(if kind.width() == ValueWidth::Two {
                    ParameterFamily::Wide
                } else if kind.field_offset() == 2 {
                    ParameterFamily::Reference
                } else {
                    ParameterFamily::Word
                })
            }
        }
    }
    pub(crate) fn opcode(&self, op: &BoundOp) -> Result<u8, RequestError> {
        let raw = match self {
            Self::Method { ssa, .. } => &ssa.raw,
            Self::Entry(ssa) => &ssa.flow.raw,
        };
        let at = ix(op.pc.0)?
            .checked_mul(2)
            .ok_or(RequestError::PlatformCapacity)?;
        raw.instructions().get(at).copied().ok_or_else(broken)
    }
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn broken() -> RequestError {
    RequestError::InternalInvariant("entry SSA differs from original definition/use order")
}
pub(crate) fn build<'a>(
    _schedule: &crate::engine::SchedulePermit,
    flow: EntryWindow<'a>,
    budget: &mut RunContext<'_>,
) -> Result<EntrySsa<'a>, RequestError> {
    build_impl(flow, budget)
}

fn build_impl<'a>(
    flow: EntryWindow<'a>,
    budget: &mut RunContext<'_>,
) -> Result<EntrySsa<'a>, RequestError> {
    let normal::EntrySeeds {
        mut values,
        mut env,
    } = normal::seed_parameters(
        flow.parameters.len(),
        |i| flow.parameters[i].width(),
        flow.raw.registers_size(),
        budget,
    )?;
    let mut operations = Vec::new();
    for &op in &flow.operations {
        budget.checkpoint()?;
        let inputs = normal::bind_inputs(op, &flow.calls, &[], &env, budget)?;
        let result = normal::define_result(op, &flow.calls, &mut env, &mut values, budget)?;
        push(
            &mut operations,
            BoundOp {
                id: op.id,
                pc: op.pc,
                op: op.kind,
                inputs,
                result,
            },
            budget,
        )?;
    }
    let entry = EntrySsa(EntrySsaData {
        _sealed: (),
        flow,
        values,
        operations,
    });
    verify(&entry, budget)?;
    Ok(entry)
}
pub(crate) fn operations<'s>(entry: &'s EntrySsa<'_>) -> &'s [BoundOp] {
    &entry.operations
}
fn literal32(value: Option<crate::raw::RawLiteral>) -> Option<i32> {
    match value {
        Some(crate::raw::RawLiteral::Bits32(v)) => Some(i32::from_le_bytes(v.to_le_bytes())),
        Some(crate::raw::RawLiteral::Signed8(v)) => Some(i32::from(v)),
        Some(crate::raw::RawLiteral::Signed16(v)) => Some(i32::from(v)),
        _ => None,
    }
}
fn literal64(value: Option<crate::raw::RawLiteral>) -> Option<i64> {
    match value {
        Some(crate::raw::RawLiteral::Bits64(v)) => Some(i64::from_le_bytes(v.to_le_bytes())),
        Some(crate::raw::RawLiteral::Bits32(v)) => {
            Some(i64::from(i32::from_le_bytes(v.to_le_bytes())))
        }
        Some(crate::raw::RawLiteral::Signed16(v)) => Some(i64::from(v)),
        _ => None,
    }
}
pub(crate) fn verify(
    entry: &EntrySsa<'_>,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let raw = &entry.flow.raw;
    let mut widths = Vec::new();
    for definition in &entry.values {
        budget.checkpoint()?;
        let width = match *definition {
            Definition::Parameter(i) => entry
                .flow
                .parameters
                .get(ix(i)?)
                .ok_or_else(broken)?
                .width(),
            Definition::OpResult(id) => entry
                .operations
                .get(ix(id.0)?)
                .ok_or_else(broken)?
                .op
                .result_width(&entry.flow.calls)?,
            _ => return Err(broken()),
        };
        push(&mut widths, width, budget)?;
    }
    let facts = super::word_ownership::WidthFacts::entry(&widths, budget)?;
    let mut env = Vec::new();
    let mut register = RegisterId(u32::from(raw.registers_size() - raw.ins_size()));
    for (i, parameter) in entry.flow.parameters.iter().enumerate() {
        budget.checkpoint()?;
        let value = ValueId(u32::try_from(i).map_err(|_| RequestError::PlatformCapacity)?);
        if entry.values.get(i) != Some(&Definition::Parameter(value.0)) {
            return Err(broken());
        }
        normal::commit(&mut env, register, value, parameter.width(), budget)?;
        register = RegisterId(
            register
                .0
                .checked_add(parameter.width().words())
                .ok_or(RequestError::PlatformCapacity)?,
        );
    }
    let mut pc = crate::CodeUnitOffset(0);
    let mut next_value = entry.flow.parameters.len();
    for (at, op) in entry.operations.iter().enumerate() {
        budget.checkpoint()?;
        if ix(op.id.0)? != at || op.pc != pc {
            return Err(broken());
        }
        let layout = crate::raw::instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let crate::raw::LayoutKind::Instruction(spec) = layout.kind() else {
            return Err(broken());
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let mut registers = operands.registers().iter();
        let matches = match op.op {
            Op::Nop => spec.opcode == 0,
            Op::Move(dst, src) => {
                matches!(spec.opcode,1..=3|7..=9)
                    && registers.next() == Some(dst)
                    && registers.next() == Some(src)
            }
            Op::MoveWide(dst, src) => {
                matches!(spec.opcode, 4..=6)
                    && registers.next() == Some(dst)
                    && registers.next() == Some(src)
            }
            Op::Constant(dst, value) => {
                matches!(spec.opcode, 0x12..=0x15)
                    && registers.next() == Some(dst)
                    && literal32(operands.literal()) == Some(value)
            }
            Op::ConstantWide(dst, value) => {
                matches!(spec.opcode, 0x16..=0x19)
                    && registers.next() == Some(dst)
                    && literal64(operands.literal()) == Some(value)
            }
            Op::Initialize(call) => {
                let site = entry.flow.calls.get(ix(call.0)?).ok_or_else(broken)?;
                let original = layout.references()[0].map(|reference| reference.target);
                if !matches!(spec.opcode, 0x70 | 0x76)
                    || original != Some(crate::raw::OperandReference::Method(site.reference))
                    || site.pc != pc
                    || at + 1 != entry.operations.len()
                {
                    return Err(broken());
                }
                if site.kind != crate::p2_flow::InvokeKind::Direct
                    || site.target != crate::p2_flow::CallTarget::Constructor
                    || !matches!(site.result, crate::p2_flow::InvokeResult::Void)
                    || site.result_pc.is_some()
                    || site.widths.first() != Some(&ValueWidth::One)
                {
                    return Err(broken());
                }
                normal::verify_call_words(site, operands, budget)?;
                for expected in &site.registers {
                    budget.checkpoint()?;
                    if registers.next() != Some(*expected) {
                        return Err(broken());
                    }
                }
                true
            }
            _ => false,
        };
        if !matches || registers.next().is_some() {
            return Err(broken());
        }
        let mut inputs = op.op.reads(&entry.flow.calls, &[])?;
        for (index, actual) in op.inputs.iter().enumerate() {
            budget.checkpoint()?;
            let source = inputs.next().ok_or_else(broken)?;
            let width = op.op.read_width(index, &entry.flow.calls)?;
            if facts.read(&env, source, width, pc, budget)? != *actual {
                return Err(broken());
            }
        }
        if inputs.next().is_some() {
            return Err(broken());
        }
        match (destination(op.op), op.result) {
            (Some(register), Some(value)) => {
                if ix(value.0)? != next_value
                    || entry.values.get(next_value) != Some(&Definition::OpResult(op.id))
                {
                    return Err(broken());
                }
                let width = op.op.result_width(&entry.flow.calls)?;
                if register
                    .0
                    .checked_add(width.words())
                    .is_none_or(|end| end > u32::from(raw.registers_size()))
                {
                    return Err(crate::input::MethodError::Malformed {
                        pc,
                        problem: crate::input::MethodProblem::InvalidRegisterSpan,
                    }
                    .into());
                }
                normal::commit(&mut env, register, value, width, budget)?;
                next_value = next_value
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            }
            (None, None) => {}
            _ => return Err(broken()),
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    if next_value != entry.values.len()
        || !matches!(
            entry.operations.last().map(|op| op.op),
            Some(Op::Initialize(_))
        )
    {
        return Err(broken());
    }
    Ok(())
}
