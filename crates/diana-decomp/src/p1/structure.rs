use std::collections::TryReserveError;

use thiserror::Error;

use super::InvokeLayout;
use super::binding::{InvokeIssue, ResultKind, check_producer};
use crate::input::formats::{FlowKind, RegisterShape, RegisterWidth, ResultRole, wide_word};
use crate::input::operands::{RawOperands, RegisterEncoding};
use crate::input::{Cursor, InputError, ProgramIndex, RawMethodView};
use crate::raw::{
    Format, InsnLayout, LayoutKind, OpcodeSpec, OperandReference, RawDecodeError,
    instruction_layout,
};
use crate::{ByteLength, CodeUnitCount, CodeUnitOffset, FailureKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodProblem {
    MissingEntryInstruction,
    InvalidRegisterSpan,
    InvalidTypeOperand,
    InvalidArrayPayloadType,
    InvalidArrayAccessType,
    InsufficientOutgoingWords,
    InvalidInvokeWordCount,
    InvalidInvokeKind,
    InvalidFieldKind,
    NonAdjacentWideArgument,
    InvalidResultKind,
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
    InvalidEntryLayout,
}

#[derive(Debug, Error)]
pub enum MethodError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Decode(#[from] RawDecodeError),
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

            Self::ForeignInput | Self::Malformed { .. } | Self::ReferenceOutOfBounds { .. } => {
                FailureKind::Malformed
            }
            Self::PlatformCapacity | Self::Allocation(_) => FailureKind::ResourceExhausted,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}

pub(super) fn bad(pc: CodeUnitOffset, problem: MethodProblem) -> MethodError {
    MethodError::Malformed { pc, problem }
}

#[derive(Debug)]
struct InvokeFact {
    pc: CodeUnitOffset,
    layout: Result<InvokeLayout, InvokeIssue>,
}

#[derive(Debug)]
struct InstructionIssue {
    pc: CodeUnitOffset,
    result: bool,
    error: MethodError,
}

/// Original boundary/control associations and per-instruction layout states.
/// Registers, result uses and argument grouping retain local issues; an unreliable
/// boundary or control relation cannot be published as a known execution graph.
#[derive(Debug)]
pub struct CheckedMethodLayout<'a> {
    original: RawMethodView<'a>,
    boundaries: Boundaries,
    invokes: Vec<InvokeFact>,
    issues: Vec<InstructionIssue>,
}

impl<'a> CheckedMethodLayout<'a> {
    pub(crate) fn is_reachable(&self, pc: CodeUnitOffset) -> bool {
        self.boundaries.flags(pc) & 8 != 0
    }

    pub fn original(&self) -> &RawMethodView<'a> {
        &self.original
    }

    /// Checked invoke layout at `pc`, in original PC order.
    pub(super) fn invoke_layout_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Option<Result<&InvokeLayout, &InvokeIssue>> {
        self.invokes
            .binary_search_by_key(&pc, |fact| fact.pc)
            .ok()
            .map(|index| self.invokes[index].layout.as_ref())
    }

    /// All checked invoke layouts in original PC order.
    pub(super) fn invoke_layouts(
        &self,
    ) -> impl Iterator<Item = (CodeUnitOffset, Result<&InvokeLayout, &InvokeIssue>)> + '_ {
        self.invokes
            .iter()
            .map(|fact| (fact.pc, fact.layout.as_ref()))
    }
    pub fn issues(&self) -> impl Iterator<Item = (CodeUnitOffset, &MethodError)> + '_ {
        self.issues
            .iter()
            .map(|i| (i.pc, &i.error))
            .chain(
                self.invokes
                    .iter()
                    .filter_map(|i| match i.layout.as_ref().err() {
                        Some(InvokeIssue::Constraint(e)) => Some((i.pc, e)),
                        _ => None,
                    }),
            )
    }
    pub fn result_issue_at(&self, pc: CodeUnitOffset) -> Option<&MethodError> {
        self.issues
            .iter()
            .find(|i| i.result && i.pc == pc)
            .map(|i| &i.error)
    }

    /// Queries an original instruction boundary, not a semantic CFG leader.
    pub fn is_instruction_start(&self, pc: CodeUnitOffset) -> Result<bool, MethodError> {
        Ok(self.boundaries.start(pc)? == Start::Instruction)
    }
}

