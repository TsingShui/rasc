use std::collections::TryReserveError;

use thiserror::Error;

use super::formats::{FlowKind, RegisterShape, RegisterWidth, ResultRole, wide_word, word};
use super::{Cursor, InputError, ProgramIndex, RawMethodView};
use crate::raw::{
    DexVersion, Format, InsnLayout, LayoutKind, OpcodeSpec, OperandReference, RawDecodeError,
    instruction_layout,
};
use crate::{
    ByteLength, CodeUnitCount, CodeUnitOffset, ControlError, FailureKind, FallbackBudget,
    MethodBudget,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodProblem {
    MissingEntryInstruction,
    InvalidRegisterSpan,
    InsufficientOutgoingWords,
    InvalidBranchTarget,
    ZeroBranchDelta,
    WrongPayloadKind,
    UnorderedSwitchKeys,
    PackedKeyOverflow,
    InvalidTryBoundary,
    InvalidHandlerTarget,
    InvalidResultEntry,
    InvalidExceptionEntry,
    InvalidFallthrough,
}

#[derive(Debug, Error)]
pub enum MethodError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Decode(#[from] RawDecodeError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error("method view belongs to another input image")]
    ForeignInput,
    #[error("malformed method at {pc:?}: {problem:?}")]
    Malformed {
        pc: CodeUnitOffset,
        problem: MethodProblem,
    },
    #[error("reference {reference:?} at {pc:?} is outside its original table")]
    ReferenceOutOfBounds {
        pc: CodeUnitOffset,
        reference: OperandReference,
    },
    #[error("method scratch capacity is not representable on this platform")]
    PlatformCapacity,
    #[error("method scratch allocation failed: {0}")]
    Allocation(#[source] TryReserveError),
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

impl MethodError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(error) => error.kind(),
            Self::Decode(error) => error.kind(),
            Self::Control(error) => error.kind(),
            Self::ForeignInput | Self::Malformed { .. } | Self::ReferenceOutOfBounds { .. } => {
                FailureKind::Malformed
            }
            Self::PlatformCapacity | Self::Allocation(_) => FailureKind::BudgetExceeded,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}

fn bad(pc: CodeUnitOffset, problem: MethodProblem) -> MethodError {
    MethodError::Malformed { pc, problem }
}

/// Structural checks over all original words, including dead instructions.
/// This is not ValidatedRawMethod: debug metadata, callee/result type binding and
/// register definition/type analysis retain their own validation obligations.
#[derive(Debug)]
pub struct CheckedMethodLayout<'a> {
    original: RawMethodView<'a>,
    boundaries: Boundaries,
}

impl<'a> CheckedMethodLayout<'a> {
    pub fn original(&self) -> &RawMethodView<'a> {
        &self.original
    }

    /// Queries an original instruction boundary, not a semantic CFG leader.
    pub fn is_instruction_start(&self, pc: CodeUnitOffset) -> Result<bool, MethodError> {
        Ok(self.boundaries.start(pc)? == Start::Instruction)
    }
}

