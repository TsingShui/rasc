//! Local symbolic execution correspondence, not a method interpreter. Each
//! relation starts in an arbitrary frame/heap, reads old exact slots, performs
//! one typed behavior and branches into success/throw before register commit.
//! No values, aliases or feasible paths are propagated between operations.

use super::scratch::push;
use super::{
    ArrayAccess, BinaryForm, BinaryOp, CommitRule, CompareOp, FieldAccess, FlowError, FlowMethod,
    InvokeKind, MoveKind, OpKind, Operation, UnaryOp, ValueKind,
};
use crate::p1::{DecodedInstruction, DecodedMethod};
use crate::raw::{OperandReference, RawLiteral, ReferenceSlot};
use crate::{CodeUnitOffset, RegisterId, RegisterSlot, SlotWidth};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Number {
    Int,
    Long,
    Float,
    Double,
    Byte,
    Char,
    Short,
}

impl Number {
    fn width(self) -> SlotWidth {
        match self {
            Self::Long | Self::Double => SlotWidth::Two,
            _ => SlotWidth::One,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Arithmetic {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    And,
    Or,
    Xor,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Compute {
    // Number fixes wrapping integral or IEEE arithmetic, including shift-count
    // masking; no reassociation or arithmetic simplification is performed.
    Binary(Number, Arithmetic),
    Negate(Number),
    Complement(Number),
    Convert(Number, Number),
    Compare(Number, i8), // floating NaN result (-1/+1), zero for cmp-long
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Element {
    Word,
    Wide,
    Reference,
    Boolean,
    Byte,
    Char,
    Short,
}

impl Element {
    fn width(self) -> SlotWidth {
        if self == Self::Wide {
            SlotWidth::Two
        } else {
            SlotWidth::One
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Behavior {
    None,
    Copy(MoveKind),
    Constant,
    Compute(Compute),
    Resolve(OperandReference),
    ArrayLoad(Element),
    ArrayStore(Element),
    FieldLoad(OperandReference, bool, Element),
    FieldStore(OperandReference, bool, Element),
    ArrayLength,
    Cast(OperandReference),
    InstanceOf(OperandReference),
    NewObject(OperandReference),
    NewArray(OperandReference),
    FilledArray(OperandReference),
    Fill(CodeUnitOffset),
    Call(
        InvokeKind,
        OperandReference,
        Option<OperandReference>,
        Option<SlotWidth>,
    ),
    MonitorEnter,
    MonitorExit,
    ReceiveException,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Symbol {
    // An exact slot is one symbolic input, never two independently combined
    // register words. Its value is captured before any overlapping write.
    Old(RegisterSlot),
    Bits32(u32),
    Bits64(u64),
}

#[derive(Debug, Eq, PartialEq)]
struct Event {
    site: CodeUnitOffset,
    behavior: Behavior,
    // Semantic argument order: e.g. array/index/value and literal/source for
    // reverse subtraction, distinct from physical register-read order.
    arguments: Vec<Symbol>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WordValue {
    Input(RegisterSlot, u8),
    Literal(u64, u8),
    Result(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WordWrite {
    register: RegisterId,
    value: WordValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameTransfer {
    Preserve,
    PatchAt(CodeUnitOffset),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeapTransfer {
    Original,
    AfterEvent(CodeUnitOffset),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LockTransfer {
    Preserve,
    Acquire(Symbol),
    Release(Symbol),
    ReleaseOutcome(Symbol),
}

#[derive(Debug, Eq, PartialEq)]
struct Completion {
    frame: FrameTransfer,
    writes: Vec<WordWrite>,
    heap: HeapTransfer,
    lock: LockTransfer,
}

#[derive(Debug, Eq, PartialEq)]
struct ThrowCompletion {
    selection: CodeUnitOffset,
    state: Completion,
}

#[derive(Debug, Eq, PartialEq)]
struct Relation {
    reads: Vec<RegisterSlot>,
    event: Event,
    normal: Completion,
    thrown: Option<ThrowCompletion>,
}

struct LocalStep {
    reads: Vec<RegisterSlot>,
    behavior: Behavior,
    arguments: Vec<Symbol>,
    destination: Option<RegisterSlot>,
    selection: Option<CodeUnitOffset>,
}

impl LocalStep {
    fn new() -> Self {
        Self {
            reads: Vec::new(),
            behavior: Behavior::None,
            arguments: Vec::new(),
            destination: None,
            selection: None,
        }
    }

    fn read(&mut self, register: RegisterId, width: SlotWidth) -> Result<Symbol, FlowError> {
        let slot = RegisterSlot::new(register, width);
        push(&mut self.reads, slot)?;
        Ok(Symbol::Old(slot))
    }

    fn arg(&mut self, symbol: Symbol) -> Result<(), FlowError> {
        push(&mut self.arguments, symbol)
    }

    fn destination(&mut self, register: RegisterId, width: SlotWidth) {
        self.destination = Some(RegisterSlot::new(register, width));
    }
}

// DEX completion obligations follow the bytecode spec and the pinned ART
// interpreter (see spec/02-flow.md). The lowered side independently interprets
// published fields; neither side manufactures missing products or propagates
// state into another instruction.
fn dex_relation(step: LocalStep, instruction: &DecodedInstruction) -> Result<Relation, FlowError> {
    let site = instruction.pc();
    let mut writes = Vec::new();
    if let Some(destination) = step.destination {
        for word in 0..destination.words() {
            let register = destination
                .start
                .0
                .checked_add(word)
                .map(RegisterId)
                .ok_or(FlowError::PlatformCapacity)?;
            let word = word as u8; // One/Two slots have at most two words.
            let value = match (step.behavior, step.arguments.first()) {
                (Behavior::Copy(_), Some(Symbol::Old(slot))) => WordValue::Input(*slot, word),
                (Behavior::Constant, Some(Symbol::Bits32(bits))) => {
                    WordValue::Literal(u64::from(*bits), word)
                }
                (Behavior::Constant, Some(Symbol::Bits64(bits))) => WordValue::Literal(*bits, word),
                _ => WordValue::Result(word),
            };
            push(&mut writes, WordWrite { register, value })?;
        }
    }
    let commit = if step.destination.is_some() {
        Some(instruction.result_pc().unwrap_or(site))
    } else {
        None
    };
    let opcode = instruction.opcode();
    let heap = if matches!(opcode, 0x1a | 0x1b | 0x1d | 0x1e | 0x22..=0x26 |
        0x4b..=0x51 | 0x59..=0x6d | 0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd)
    {
        HeapTransfer::AfterEvent(site)
    } else {
        HeapTransfer::Original
    };
    let lock_operand = if matches!(opcode, 0x1d | 0x1e) {
        Some(*step.arguments.first().ok_or(FlowError::Correspondence {
            pc: site,
            obligation: "original monitor has no lock operand",
        })?)
    } else {
        None
    };
    let lock = match (opcode, lock_operand) {
        (0x1d, Some(input)) => LockTransfer::Acquire(input),
        (0x1e, Some(input)) => LockTransfer::Release(input),
        _ => LockTransfer::Preserve,
    };
    let thrown = step.selection.map(|selection| ThrowCompletion {
        selection,
        state: Completion {
            frame: FrameTransfer::Preserve,
            writes: Vec::new(),
            heap,
            // monitor-exit selection is advanced; this does not establish that
            // release failed or succeeded. Retain an opaque outcome, not rollback.
            lock: match (opcode, lock_operand) {
                (0x1e, Some(input)) => LockTransfer::ReleaseOutcome(input),
                _ => LockTransfer::Preserve,
            },
        },
    });
    Ok(Relation {
        reads: step.reads,
        event: Event {
            site,
            behavior: step.behavior,
            arguments: step.arguments,
        },
        normal: Completion {
            frame: commit.map_or(FrameTransfer::Preserve, FrameTransfer::PatchAt),
            writes,
            heap,
            lock,
        },
        thrown,
    })
}

fn flow_relation(
    step: LocalStep,
    flow: &FlowMethod<'_>,
    operation: &Operation,
) -> Result<Relation, FlowError> {
    let site = operation.pc;
    let destination = flow.writes(operation).first().copied();
    let commit = match operation.normal.commit {
        CommitRule::None => None,
        CommitRule::AtOperation => Some(site),
        CommitRule::AfterResult => Some(operation.result_pc.ok_or(FlowError::Correspondence {
            pc: site,
            obligation: "normal commit has no result site",
        })?),
    };
    if destination.is_some() != commit.is_some() {
        return Err(FlowError::Correspondence {
            pc: site,
            obligation: "published destination/normal commit",
        });
    }
    // Construct the actual register patch from the product, independently of
    // the DEX expected patch. Captured inputs are old-slot symbols, not lookups
    // from a progressively modified frame, so overlapping move-wide is atomic.
    let mut writes = Vec::new();
    for destination in flow.writes(operation) {
        let source = match operation.kind {
            OpKind::Move { .. } => {
                Some(*step.arguments.first().ok_or(FlowError::Correspondence {
                    pc: site,
                    obligation: "copy has no captured old input",
                })?)
            }
            OpKind::Const {
                bits,
                width: SlotWidth::One,
            } => Some(Symbol::Bits32(u32::try_from(bits).map_err(|_| {
                FlowError::Correspondence {
                    pc: site,
                    obligation: "word constant overflow",
                }
            })?)),
            OpKind::Const {
                bits,
                width: SlotWidth::Two,
            } => Some(Symbol::Bits64(bits)),
            _ => None,
        };
        for word in 0..destination.words() {
            let value = match source {
                Some(Symbol::Old(slot)) => WordValue::Input(slot, word as u8),
                Some(Symbol::Bits32(bits)) => WordValue::Literal(u64::from(bits), word as u8),
                Some(Symbol::Bits64(bits)) => WordValue::Literal(bits, word as u8),
                None => WordValue::Result(word as u8),
            };
            let register = destination
                .start
                .0
                .checked_add(word)
                .map(RegisterId)
                .ok_or(FlowError::PlatformCapacity)?;
            push(&mut writes, WordWrite { register, value })?;
        }
    }
    if commit != operation.normal.commit_pc {
        return Err(FlowError::Correspondence {
            pc: site,
            obligation: "explicit normal commit position",
        });
    }
    let heap = match operation.normal.heap {
        super::HeapTransfer::Preserve => HeapTransfer::Original,
        super::HeapTransfer::EventOutcome => HeapTransfer::AfterEvent(site),
    };
    let input = || {
        step.arguments
            .first()
            .copied()
            .ok_or(FlowError::Correspondence {
                pc: site,
                obligation: "lock transfer has no operand",
            })
    };
    let lock = match operation.normal.lock {
        super::LockTransfer::Preserve => LockTransfer::Preserve,
        super::LockTransfer::Acquire => LockTransfer::Acquire(input()?),
        super::LockTransfer::Release => LockTransfer::Release(input()?),
        super::LockTransfer::ReleaseOutcome => LockTransfer::ReleaseOutcome(input()?),
    };
    let thrown = if let Some(completion) = operation.exceptional {
        let heap = match completion.heap {
            super::HeapTransfer::Preserve => HeapTransfer::Original,
            super::HeapTransfer::EventOutcome => HeapTransfer::AfterEvent(site),
        };
        let lock = match completion.lock {
            super::LockTransfer::Preserve => LockTransfer::Preserve,
            super::LockTransfer::Acquire => LockTransfer::Acquire(input()?),
            super::LockTransfer::Release => LockTransfer::Release(input()?),
            super::LockTransfer::ReleaseOutcome => LockTransfer::ReleaseOutcome(input()?),
        };
        Some(ThrowCompletion {
            selection: completion.selection_pc,
            state: Completion {
                frame: match completion.frame {
                    super::ExceptionFrame::PreserveBeforeOperation => FrameTransfer::Preserve,
                },
                writes: Vec::new(),
                heap,
                lock,
            },
        })
    } else {
        None
    };
    Ok(Relation {
        reads: step.reads,
        event: Event {
            site,
            behavior: step.behavior,
            arguments: step.arguments,
        },
        normal: Completion {
            frame: commit.map_or(FrameTransfer::Preserve, FrameTransfer::PatchAt),
            writes,
            heap,
            lock,
        },
        thrown,
    })
}

fn verify_unresolved(
    flow: &FlowMethod<'_>,
    instruction: &DecodedInstruction,
    op: &Operation,
) -> Result<(), FlowError> {
    let pc = instruction.pc();
    let bad = || FlowError::Correspondence {
        pc,
        obligation: "unavailable input/result facts and bounded normal write",
    };
    let (result, expected_heap) = match op.kind {
        OpKind::UnresolvedInvoke {
            kind,
            target,
            prototype,
            result: category,
        } => {
            let expected = match instruction.opcode() {
                0x6e | 0x74 => InvokeKind::Virtual,
                0x6f | 0x75 => InvokeKind::Super,
                0x70 | 0x76 => InvokeKind::Direct,
                0x71 | 0x77 => InvokeKind::Static,
                0x72 | 0x78 => InvokeKind::Interface,
                0xfa | 0xfb => InvokeKind::Polymorphic,
                0xfc | 0xfd => InvokeKind::Custom,
                _ => return Err(bad()),
            };
            let second = if expected == InvokeKind::Polymorphic {
                match reference(instruction, ReferenceSlot::Second)? {
                    OperandReference::Proto(p) => Some(p),
                    _ => return Err(bad()),
                }
            } else {
                None
            };
            if kind != expected
                || target != reference(instruction, ReferenceSlot::First)?
                || prototype != second
                || flow.reads(op).is_ok()
                || !matches!(flow.decoded().invoke_layout_at(pc), Some(Err(_)))
                || category
                    != flow
                        .decoded()
                        .invoke_result_at(pc)
                        .and_then(Result::ok)
                        .map(super::InvokeResult::from)
                        .unwrap_or(super::InvokeResult::Unavailable)
            {
                return Err(bad());
            }
            let result = instruction
                .result_pc()
                .map(|pc| flow.decoded().instruction_at(pc).ok_or_else(bad))
                .transpose()?;
            (result, super::HeapTransfer::EventOutcome)
        }
        OpKind::UnresolvedResult { width } => {
            if !matches!(instruction.opcode(), 0x0a..=0x0c)
                || flow.decoded().result_issue_at(pc).is_none()
                || width
                    != if instruction.opcode() == 0x0b {
                        SlotWidth::Two
                    } else {
                        SlotWidth::One
                    }
                || !flow.reads(op).map_err(|_| bad())?.is_empty()
            {
                return Err(bad());
            }
            (Some(instruction), super::HeapTransfer::Preserve)
        }
        _ => return Err(bad()),
    };
    let expected_commit = if result.is_none() {
        CommitRule::None
    } else if matches!(op.kind, OpKind::UnresolvedResult { .. }) {
        CommitRule::AtOperation
    } else {
        CommitRule::AfterResult
    };
    if op.normal.commit() != expected_commit
        || op.normal.commit_pc() != result.map(|i| i.pc())
        || op.normal.heap() != expected_heap
        || op.normal.lock() != super::LockTransfer::Preserve
    {
        return Err(bad());
    }
    match result {
        None if flow.writes(op).is_empty() => {}
        Some(result) => {
            let width = if result.opcode() == 0x0b {
                SlotWidth::Two
            } else {
                SlotWidth::One
            };
            let reg = result.registers().next().ok_or_else(bad)?;
            if flow.writes(op) != [RegisterSlot::new(reg, width)] {
                return Err(bad());
            }
        }
        _ => return Err(bad()),
    }
    Ok(())
}

pub(super) fn verify(
    flow: &FlowMethod<'_>,
    instruction: &DecodedInstruction,
    operation: &Operation,
) -> Result<(), FlowError> {
    if matches!(
        operation.kind,
        OpKind::UnresolvedInvoke { .. } | OpKind::UnresolvedResult { .. }
    ) {
        return verify_unresolved(flow, instruction, operation);
    }
    if let OpKind::Binary { form, .. } = operation.kind {
        let expected = match instruction.opcode() {
            0x90..=0xaf => BinaryForm::ThreeAddress,
            0xb0..=0xcf => BinaryForm::TwoAddress,
            0xd0..=0xe2 => BinaryForm::Literal,
            _ => {
                return Err(FlowError::Correspondence {
                    pc: instruction.pc(),
                    obligation: "binary form has a non-arithmetic source",
                });
            }
        };
        if form != expected {
            return Err(FlowError::Correspondence {
                pc: instruction.pc(),
                obligation: "binary encoding-form annotation",
            });
        }
    }
    let original = dex_relation(dex_step(flow.decoded(), instruction)?, instruction)?;
    let lowered = flow_relation(flow_step(flow, operation)?, flow, operation)?;
    let Completion {
        frame: original_frame,
        writes: original_writes,
        heap: original_heap,
        lock: original_lock,
    } = original.normal;
    let Completion {
        frame: lowered_frame,
        writes: lowered_writes,
        heap: lowered_heap,
        lock: lowered_lock,
    } = lowered.normal;
    let writes_match = if matches!(instruction.opcode(), 0x04..=0x06) {
        wide_patch_matches(instruction, &lowered_writes)?
    } else {
        original_writes == lowered_writes
    };
    if !writes_match
        || (
            original.reads,
            original.event,
            original_frame,
            original_heap,
            original_lock,
            original.thrown,
        ) != (
            lowered.reads,
            lowered.event,
            lowered_frame,
            lowered_heap,
            lowered_lock,
            lowered.thrown,
        )
    {
        return Err(FlowError::Correspondence {
            pc: instruction.pc(),
            obligation: "symbolic event/normal patch/throw state",
        });
    }
    Ok(())
}

fn wide_patch_matches(
    instruction: &DecodedInstruction,
    writes: &[WordWrite],
) -> Result<bool, FlowError> {
    let [first, second] = writes else {
        return Ok(false);
    };
    let captured = |write: &WordWrite| match write.value {
        WordValue::Input(source, part) => Some(super::wide_copy::CopyWord {
            register: write.register.0,
            source: source.start.0,
            source_words: source.words(),
            part,
        }),
        _ => None,
    };
    let (Some(first), Some(second)) = (captured(first), captured(second)) else {
        return Ok(false);
    };
    Ok(super::wide_copy::wide_copy_matches(
        register(instruction, 1)?.0,
        register(instruction, 0)?.0,
        first,
        second,
    ))
}

fn register(instruction: &DecodedInstruction, index: usize) -> Result<RegisterId, FlowError> {
    instruction
        .registers()
        .nth(index)
        .ok_or(FlowError::InternalInvariant("replay lost checked register"))
}

fn reference(
    instruction: &DecodedInstruction,
    slot: ReferenceSlot,
) -> Result<OperandReference, FlowError> {
    instruction
        .references()
        .find(|reference| reference.slot == slot)
        .map(|reference| reference.target)
        .ok_or(FlowError::InternalInvariant(
            "replay lost checked reference",
        ))
}

fn literal(instruction: &DecodedInstruction) -> Result<Symbol, FlowError> {
    match instruction.operands().literal() {
        Some(RawLiteral::Bits32(bits)) => Ok(Symbol::Bits32(bits)),
        Some(RawLiteral::Bits64(bits)) => Ok(Symbol::Bits64(bits)),
        Some(RawLiteral::Signed8(value)) => Ok(Symbol::Bits32(value as i32 as u32)),
        Some(RawLiteral::Signed16(value)) => Ok(Symbol::Bits32(value as i32 as u32)),
        None => Err(FlowError::InternalInvariant("replay lost original literal")),
    }
}

fn dex_step(
    decoded: &DecodedMethod<'_>,
    instruction: &DecodedInstruction,
) -> Result<LocalStep, FlowError> {
    let opcode = instruction.opcode();
    let mut step = LocalStep::new();
    match opcode {
        0x00 => {}
        0x01..=0x09 => {
            let kind = match opcode {
                0x01..=0x03 => MoveKind::Word,
                0x04..=0x06 => MoveKind::Wide,
                _ => MoveKind::Object,
            };
            step.behavior = Behavior::Copy(kind);
            let width = if kind == MoveKind::Wide {
                SlotWidth::Two
            } else {
                SlotWidth::One
            };
            let input = step.read(register(instruction, 1)?, width)?;
            step.arg(input)?;
            step.destination(register(instruction, 0)?, width);
        }
        0x0d => {
            step.behavior = Behavior::ReceiveException;
            step.destination(register(instruction, 0)?, SlotWidth::One);
        }
        0x12..=0x19 => {
            step.behavior = Behavior::Constant;
            step.arg(literal(instruction)?)?;
            step.destination(
                register(instruction, 0)?,
                if opcode >= 0x16 {
                    SlotWidth::Two
                } else {
                    SlotWidth::One
                },
            );
        }
        0x1a..=0x1c | 0xfe | 0xff => {
            step.behavior = Behavior::Resolve(reference(instruction, ReferenceSlot::First)?);
            step.destination(register(instruction, 0)?, SlotWidth::One);
        }
        0x1d | 0x1e | 0x1f | 0x26 => {
            step.behavior = match opcode {
                0x1d => Behavior::MonitorEnter,
                0x1e => Behavior::MonitorExit,
                0x1f => Behavior::Cast(reference(instruction, ReferenceSlot::First)?),
                _ => Behavior::Fill(
                    instruction
                        .payload_target()
                        .ok_or(FlowError::InternalInvariant("fill lost payload"))?,
                ),
            };
            let input = step.read(register(instruction, 0)?, SlotWidth::One)?;
            step.arg(input)?;
        }
        0x20 | 0x21 | 0x23 => {
            step.behavior = match opcode {
                0x20 => Behavior::InstanceOf(reference(instruction, ReferenceSlot::First)?),
                0x21 => Behavior::ArrayLength,
                _ => Behavior::NewArray(reference(instruction, ReferenceSlot::First)?),
            };
            let input = step.read(register(instruction, 1)?, SlotWidth::One)?;
            step.arg(input)?;
            step.destination(register(instruction, 0)?, SlotWidth::One);
        }
        0x22 => {
            step.behavior = Behavior::NewObject(reference(instruction, ReferenceSlot::First)?);
            step.destination(register(instruction, 0)?, SlotWidth::One);
        }
        0x24 | 0x25 => {
            step.behavior = Behavior::FilledArray(reference(instruction, ReferenceSlot::First)?);
            for register in instruction.registers() {
                let input = step.read(register, SlotWidth::One)?;
                step.arg(input)?;
            }
        }
        0x2d..=0x31 => {
            let (number, nan) = match opcode {
                0x2d => (Number::Float, -1),
                0x2e => (Number::Float, 1),
                0x2f => (Number::Double, -1),
                0x30 => (Number::Double, 1),
                _ => (Number::Long, 0),
            };
            step.behavior = Behavior::Compute(Compute::Compare(number, nan));
            for index in 1..=2 {
                let input = step.read(register(instruction, index)?, number.width())?;
                step.arg(input)?;
            }
            step.destination(register(instruction, 0)?, SlotWidth::One);
        }
        0x44..=0x51 => {
            let get = opcode <= 0x4a;
            let element = dex_element(opcode);
            step.behavior = if get {
                Behavior::ArrayLoad(element)
            } else {
                Behavior::ArrayStore(element)
            };
            let value = if get {
                None
            } else {
                Some(step.read(register(instruction, 0)?, element.width())?)
            };
            let array = step.read(register(instruction, 1)?, SlotWidth::One)?;
            let index = step.read(register(instruction, 2)?, SlotWidth::One)?;
            step.arg(array)?;
            step.arg(index)?;
            if let Some(value) = value {
                step.arg(value)?;
            }
            if get {
                step.destination(register(instruction, 0)?, element.width());
            }
        }
        0x52..=0x6d => {
            let static_field = opcode >= 0x60;
            let get = matches!(opcode, 0x52..=0x58 | 0x60..=0x66);
            let element = dex_element(opcode);
            let field = reference(instruction, ReferenceSlot::First)?;
            step.behavior = if get {
                Behavior::FieldLoad(field, static_field, element)
            } else {
                Behavior::FieldStore(field, static_field, element)
            };
            let value = if get {
                None
            } else {
                Some(step.read(register(instruction, 0)?, element.width())?)
            };
            if !static_field {
                let receiver = step.read(register(instruction, 1)?, SlotWidth::One)?;
                step.arg(receiver)?;
            }
            if let Some(value) = value {
                step.arg(value)?;
            }
            if get {
                step.destination(register(instruction, 0)?, element.width());
            }
        }
        0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd => {
            let kind = match opcode {
                0x6e | 0x74 => InvokeKind::Virtual,
                0x6f | 0x75 => InvokeKind::Super,
                0x70 | 0x76 => InvokeKind::Direct,
                0x71 | 0x77 => InvokeKind::Static,
                0x72 | 0x78 => InvokeKind::Interface,
                0xfa | 0xfb => InvokeKind::Polymorphic,
                _ => InvokeKind::Custom,
            };
            let second = if kind == InvokeKind::Polymorphic {
                Some(reference(instruction, ReferenceSlot::Second)?)
            } else {
                None
            };
            let layout = decoded
                .invoke_layout_at(instruction.pc())
                .ok_or(FlowError::InternalInvariant("call lost layout"))?
                .map_err(|_| {
                    FlowError::InternalInvariant("known invoke has unavailable input layout")
                })?;
            step.behavior = Behavior::Call(
                kind,
                reference(instruction, ReferenceSlot::First)?,
                second,
                layout.result(),
            );
            let mut registers = instruction.registers();
            if layout.has_receiver() {
                let first = registers
                    .next()
                    .ok_or(FlowError::InternalInvariant("call lost receiver"))?;
                let input = step.read(first, SlotWidth::One)?;
                step.arg(input)?;
            }
            for width in layout.arguments() {
                let first = registers
                    .next()
                    .ok_or(FlowError::InternalInvariant("call lost argument"))?;
                let input = step.read(first, *width)?;
                step.arg(input)?;
                if *width == SlotWidth::Two {
                    registers
                        .next()
                        .ok_or(FlowError::InternalInvariant("call lost wide word"))?;
                }
            }
        }
        0x7b..=0x8f => {
            let compute = dex_unary(opcode)?;
            let (source, target) = unary_domains(compute)?;
            step.behavior = Behavior::Compute(compute);
            let input = step.read(register(instruction, 1)?, source.width())?;
            step.arg(input)?;
            step.destination(register(instruction, 0)?, target.width());
        }
        0x90..=0xcf => {
            let canonical = if opcode >= 0xb0 {
                opcode - 0x20
            } else {
                opcode
            };
            let (number, arithmetic) = dex_binary(canonical)?;
            let first_index = if opcode >= 0xb0 { 0 } else { 1 };
            let right_width = if matches!(canonical, 0xa3..=0xa5) {
                SlotWidth::One
            } else {
                number.width()
            };
            step.behavior = Behavior::Compute(Compute::Binary(number, arithmetic));
            let left = step.read(register(instruction, first_index)?, number.width())?;
            let right = step.read(register(instruction, first_index + 1)?, right_width)?;
            step.arg(left)?;
            step.arg(right)?;
            step.destination(register(instruction, 0)?, number.width());
        }
        0xd0..=0xe2 => {
            let index = if opcode < 0xd8 {
                opcode - 0xd0
            } else {
                opcode - 0xd8
            };
            let arithmetic = match index {
                0 => Arithmetic::Add,
                1 => Arithmetic::Subtract,
                2 => Arithmetic::Multiply,
                3 => Arithmetic::Divide,
                4 => Arithmetic::Remainder,
                5 => Arithmetic::And,
                6 => Arithmetic::Or,
                7 => Arithmetic::Xor,
                8 => Arithmetic::ShiftLeft,
                9 => Arithmetic::ShiftRight,
                _ => Arithmetic::UnsignedShiftRight,
            };
            step.behavior = Behavior::Compute(Compute::Binary(Number::Int, arithmetic));
            let input = step.read(register(instruction, 1)?, SlotWidth::One)?;
            let bits = literal(instruction)?;
            if index == 1 {
                step.arg(bits)?;
                step.arg(input)?;
            } else {
                step.arg(input)?;
                step.arg(bits)?;
            }
            step.destination(register(instruction, 0)?, SlotWidth::One);
        }
        _ => {
            return Err(FlowError::InternalInvariant(
                "local DEX replay has no operation rule",
            ));
        }
    }
    if let Some(result_pc) = instruction.result_pc() {
        let result = decoded
            .instruction_at(result_pc)
            .ok_or(FlowError::InternalInvariant("replay result missing"))?;
        step.destination(
            register(result, 0)?,
            if result.opcode() == 0x0b {
                SlotWidth::Two
            } else {
                SlotWidth::One
            },
        );
    }
    // Independent fixed DEX throw sites, not the lowered effect flag.
    if matches!(opcode, 0x1a..=0x26 | 0x44..=0x72 | 0x74..=0x78 |
        0x93 | 0x94 | 0x9e | 0x9f | 0xb3 | 0xb4 | 0xbe | 0xbf |
        0xd3 | 0xd4 | 0xdb | 0xdc | 0xfa..=0xff)
    {
        step.selection = Some(if opcode == 0x1e {
            super::check::next(instruction)?
        } else {
            instruction.pc()
        });
    }
    Ok(step)
}

fn flow_step(flow: &FlowMethod<'_>, operation: &Operation) -> Result<LocalStep, FlowError> {
    let mut step = LocalStep::new();
    let mut inputs = Vec::new();
    for slot in flow.reads(operation).map_err(|_| {
        FlowError::InternalInvariant("symbolic replay needs established input slots")
    })? {
        let input = step.read(slot.start, slot.width)?;
        push(&mut inputs, input)?;
    }
    let input = |index: usize| -> Result<Symbol, FlowError> {
        inputs
            .get(index)
            .copied()
            .ok_or(FlowError::InternalInvariant("event lost input"))
    };
    step.behavior = match operation.kind {
        OpKind::Nop => Behavior::None,
        OpKind::Move { kind } => Behavior::Copy(kind),
        OpKind::Const { .. } => Behavior::Constant,
        OpKind::Unary { op } => Behavior::Compute(flow_unary(op)),
        OpKind::Binary { op, .. } => {
            let (number, arithmetic) = flow_binary(op);
            Behavior::Compute(Compute::Binary(number, arithmetic))
        }
        OpKind::Compare { op } => Behavior::Compute(match op {
            CompareOp::CmplFloat => Compute::Compare(Number::Float, -1),
            CompareOp::CmpgFloat => Compute::Compare(Number::Float, 1),
            CompareOp::CmplDouble => Compute::Compare(Number::Double, -1),
            CompareOp::CmpgDouble => Compute::Compare(Number::Double, 1),
            CompareOp::CmpLong => Compute::Compare(Number::Long, 0),
        }),
        OpKind::ConstString { value } => Behavior::Resolve(OperandReference::String(value)),
        OpKind::ConstClass { type_id } => Behavior::Resolve(OperandReference::Type(type_id)),
        OpKind::ConstMethodHandle { handle } => {
            Behavior::Resolve(OperandReference::MethodHandle(handle))
        }
        OpKind::ConstMethodType { proto } => Behavior::Resolve(OperandReference::Proto(proto)),
        OpKind::Array { access, kind } => {
            if access == ArrayAccess::Get {
                Behavior::ArrayLoad(flow_element(kind))
            } else {
                Behavior::ArrayStore(flow_element(kind))
            }
        }
        OpKind::Field {
            access,
            static_field,
            field,
            kind,
        } => {
            if access == FieldAccess::Get {
                Behavior::FieldLoad(
                    OperandReference::Field(field),
                    static_field,
                    flow_element(kind),
                )
            } else {
                Behavior::FieldStore(
                    OperandReference::Field(field),
                    static_field,
                    flow_element(kind),
                )
            }
        }
        OpKind::ArrayLength => Behavior::ArrayLength,
        OpKind::Cast { type_id } => Behavior::Cast(OperandReference::Type(type_id)),
        OpKind::InstanceOf { type_id } => Behavior::InstanceOf(OperandReference::Type(type_id)),
        OpKind::NewInstance { type_id } => Behavior::NewObject(OperandReference::Type(type_id)),
        OpKind::NewArray { type_id } => Behavior::NewArray(OperandReference::Type(type_id)),
        OpKind::FilledNewArray { type_id } => {
            Behavior::FilledArray(OperandReference::Type(type_id))
        }
        OpKind::FillArrayData { payload } => Behavior::Fill(payload),
        OpKind::Invoke {
            kind,
            target,
            prototype,
            result,
        } => Behavior::Call(kind, target, prototype.map(OperandReference::Proto), result),
        OpKind::Monitor { enter } => {
            if enter {
                Behavior::MonitorEnter
            } else {
                Behavior::MonitorExit
            }
        }
        OpKind::MoveException => Behavior::ReceiveException,
        OpKind::UnresolvedInvoke { .. } | OpKind::UnresolvedResult { .. } => {
            return Err(FlowError::InternalInvariant(
                "unavailable operation entered full symbolic replay",
            ));
        }
    };
    match operation.kind {
        OpKind::Const { bits, width } => {
            step.arg(if width == SlotWidth::Two {
                Symbol::Bits64(bits)
            } else {
                Symbol::Bits32(u32::try_from(bits).map_err(|_| FlowError::Correspondence {
                    pc: operation.pc,
                    obligation: "word literal exceeds 32 bits",
                })?)
            })?;
        }
        OpKind::Binary {
            op,
            literal: Some(value),
            ..
        } => {
            let bits =
                Symbol::Bits32(i32::try_from(value).map_err(|_| FlowError::Correspondence {
                    pc: operation.pc,
                    obligation: "arithmetic immediate exceeds int",
                })? as u32);
            if op == BinaryOp::RsubInt {
                step.arg(bits)?;
                step.arg(input(0)?)?;
            } else {
                step.arg(input(0)?)?;
                step.arg(bits)?;
            }
        }
        OpKind::Array {
            access: ArrayAccess::Put,
            ..
        } => {
            step.arg(input(1)?)?;
            step.arg(input(2)?)?;
            step.arg(input(0)?)?;
        }
        OpKind::Field {
            access: FieldAccess::Put,
            static_field,
            ..
        } => {
            if !static_field {
                step.arg(input(1)?)?;
            }
            step.arg(input(0)?)?;
        }
        _ => {
            for input in inputs {
                step.arg(input)?;
            }
        }
    }
    if flow.writes(operation).len() > 1 {
        return Err(FlowError::Correspondence {
            pc: operation.pc,
            obligation: "local event has multiple register results",
        });
    }
    Ok(step)
}

fn dex_element(opcode: u8) -> Element {
    match opcode {
        0x45 | 0x4c | 0x53 | 0x5a | 0x61 | 0x68 => Element::Wide,
        0x46 | 0x4d | 0x54 | 0x5b | 0x62 | 0x69 => Element::Reference,
        0x47 | 0x4e | 0x55 | 0x5c | 0x63 | 0x6a => Element::Boolean,
        0x48 | 0x4f | 0x56 | 0x5d | 0x64 | 0x6b => Element::Byte,
        0x49 | 0x50 | 0x57 | 0x5e | 0x65 | 0x6c => Element::Char,
        0x4a | 0x51 | 0x58 | 0x5f | 0x66 | 0x6d => Element::Short,
        _ => Element::Word,
    }
}

fn flow_element(kind: ValueKind) -> Element {
    match kind {
        ValueKind::Word => Element::Word,
        ValueKind::Wide => Element::Wide,
        ValueKind::Object => Element::Reference,
        ValueKind::Boolean => Element::Boolean,
        ValueKind::Byte => Element::Byte,
        ValueKind::Char => Element::Char,
        ValueKind::Short => Element::Short,
    }
}

fn dex_unary(opcode: u8) -> Result<Compute, FlowError> {
    use Number::*;
    Ok(match opcode {
        0x7b => Compute::Negate(Int),
        0x7c => Compute::Complement(Int),
        0x7d => Compute::Negate(Long),
        0x7e => Compute::Complement(Long),
        0x7f => Compute::Negate(Float),
        0x80 => Compute::Negate(Double),
        0x81 => Compute::Convert(Int, Long),
        0x82 => Compute::Convert(Int, Float),
        0x83 => Compute::Convert(Int, Double),
        0x84 => Compute::Convert(Long, Int),
        0x85 => Compute::Convert(Long, Float),
        0x86 => Compute::Convert(Long, Double),
        0x87 => Compute::Convert(Float, Int),
        0x88 => Compute::Convert(Float, Long),
        0x89 => Compute::Convert(Float, Double),
        0x8a => Compute::Convert(Double, Int),
        0x8b => Compute::Convert(Double, Long),
        0x8c => Compute::Convert(Double, Float),
        0x8d => Compute::Convert(Int, Byte),
        0x8e => Compute::Convert(Int, Char),
        0x8f => Compute::Convert(Int, Short),
        _ => return Err(FlowError::InternalInvariant("unary replay has no DEX rule")),
    })
}

fn flow_unary(op: UnaryOp) -> Compute {
    use Number::*;
    match op {
        UnaryOp::NegInt => Compute::Negate(Int),
        UnaryOp::NotInt => Compute::Complement(Int),
        UnaryOp::NegLong => Compute::Negate(Long),
        UnaryOp::NotLong => Compute::Complement(Long),
        UnaryOp::NegFloat => Compute::Negate(Float),
        UnaryOp::NegDouble => Compute::Negate(Double),
        UnaryOp::IntToLong => Compute::Convert(Int, Long),
        UnaryOp::IntToFloat => Compute::Convert(Int, Float),
        UnaryOp::IntToDouble => Compute::Convert(Int, Double),
        UnaryOp::LongToInt => Compute::Convert(Long, Int),
        UnaryOp::LongToFloat => Compute::Convert(Long, Float),
        UnaryOp::LongToDouble => Compute::Convert(Long, Double),
        UnaryOp::FloatToInt => Compute::Convert(Float, Int),
        UnaryOp::FloatToLong => Compute::Convert(Float, Long),
        UnaryOp::FloatToDouble => Compute::Convert(Float, Double),
        UnaryOp::DoubleToInt => Compute::Convert(Double, Int),
        UnaryOp::DoubleToLong => Compute::Convert(Double, Long),
        UnaryOp::DoubleToFloat => Compute::Convert(Double, Float),
        UnaryOp::IntToByte => Compute::Convert(Int, Byte),
        UnaryOp::IntToChar => Compute::Convert(Int, Char),
        UnaryOp::IntToShort => Compute::Convert(Int, Short),
    }
}

fn unary_domains(compute: Compute) -> Result<(Number, Number), FlowError> {
    match compute {
        Compute::Negate(number) | Compute::Complement(number) => Ok((number, number)),
        Compute::Convert(from, to) => Ok((from, to)),
        _ => Err(FlowError::InternalInvariant(
            "replay unary behavior has binary domain",
        )),
    }
}

fn dex_binary(opcode: u8) -> Result<(Number, Arithmetic), FlowError> {
    let (number, index) = match opcode {
        0x90..=0x9a => (Number::Int, opcode - 0x90),
        0x9b..=0xa5 => (Number::Long, opcode - 0x9b),
        0xa6..=0xaa => (Number::Float, opcode - 0xa6),
        0xab..=0xaf => (Number::Double, opcode - 0xab),
        _ => return Err(FlowError::InternalInvariant("replay binary opcode missing")),
    };
    let arithmetic = match index {
        0 => Arithmetic::Add,
        1 => Arithmetic::Subtract,
        2 => Arithmetic::Multiply,
        3 => Arithmetic::Divide,
        4 => Arithmetic::Remainder,
        5 => Arithmetic::And,
        6 => Arithmetic::Or,
        7 => Arithmetic::Xor,
        8 => Arithmetic::ShiftLeft,
        9 => Arithmetic::ShiftRight,
        _ => Arithmetic::UnsignedShiftRight,
    };
    Ok((number, arithmetic))
}

fn flow_binary(op: BinaryOp) -> (Number, Arithmetic) {
    use Arithmetic::*;
    use Number::*;
    match op {
        BinaryOp::AddInt => (Int, Add),
        BinaryOp::SubInt | BinaryOp::RsubInt => (Int, Subtract),
        BinaryOp::MulInt => (Int, Multiply),
        BinaryOp::DivInt => (Int, Divide),
        BinaryOp::RemInt => (Int, Remainder),
        BinaryOp::AndInt => (Int, And),
        BinaryOp::OrInt => (Int, Or),
        BinaryOp::XorInt => (Int, Xor),
        BinaryOp::ShlInt => (Int, ShiftLeft),
        BinaryOp::ShrInt => (Int, ShiftRight),
        BinaryOp::UshrInt => (Int, UnsignedShiftRight),
        BinaryOp::AddLong => (Long, Add),
        BinaryOp::SubLong => (Long, Subtract),
        BinaryOp::MulLong => (Long, Multiply),
        BinaryOp::DivLong => (Long, Divide),
        BinaryOp::RemLong => (Long, Remainder),
        BinaryOp::AndLong => (Long, And),
        BinaryOp::OrLong => (Long, Or),
        BinaryOp::XorLong => (Long, Xor),
        BinaryOp::ShlLong => (Long, ShiftLeft),
        BinaryOp::ShrLong => (Long, ShiftRight),
        BinaryOp::UshrLong => (Long, UnsignedShiftRight),
        BinaryOp::AddFloat => (Float, Add),
        BinaryOp::SubFloat => (Float, Subtract),
        BinaryOp::MulFloat => (Float, Multiply),
        BinaryOp::DivFloat => (Float, Divide),
        BinaryOp::RemFloat => (Float, Remainder),
        BinaryOp::AddDouble => (Double, Add),
        BinaryOp::SubDouble => (Double, Subtract),
        BinaryOp::MulDouble => (Double, Multiply),
        BinaryOp::DivDouble => (Double, Divide),
        BinaryOp::RemDouble => (Double, Remainder),
    }
}