impl<'a> ProgramIndex<'a> {
    /// Establishes boundaries and control associations, retaining operand/result
    /// issues separately. Binding consumes P1's already-resolved reference facts,
    /// never re-reads prototypes or opens another analysis route.
    pub(super) fn check_method_layout(
        &self,
        original: &RawMethodView<'a>,
        items: &[super::DecodedItem],
        facts: &[super::reference::ReferenceFactEntry<'a>],
    ) -> Result<CheckedMethodLayout<'a>, MethodError> {
        if !std::ptr::eq(self.bytes, original.source_bytes)
            || self.header.header_offset != original.header_offset
        {
            return Err(MethodError::ForeignInput);
        }

        let mut boundaries = Boundaries::new(original.code_units())?;
        let mut instruction_count = 0usize;
        for item in items {
            match item {
                super::DecodedItem::Instruction(_) => {
                    instruction_count = instruction_count
                        .checked_add(1)
                        .ok_or(MethodError::PlatformCapacity)?;
                    boundaries.set_start(item.pc(), Start::Instruction)?;
                }
                super::DecodedItem::Payload(payload) => {
                    check_keys(original, item.pc(), payload.layout().kind())?;
                    boundaries.set_start(item.pc(), Start::Payload)?;
                }
            }
        }
        let mut checker = Checker {
            original,
            items,
            boundaries,
            invokes: Vec::new(),
            issues: Vec::new(),
        };
        checker.check_exceptions()?;
        checker.check_instructions(facts)?;
        checker.check_reachability(instruction_count)?;

        Ok(CheckedMethodLayout {
            original: original.clone(),
            boundaries: checker.boundaries,
            invokes: checker.invokes,
            issues: checker.issues,
        })
    }

    pub(super) fn reference_in_bounds(&self, reference: OperandReference) -> bool {
        match reference {
            OperandReference::String(id) => id.0 < self.header.string_ids.count,
            OperandReference::Type(id) => id.0 < self.header.type_ids.count,
            OperandReference::Field(id) => id.0 < self.header.field_ids.count,
            OperandReference::Method(id) => id.0 < self.header.method_ids.count,
            OperandReference::Proto(id) => id.0 < self.header.proto_ids.count,
            OperandReference::CallSite(id) => id.0 < self.mapped_count(7),
            OperandReference::MethodHandle(id) => id.0 < self.mapped_count(8),
        }
    }
}