impl<'a> ProgramIndex<'a> {
    /// Checks registers, reference bounds, branch/payload/try/handler boundaries,
    /// result/exception entry positions and reachable fallthrough. Only the
    /// matching original image supplies reference bounds. Work and scratch use
    /// the normal method budget; failure leaves the borrowed original intact.
    pub fn check_method_layout(
        &self,
        original: &RawMethodView<'a>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<CheckedMethodLayout<'a>, MethodError> {
        budget.checkpoint()?;
        if !std::ptr::eq(self.bytes, original.source_bytes) {
            return Err(MethodError::ForeignInput);
        }
        budget.check_code_units(original.code_units())?;
        budget.charge_work(1)?;
        let mut boundaries = Boundaries::new(original.code_units(), budget)?;
        let mut instruction_count = 0usize;
        let mut pc = CodeUnitOffset(0);
        while pc.0 < original.code_units().0 {
            budget.charge_work(1)?;
            let layout = original.layout_at(pc)?;
            let start = match layout.kind() {
                LayoutKind::Instruction(_) => {
                    instruction_count = instruction_count
                        .checked_add(1)
                        .ok_or(MethodError::PlatformCapacity)?;
                    Start::Instruction
                }
                _ => {
                    check_keys(original, pc, layout.kind(), budget)?;
                    Start::Payload
                }
            };
            boundaries.set_start(pc, start)?;
            pc = advance(pc, layout)?;
        }
        let mut checker = Checker {
            original,
            boundaries,
        };
        checker.check_exceptions(budget)?;
        checker.check_instructions(self, budget)?;
        checker.check_reachability(instruction_count, budget)?;
        budget.checkpoint()?;
        Ok(CheckedMethodLayout {
            original: original.clone(),
            boundaries: checker.boundaries,
        })
    }

    fn reference_in_bounds(&self, reference: OperandReference) -> bool {
        match reference {
            OperandReference::String(id) => id.0 < self.header.string_ids.count,
            OperandReference::Type(id) => id.0 < self.header.type_ids.count,
            OperandReference::Field(id) => id.0 < self.header.field_ids.count,
            OperandReference::Method(id) => id.0 < self.header.method_ids.count,
            OperandReference::Proto(id) => id.0 < self.header.proto_ids.count,
            // ProgramIndex currently accepts 035 only; newer opcodes fail the
            // shared version check before references reach this boundary.
            OperandReference::CallSite(_) | OperandReference::MethodHandle(_) => false,
        }
    }
}

impl RawMethodView<'_> {
    /// Visits original layouts in PC order using independent fallback work.
    /// A decoding failure retains the delivered prefix, but never guesses a
    /// width or claims semantic validation. Callback output uses the supplied
    /// fallback budget. Cancellation stops before the next layout/callback.
    pub fn visit_layouts(
        &self,
        budget: &mut FallbackBudget<'_, '_>,
        mut visit: impl FnMut(
            CodeUnitOffset,
            InsnLayout,
            &mut FallbackBudget<'_, '_>,
        ) -> Result<(), MethodError>,
    ) -> Result<(), MethodError> {
        budget.check_code_units(self.code_units())?;
        let mut pc = CodeUnitOffset(0);
        while pc.0 < self.code_units().0 {
            budget.charge_work(1)?;
            let layout = self.layout_at(pc)?;
            budget.checkpoint()?;
            visit(pc, layout, budget)?;
            pc = advance(pc, layout)?;
        }
        budget.checkpoint()?;
        Ok(())
    }

    fn layout_at(&self, pc: CodeUnitOffset) -> Result<InsnLayout, MethodError> {
        // The range reader constructs RawMethodView only from accepted 035 images.
        Ok(instruction_layout(
            self.instructions(),
            pc,
            DexVersion::V035,
        )?)
    }

    fn signed_word(&self, pc: CodeUnitOffset, delta: u32) -> Result<i32, MethodError> {
        Ok(i32::from_le_bytes(
            wide_word(self.instructions(), pc, delta)?.to_le_bytes(),
        ))
    }
}

fn advance(pc: CodeUnitOffset, layout: InsnLayout) -> Result<CodeUnitOffset, MethodError> {
    pc.checked_advance(layout.width())
        .ok_or(MethodError::InternalInvariant(
            "checked layout extent overflowed",
        ))
}

fn relative(pc: CodeUnitOffset, delta: i32) -> Result<CodeUnitOffset, MethodError> {
    u32::try_from(i64::from(pc.0) + i64::from(delta))
        .map(CodeUnitOffset)
        .map_err(|_| bad(pc, MethodProblem::InvalidBranchTarget))
}

fn exact_buffer<T>(count: usize, budget: &mut MethodBudget<'_, '_>) -> Result<Vec<T>, MethodError> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(MethodError::PlatformCapacity)?;
    if bytes > isize::MAX as usize {
        return Err(MethodError::PlatformCapacity);
    }
    budget.charge_allocation(u64::try_from(bytes).map_err(|_| MethodError::PlatformCapacity)?)?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(count)
        .map_err(MethodError::Allocation)?;
    // Both callers use non-ZST elements and fresh Global vectors. Their exact
    // reservation exposes the requested capacity; no amortized growth is used.
    if buffer.capacity() != count {
        return Err(MethodError::InternalInvariant(
            "exact scratch reservation changed capacity",
        ));
    }
    Ok(buffer)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Start {
    Absent,
    Instruction,
    Payload,
}

#[derive(Debug)]
struct Boundaries {
    units: CodeUnitCount,
    // A nibble per code unit: start kind (2 bits), handler entry, reached.
    bits: Vec<u8>,
}

impl Boundaries {
    fn new(units: CodeUnitCount, budget: &mut MethodBudget<'_, '_>) -> Result<Self, MethodError> {
        let count = usize::try_from(units.0)
            .map_err(|_| MethodError::PlatformCapacity)?
            .div_ceil(2);
        let mut bits = exact_buffer(count, budget)?;
        while bits.len() < count {
            budget.charge_work(1)?;
            let end = bits.len() + (count - bits.len()).min(65536);
            bits.resize(end, 0);
        }
        Ok(Self { units, bits })
    }

    fn flags(&self, pc: CodeUnitOffset) -> u8 {
        if pc.0 >= self.units.0 {
            return 0;
        }
        // Allocation used the checked conversion of the larger code-unit count.
        let index = (pc.0 / 2) as usize;
        (self.bits[index] >> ((pc.0 % 2) * 4)) & 15
    }

    fn start(&self, pc: CodeUnitOffset) -> Result<Start, MethodError> {
        match self.flags(pc) & 3 {
            0 => Ok(Start::Absent),
            1 => Ok(Start::Instruction),
            2 => Ok(Start::Payload),
            _ => Err(MethodError::InternalInvariant(
                "overlapping instruction and payload starts",
            )),
        }
    }

    fn set_start(&mut self, pc: CodeUnitOffset, start: Start) -> Result<(), MethodError> {
        self.mark(
            pc,
            match start {
                Start::Instruction => 1,
                Start::Payload => 2,
                Start::Absent => 0,
            },
        )
    }

    fn mark(&mut self, pc: CodeUnitOffset, flags: u8) -> Result<(), MethodError> {
        if pc.0 >= self.units.0 {
            return Err(MethodError::InternalInvariant(
                "boundary mark outside checked code",
            ));
        }
        let index = usize::try_from(pc.0 / 2).map_err(|_| MethodError::PlatformCapacity)?;
        let byte = self
            .bits
            .get_mut(index)
            .ok_or(MethodError::InternalInvariant("missing boundary storage"))?;
        *byte |= flags << ((pc.0 % 2) * 4);
        Ok(())
    }
}

fn check_keys(
    original: &RawMethodView<'_>,
    pc: CodeUnitOffset,
    kind: LayoutKind,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), MethodError> {
    match kind {
        LayoutKind::PackedSwitch { entries } if entries > 0 => {
            let first = original.signed_word(pc, 2)?;
            if i64::from(first) + i64::from(entries) - 1 > i64::from(i32::MAX) {
                return Err(bad(pc, MethodProblem::PackedKeyOverflow));
            }
        }
        LayoutKind::SparseSwitch { entries } => {
            let mut previous = None;
            for index in 0..u32::from(entries) {
                budget.charge_work(1)?;
                let key = original.signed_word(pc, 2 + 2 * index)?;
                if previous.is_some_and(|old| old >= key) {
                    return Err(bad(pc, MethodProblem::UnorderedSwitchKeys));
                }
                previous = Some(key);
            }
        }
        _ => {}
    }
    Ok(())
}

struct Checker<'m, 'a> {
    original: &'m RawMethodView<'a>,
    boundaries: Boundaries,
}