impl RawMethodView<'_> {
    /// Visits original layouts in PC order using independent fallback work.
    /// A decoding failure retains the delivered prefix, but never guesses a
    /// width or claims semantic validation. A callback error stops visitation.
    pub fn visit_layouts(
        &self,
        mut visit: impl FnMut(CodeUnitOffset, InsnLayout) -> Result<(), MethodError>,
    ) -> Result<(), MethodError> {
        let mut pc = CodeUnitOffset(0);
        while pc.0 < self.code_units().0 {
            let layout = self.layout_at(pc)?;

            visit(pc, layout)?;
            pc = advance(pc, layout)?;
        }

        Ok(())
    }

    pub(super) fn layout_at(&self, pc: CodeUnitOffset) -> Result<InsnLayout, MethodError> {
        Ok(instruction_layout(self.instructions(), pc, self.version)?)
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

fn exact_buffer<T>(count: usize) -> Result<Vec<T>, MethodError> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(MethodError::PlatformCapacity)?;
    if bytes > isize::MAX as usize {
        return Err(MethodError::PlatformCapacity);
    }

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

#[derive(Debug, Clone)]
struct Boundaries {
    units: CodeUnitCount,
    // A nibble per code unit: start kind (2 bits), handler entry, reached.
    bits: Vec<u8>,
}

impl Boundaries {
    fn new(units: CodeUnitCount) -> Result<Self, MethodError> {
        let count = usize::try_from(units.0)
            .map_err(|_| MethodError::PlatformCapacity)?
            .div_ceil(2);
        let mut bits = exact_buffer(count)?;
        bits.resize(count, 0);
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
    items: &'m [super::DecodedItem],
    boundaries: Boundaries,
    invokes: Vec<InvokeFact>,
    issues: Vec<InstructionIssue>,
}

impl<'a> Checker<'_, 'a> {
    fn check_exceptions(&mut self) -> Result<(), MethodError> {
        let range = self.original.handler_byte_range();
        if range.start == range.end {
            return Ok(());
        }
        let mut handlers = Cursor::bounded(self.original.source_bytes, range.start, range.end)?;
        let count = handlers.read_uleb32()?;
        for _ in 0..count {
            visit_handler(&mut handlers, |pc| {
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
            let record = read_try(&mut cursor)?;
            if self.boundaries.start(record.start)? != Start::Instruction
                || (record.end.0 != self.original.code_units().0
                    && self.boundaries.start(record.end)? == Start::Absent)
            {
                return Err(bad(record.start, MethodProblem::InvalidTryBoundary));
            }
        }
        Ok(())
    }

    fn item_at(&self, pc: CodeUnitOffset) -> Result<&super::DecodedItem, MethodError> {
        self.items
            .binary_search_by(|item| item.pc().cmp(&pc))
            .ok()
            .map(|index| &self.items[index])
            .ok_or(MethodError::InternalInvariant(
                "decoded directory lost a checked item",
            ))
    }

    fn layout_at(&self, pc: CodeUnitOffset) -> Result<InsnLayout, MethodError> {
        match self.item_at(pc)? {
            super::DecodedItem::Instruction(instruction) => Ok(instruction.layout()),
            super::DecodedItem::Payload(payload) => Ok(payload.layout()),
        }
    }

    fn operands_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Result<crate::input::operands::RawOperands, MethodError> {
        match self.item_at(pc)? {
            super::DecodedItem::Instruction(instruction) => Ok(*instruction.operands()),
            super::DecodedItem::Payload(_) => Err(MethodError::InternalInvariant(
                "decoded directory has no operands for a payload",
            )),
        }
    }

    fn spec_at(&self, pc: CodeUnitOffset) -> Result<OpcodeSpec, MethodError> {
        match self.layout_at(pc)?.kind() {
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
        let operands = self.operands_at(pc)?;
        let delta = operands
            .offset()
            .ok_or(MethodError::InternalInvariant("branch format has no delta"))?
            .0;
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
        let target = relative(
            pc,
            self.operands_at(pc)?
                .offset()
                .ok_or(MethodError::InternalInvariant("switch lost payload delta"))?
                .0,
        )?;
        if self.boundaries.start(target)? != Start::Payload {
            return Err(bad(pc, MethodProblem::WrongPayloadKind));
        }
        let kind = self.layout_at(target)?.kind();
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
        &mut self,
        facts: &[super::reference::ReferenceFactEntry<'a>],
    ) -> Result<(), MethodError> {
        enum Producer {
            Known(ResultKind),
            Unavailable,
        }
        let mut pc = CodeUnitOffset(0);
        let mut previous = None;
        while pc.0 < self.original.code_units().0 {
            let layout = self.layout_at(pc)?;
            if let LayoutKind::Instruction(spec) = layout.kind() {
                let operands = self.operands_at(pc)?;
                if let Err(error) = check_registers(self.original, pc, spec, operands) {
                    crate::substrate::owned::push(
                        &mut self.issues,
                        InstructionIssue {
                            pc,
                            result: false,
                            error,
                        },
                    )
                    .map_err(super::disassembly::scratch)?;
                }
                if spec.result == ResultRole::MoveResult {
                    let issue = match &previous {
                        Some(Producer::Known(expected)) if !expected.accepts(spec.opcode) => {
                            Some(bad(pc, MethodProblem::InvalidResultKind))
                        }
                        None => Some(bad(pc, MethodProblem::InvalidResultEntry)),
                        _ => None,
                    };
                    if let Some(error) = issue {
                        crate::substrate::owned::push(
                            &mut self.issues,
                            InstructionIssue {
                                pc,
                                result: true,
                                error,
                            },
                        )
                        .map_err(super::disassembly::scratch)?;
                    }
                } else if spec.result == ResultRole::MoveException
                    && self.boundaries.flags(pc) & 4 == 0
                {
                    return Err(bad(pc, MethodProblem::InvalidExceptionEntry));
                }
                match spec.flow {
                    FlowKind::Goto | FlowKind::Conditional => {
                        self.branch_target(pc, spec)?;
                    }
                    FlowKind::PackedSwitch | FlowKind::SparseSwitch => {
                        let targets = self.switch_targets(pc, spec.flow)?;
                        for index in 0..targets.entries {
                            self.normal_target(pc, targets.at(index)?, true)?;
                        }
                    }
                    FlowKind::FillArray => {
                        self.payload_at(pc, spec.flow)?;
                    }
                    _ => {}
                }
                match check_producer(facts, pc, spec, operands) {
                    Ok(Some(producer)) => {
                        if let Some(layout) = producer.invoke {
                            crate::substrate::owned::push(
                                &mut self.invokes,
                                InvokeFact {
                                    pc,
                                    layout: layout.map_err(InvokeIssue::Constraint),
                                },
                            )
                            .map_err(super::disassembly::scratch)?;
                        }
                        previous = Some(Producer::Known(producer.result));
                    }
                    Ok(None) => previous = None,
                    Err(error) => {
                        crate::substrate::owned::push(
                            &mut self.invokes,
                            InvokeFact {
                                pc,
                                layout: Err(error),
                            },
                        )
                        .map_err(super::disassembly::scratch)?;
                        previous = Some(Producer::Unavailable);
                    }
                }
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

    fn check_reachability(&mut self, instruction_count: usize) -> Result<(), MethodError> {
        let entry = CodeUnitOffset(0);
        if self.boundaries.start(entry)? != Start::Instruction {
            return Err(bad(entry, MethodProblem::MissingEntryInstruction));
        }
        self.normal_target(entry, entry, true)?;
        let mut pending = exact_buffer(instruction_count)?;
        self.enqueue(entry, &mut pending)?;
        while let Some(pc) = pending.pop() {
            let layout = self.layout_at(pc)?;
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
                // monitor-exit selects a handler after its advanced PC.
                let selection = if spec.opcode == 0x1e {
                    advance(pc, layout)?
                } else {
                    pc
                };
                self.enqueue_handlers(selection, &mut pending)?;
            }
        }
        Ok(())
    }

    fn enqueue_handlers(
        &mut self,
        pc: CodeUnitOffset,
        pending: &mut Vec<CodeUnitOffset>,
    ) -> Result<(), MethodError> {
        let range = self.original.try_byte_range();
        let mut lower = 0;
        let mut upper = (range.end.0 - range.start.0) / 8;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let start = range
                .start
                .checked_advance(ByteLength(middle * 8))
                .ok_or(MethodError::InternalInvariant("try offset overflow"))?;
            let mut cursor = Cursor::bounded(self.original.source_bytes, start, range.end)?;
            let record = read_try(&mut cursor)?;
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
                return visit_handler(&mut cursor, |target| self.enqueue(target, pending));
            }
        }
        Ok(())
    }
}

fn check_registers(
    original: &RawMethodView<'_>,
    pc: CodeUnitOffset,
    spec: OpcodeSpec,
    operands: RawOperands,
) -> Result<(), MethodError> {
    let span = |start: u32, count: u8| -> Result<(), MethodError> {
        if count != 0 && u64::from(start) + u64::from(count) > u64::from(original.registers_size())
        {
            Err(bad(pc, MethodProblem::InvalidRegisterSpan))
        } else {
            Ok(())
        }
    };
    match spec.registers {
        RegisterShape::InvokeWords | RegisterShape::ArrayWords => {
            let count = match operands.registers().encoding {
                RegisterEncoding::List { count, .. } | RegisterEncoding::Range { count, .. } => {
                    count
                }
            };
            if spec.registers == RegisterShape::InvokeWords
                && u16::from(count) > original.outs_size()
            {
                return Err(bad(pc, MethodProblem::InsufficientOutgoingWords));
            }
            match operands.registers().encoding {
                RegisterEncoding::Range { first, count } => span(first.0, count)?,
                RegisterEncoding::List { .. } => {
                    for register in operands.registers().iter() {
                        span(register.0, 1)?;
                    }
                }
            }
        }
        RegisterShape::Fixed(widths) => {
            let mut registers = operands.registers().iter();
            for width in widths {
                match (registers.next(), width) {
                    (Some(register), Some(width)) => span(
                        register.0,
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
            if registers.next().is_some() {
                return Err(MethodError::InternalInvariant(
                    "fixed register format has extra operands",
                ));
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

fn read_try(cursor: &mut Cursor<'_>) -> Result<TryRecord, MethodError> {
    let start = CodeUnitOffset(u32::from_le_bytes(cursor.read_array()?));
    let count = u16::from_le_bytes(cursor.read_array()?);
    let handler_offset = u16::from_le_bytes(cursor.read_array()?);
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
    mut visit: impl FnMut(CodeUnitOffset) -> Result<(), MethodError>,
) -> Result<(), MethodError> {
    let size = cursor.read_sleb32()?;
    for _ in 0..i64::from(size).unsigned_abs() {
        cursor.read_uleb32()?;
        let target = CodeUnitOffset(cursor.read_uleb32()?);
        visit(target)?;
    }
    if size <= 0 {
        visit(CodeUnitOffset(cursor.read_uleb32()?))?;
    }
    Ok(())
}