impl<'a> Checker<'_, 'a> {
    fn check_exceptions(&mut self, budget: &mut MethodBudget<'_, '_>) -> Result<(), MethodError> {
        let range = self.original.handler_byte_range();
        if range.start == range.end {
            return Ok(());
        }
        let mut handlers = Cursor::bounded(self.original.source_bytes, range.start, range.end)?;
        let count = handlers.read_uleb32_charged(|| budget.charge_work(1))?;
        for _ in 0..count {
            budget.charge_work(1)?;
            visit_handler(&mut handlers, budget, |pc| {
                if self.boundaries.start(pc)? != Start::Instruction
                    || self.spec_at(pc)?.result == ResultRole::MoveResult
                {
                    return Err(bad(pc, MethodProblem::InvalidHandlerTarget));
                }
                self.boundaries.mark(pc, 4)
            })?;
        }
        let tries = self.original.try_byte_range();
        let mut cursor = Cursor::bounded(self.original.source_bytes, tries.start, tries.end)?;
        while cursor.remaining_byte() != ByteLength(0) {
            budget.charge_work(1)?;
            let record = read_try(&mut cursor, budget)?;
            if self.boundaries.start(record.start)? != Start::Instruction
                || (record.end.0 != self.original.code_units().0
                    && self.boundaries.start(record.end)? == Start::Absent)
            {
                return Err(bad(record.start, MethodProblem::InvalidTryBoundary));
            }
        }
        Ok(())
    }

    fn spec_at(&self, pc: CodeUnitOffset) -> Result<OpcodeSpec, MethodError> {
        match self.original.layout_at(pc)?.kind() {
            LayoutKind::Instruction(spec) => Ok(spec),
            _ => Err(MethodError::InternalInvariant(
                "instruction lookup reached payload",
            )),
        }
    }

    fn branch_target(
        &self,
        pc: CodeUnitOffset,
        spec: OpcodeSpec,
    ) -> Result<CodeUnitOffset, MethodError> {
        let bytes = self.original.instructions();
        let delta = match spec.format {
            Format::F10t => i32::from(i8::from_le_bytes([word(bytes, pc, 0)?.to_le_bytes()[1]])),
            Format::F20t | Format::F21t | Format::F22t => {
                i32::from(i16::from_le_bytes(word(bytes, pc, 1)?.to_le_bytes()))
            }
            Format::F30t => self.original.signed_word(pc, 1)?,
            _ => return Err(MethodError::InternalInvariant("branch format has no delta")),
        };
        if delta == 0 && spec.format != Format::F30t {
            return Err(bad(pc, MethodProblem::ZeroBranchDelta));
        }
        let target = relative(pc, delta)?;
        self.normal_target(pc, target, true)?;
        Ok(target)
    }

    fn normal_target(
        &self,
        source: CodeUnitOffset,
        target: CodeUnitOffset,
        explicit: bool,
    ) -> Result<(), MethodError> {
        if self.boundaries.start(target)? != Start::Instruction {
            return Err(bad(
                source,
                if explicit {
                    MethodProblem::InvalidBranchTarget
                } else {
                    MethodProblem::InvalidFallthrough
                },
            ));
        }
        match self.spec_at(target)?.result {
            ResultRole::MoveResult if explicit => {
                Err(bad(source, MethodProblem::InvalidResultEntry))
            }
            ResultRole::MoveException => Err(bad(source, MethodProblem::InvalidExceptionEntry)),
            _ => Ok(()),
        }
    }

    fn payload_at(
        &self,
        pc: CodeUnitOffset,
        flow: FlowKind,
    ) -> Result<(CodeUnitOffset, LayoutKind), MethodError> {
        let target = relative(pc, self.original.signed_word(pc, 1)?)?;
        if self.boundaries.start(target)? != Start::Payload {
            return Err(bad(pc, MethodProblem::WrongPayloadKind));
        }
        let kind = self.original.layout_at(target)?.kind();
        if !matches!(
            (flow, kind),
            (FlowKind::PackedSwitch, LayoutKind::PackedSwitch { .. })
                | (FlowKind::SparseSwitch, LayoutKind::SparseSwitch { .. })
                | (FlowKind::FillArray, LayoutKind::ArrayData { .. })
        ) {
            return Err(bad(pc, MethodProblem::WrongPayloadKind));
        }
        Ok((target, kind))
    }

    fn switch_targets(
        &self,
        pc: CodeUnitOffset,
        flow: FlowKind,
    ) -> Result<SwitchTargets<'a>, MethodError> {
        let (payload, kind) = self.payload_at(pc, flow)?;
        let (entries, base) = match kind {
            LayoutKind::PackedSwitch { entries } => (u32::from(entries), 4),
            LayoutKind::SparseSwitch { entries } => {
                (u32::from(entries), 2 + 2 * u32::from(entries))
            }
            _ => {
                return Err(MethodError::InternalInvariant(
                    "switch lookup reached array payload",
                ));
            }
        };
        Ok(SwitchTargets {
            bytes: self.original.instructions(),
            source: pc,
            payload,
            entries,
            base,
        })
    }

    fn check_instructions(
        &self,
        index: &ProgramIndex<'_>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<(), MethodError> {
        let mut pc = CodeUnitOffset(0);
        let mut previous = None;
        while pc.0 < self.original.code_units().0 {
            budget.charge_work(1)?;
            let layout = self.original.layout_at(pc)?;
            if let LayoutKind::Instruction(spec) = layout.kind() {
                check_registers(self.original, pc, spec, budget)?;
                for reference in layout.references().into_iter().flatten() {
                    budget.charge_work(1)?;
                    if !index.reference_in_bounds(reference.target) {
                        return Err(MethodError::ReferenceOutOfBounds {
                            pc,
                            reference: reference.target,
                        });
                    }
                }
                match spec.result {
                    ResultRole::MoveResult => {
                        let valid = match previous {
                            Some(RegisterShape::InvokeWords) => true,
                            Some(RegisterShape::ArrayWords) => spec.opcode == 0x0c,
                            _ => false,
                        };
                        if !valid {
                            return Err(bad(pc, MethodProblem::InvalidResultEntry));
                        }
                    }
                    ResultRole::MoveException if self.boundaries.flags(pc) & 4 == 0 => {
                        return Err(bad(pc, MethodProblem::InvalidExceptionEntry));
                    }
                    _ => {}
                }
                match spec.flow {
                    FlowKind::Goto | FlowKind::Conditional => {
                        self.branch_target(pc, spec)?;
                    }
                    FlowKind::PackedSwitch | FlowKind::SparseSwitch => {
                        let targets = self.switch_targets(pc, spec.flow)?;
                        for index in 0..targets.entries {
                            budget.charge_work(1)?;
                            self.normal_target(pc, targets.at(index)?, true)?;
                        }
                    }
                    FlowKind::FillArray => {
                        self.payload_at(pc, spec.flow)?;
                    }
                    _ => {}
                }
                previous = Some(spec.registers);
            } else {
                previous = None;
            }
            pc = advance(pc, layout)?;
        }
        Ok(())
    }

    fn enqueue(
        &mut self,
        pc: CodeUnitOffset,
        pending: &mut Vec<CodeUnitOffset>,
    ) -> Result<(), MethodError> {
        if self.boundaries.flags(pc) & 8 == 0 {
            if pending.len() == pending.capacity() {
                return Err(MethodError::InternalInvariant(
                    "reachability worklist exceeded instruction count",
                ));
            }
            self.boundaries.mark(pc, 8)?;
            pending.push(pc);
        }
        Ok(())
    }

    fn check_reachability(
        &mut self,
        instruction_count: usize,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<(), MethodError> {
        let entry = CodeUnitOffset(0);
        if self.boundaries.start(entry)? != Start::Instruction {
            return Err(bad(entry, MethodProblem::MissingEntryInstruction));
        }
        self.normal_target(entry, entry, true)?;
        let mut pending = exact_buffer(instruction_count, budget)?;
        self.enqueue(entry, &mut pending)?;
        while let Some(pc) = pending.pop() {
            budget.charge_work(1)?;
            let layout = self.original.layout_at(pc)?;
            let LayoutKind::Instruction(spec) = layout.kind() else {
                return Err(MethodError::InternalInvariant(
                    "reachability entered a payload",
                ));
            };
            match spec.flow {
                FlowKind::Goto | FlowKind::Conditional => {
                    let target = self.branch_target(pc, spec)?;
                    self.enqueue(target, &mut pending)?;
                }
                FlowKind::PackedSwitch | FlowKind::SparseSwitch => {
                    let targets = self.switch_targets(pc, spec.flow)?;
                    for index in 0..targets.entries {
                        budget.charge_work(1)?;
                        self.enqueue(targets.at(index)?, &mut pending)?;
                    }
                }
                _ => {}
            }
            if !matches!(
                spec.flow,
                FlowKind::Return | FlowKind::Throw | FlowKind::Goto
            ) {
                let next = advance(pc, layout)?;
                self.normal_target(pc, next, false)?;
                self.enqueue(next, &mut pending)?;
            }
            if spec.may_throw {
                self.enqueue_handlers(pc, &mut pending, budget)?;
            }
        }
        Ok(())
    }

    fn enqueue_handlers(
        &mut self,
        pc: CodeUnitOffset,
        pending: &mut Vec<CodeUnitOffset>,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<(), MethodError> {
        let range = self.original.try_byte_range();
        let mut lower = 0;
        let mut upper = (range.end.0 - range.start.0) / 8;
        while lower < upper {
            budget.charge_work(1)?;
            let middle = lower + (upper - lower) / 2;
            let start = range
                .start
                .checked_advance(ByteLength(middle * 8))
                .ok_or(MethodError::InternalInvariant("try offset overflow"))?;
            let mut cursor = Cursor::bounded(self.original.source_bytes, start, range.end)?;
            let record = read_try(&mut cursor, budget)?;
            if pc < record.start {
                upper = middle;
            } else if pc >= record.end {
                lower = middle + 1;
            } else {
                let handlers = self.original.handler_byte_range();
                let start = handlers
                    .start
                    .checked_advance(ByteLength(usize::from(record.handler_offset)))
                    .ok_or(MethodError::InternalInvariant("handler offset overflow"))?;
                let mut cursor = Cursor::bounded(self.original.source_bytes, start, handlers.end)?;
                return visit_handler(&mut cursor, budget, |target| self.enqueue(target, pending));
            }
        }
        Ok(())
    }
}

fn check_registers(
    original: &RawMethodView<'_>,
    pc: CodeUnitOffset,
    spec: OpcodeSpec,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), MethodError> {
    let bytes = original.instructions();
    let first = word(bytes, pc, 0)?;
    let a8 = first >> 8;
    let a4 = a8 & 15;
    let b4 = first >> 12;
    let span = |start: u16, count: u16| -> Result<(), MethodError> {
        if count != 0 && u32::from(start) + u32::from(count) > u32::from(original.registers_size())
        {
            Err(bad(pc, MethodProblem::InvalidRegisterSpan))
        } else {
            Ok(())
        }
    };
    match spec.registers {
        RegisterShape::InvokeWords | RegisterShape::ArrayWords => {
            let count = match spec.format {
                Format::F35c | Format::F45cc => b4,
                Format::F3rc | Format::F4rcc => a8,
                _ => {
                    return Err(MethodError::InternalInvariant(
                        "argument list has no word-count format",
                    ));
                }
            };
            if spec.registers == RegisterShape::InvokeWords && count > original.outs_size() {
                return Err(bad(pc, MethodProblem::InsufficientOutgoingWords));
            }
            if matches!(spec.format, Format::F3rc | Format::F4rcc) {
                span(word(bytes, pc, 2)?, count)?;
            } else {
                let packed = word(bytes, pc, 2)?;
                let registers = [
                    packed & 15,
                    (packed >> 4) & 15,
                    (packed >> 8) & 15,
                    packed >> 12,
                    a4,
                ];
                for register in registers.into_iter().take(usize::from(count)) {
                    budget.charge_work(1)?;
                    span(register, 1)?;
                }
            }
        }
        RegisterShape::Fixed(widths) => {
            let registers = match spec.format {
                Format::F10x | Format::F10t | Format::F20t | Format::F30t => [None; 3],
                Format::F12x | Format::F22t | Format::F22s | Format::F22c => {
                    [Some(a4), Some(b4), None]
                }
                Format::F22x => [Some(a8), Some(word(bytes, pc, 1)?), None],
                Format::F32x => [Some(word(bytes, pc, 1)?), Some(word(bytes, pc, 2)?), None],
                Format::F23x => {
                    let bc = word(bytes, pc, 1)?;
                    [Some(a8), Some(bc & 255), Some(bc >> 8)]
                }
                Format::F22b => [Some(a8), Some(word(bytes, pc, 1)? & 255), None],
                Format::F11n => [Some(a4), None, None],
                _ => [Some(a8), None, None],
            };
            for (register, width) in registers.into_iter().zip(widths) {
                budget.charge_work(1)?;
                match (register, width) {
                    (Some(register), Some(width)) => span(
                        register,
                        match width {
                            RegisterWidth::One => 1,
                            RegisterWidth::Two => 2,
                        },
                    )?,
                    (None, None) => {}
                    _ => {
                        return Err(MethodError::InternalInvariant(
                            "register format and span specification disagree",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

struct SwitchTargets<'a> {
    bytes: &'a [u8],
    source: CodeUnitOffset,
    payload: CodeUnitOffset,
    entries: u32,
    base: u32,
}

impl SwitchTargets<'_> {
    fn at(&self, index: u32) -> Result<CodeUnitOffset, MethodError> {
        if index >= self.entries {
            return Err(MethodError::InternalInvariant(
                "switch target index outside payload",
            ));
        }
        // The u16 entry count bounds this displacement below 4 * 65535 + 2.
        let bits = wide_word(self.bytes, self.payload, self.base + 2 * index)?;
        relative(self.source, i32::from_le_bytes(bits.to_le_bytes()))
    }
}

struct TryRecord {
    start: CodeUnitOffset,
    end: CodeUnitOffset,
    handler_offset: u16,
}

fn read_try(
    cursor: &mut Cursor<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<TryRecord, MethodError> {
    let start = CodeUnitOffset(u32::from_le_bytes(
        cursor.read_array_charged(|| budget.charge_work(1))?,
    ));
    let count = u16::from_le_bytes(cursor.read_array_charged(|| budget.charge_work(1))?);
    let handler_offset = u16::from_le_bytes(cursor.read_array_charged(|| budget.charge_work(1))?);
    let end = start
        .checked_advance(CodeUnitCount(u32::from(count)))
        .ok_or(MethodError::InternalInvariant(
            "checked try extent overflowed",
        ))?;
    Ok(TryRecord {
        start,
        end,
        handler_offset,
    })
}

fn visit_handler(
    cursor: &mut Cursor<'_>,
    budget: &mut MethodBudget<'_, '_>,
    mut visit: impl FnMut(CodeUnitOffset) -> Result<(), MethodError>,
) -> Result<(), MethodError> {
    let size = cursor.read_sleb32_charged(|| budget.charge_work(1))?;
    for _ in 0..i64::from(size).unsigned_abs() {
        budget.charge_work(1)?;
        cursor.read_uleb32_charged(|| budget.charge_work(1))?;
        let target = CodeUnitOffset(cursor.read_uleb32_charged(|| budget.charge_work(1))?);
        visit(target)?;
    }
    if size <= 0 {
        visit(CodeUnitOffset(
            cursor.read_uleb32_charged(|| budget.charge_work(1))?,
        ))?;
    }
    Ok(())
}
