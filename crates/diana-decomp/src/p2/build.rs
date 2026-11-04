use std::collections::TryReserveError;

use thiserror::Error;

use super::transfer::*;
use super::{
    ArrayAccess, BinaryForm, BinaryOp, Block, BlockId, BranchOp, CommitRule, CompareOp, Coverage,
    Edge, EdgeId, EdgeRole, EdgeTarget, Effects, Exit, ExitId, ExitKind, FieldAccess, FlowMethod,
    InvokeKind, InvokeResult, MoveKind, NormalEdge, OpId, OpKind, Operation, SlotSpan, SwitchKind,
    Terminal, UnaryOp, ValueKind,
};
use crate::input::formats::FlowKind;
use crate::p1::{DecodedInstruction, DecodedItem, DecodedMethod};
use crate::raw::OperandReference;
use crate::raw::RawLiteral;
use crate::substrate::owned::push;
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{CodeUnitOffset, FailureKind, RegisterId, RequestError, TypeId};

#[derive(Debug, Error)]
pub enum FlowError {
    #[error("flow allocation failed: {0}")]
    Allocation(#[source] TryReserveError),
    #[error("flow capacity is not representable on this platform")]
    PlatformCapacity,
    #[error("unsupported flow at {pc:?} opcode 0x{opcode:02x}: {reason}")]
    Unsupported {
        pc: CodeUnitOffset,
        opcode: u8,
        reason: &'static str,
    },
    #[cfg(test)]
    #[error("P2 correspondence failed at {pc:?}: {obligation}")]
    Correspondence {
        pc: CodeUnitOffset,
        obligation: &'static str,
    },
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

impl FlowError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Allocation(_) | Self::PlatformCapacity => FailureKind::ResourceExhausted,
            Self::Unsupported { .. } => FailureKind::Unsupported,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
            #[cfg(test)]
            Self::Correspondence { .. } => FailureKind::InternalInvariant,
        }
    }
}

pub(crate) fn request_error(error: RequestError) -> FlowError {
    match error {
        RequestError::Allocation(error) => FlowError::Allocation(error),
        RequestError::PlatformCapacity => FlowError::PlatformCapacity,
        RequestError::InternalInvariant(message) => FlowError::InternalInvariant(message),
        _ => FlowError::InternalInvariant("P2 received a foreign P1 error"),
    }
}

fn internal(message: &'static str) -> FlowError {
    FlowError::InternalInvariant(message)
}

#[derive(Clone, Copy)]
struct Atom {
    producer: usize,
    result: Option<usize>,
}

struct Builder<'a, 'd> {
    decoded: &'d DecodedMethod<'a>,
    instructions: Vec<&'d DecodedInstruction>,
    atoms: Vec<Atom>,
    slots: Vec<RegisterSlot>,
    operations: Vec<Operation>,
    blocks: Vec<Block>,
    edges: Vec<Edge>,
    exits: Vec<Exit>,
    coverage: Vec<Coverage>,
}

struct FlowParts {
    slots: Vec<RegisterSlot>,
    operations: Vec<Operation>,
    blocks: Vec<Block>,
    edges: Vec<Edge>,
    exits: Vec<Exit>,
    coverage: Vec<Coverage>,
}

pub(crate) enum FlowBuild<'a> {
    Ready(FlowMethod<'a>),
    Stopped(super::FlowStop<'a>),
}

pub(crate) fn build_flow<'a>(
    _task: &crate::engine::MethodTask,
    decoded: DecodedMethod<'a>,
) -> FlowBuild<'a> {
    let mut builder = match Builder::new(&decoded) {
        Ok(builder) => builder,
        Err(error) => {
            return FlowBuild::Stopped(super::FlowStop {
                input: decoded,
                error,
                partial: None,
            });
        }
    };
    let result = builder.build();
    let parts = builder.finish();
    if let Err(error) = result {
        return FlowBuild::Stopped(super::FlowStop {
            input: decoded,
            error,
            partial: Some(super::FlowPartial {
                slots: parts.slots,
                operations: parts.operations,
                blocks: parts.blocks,
                edges: parts.edges,
                exits: parts.exits,
                coverage: parts.coverage,
            }),
        });
    }
    let flow = FlowMethod {
        decoded,
        slots: parts.slots,
        operations: parts.operations,
        blocks: parts.blocks,
        edges: parts.edges,
        exits: parts.exits,
        coverage: parts.coverage,
    };
    #[cfg(test)]
    if let Err(error) = super::check::verify(&flow) {
        return FlowBuild::Stopped(super::FlowStop {
            input: flow.decoded,
            error,
            partial: Some(super::FlowPartial {
                slots: flow.slots,
                operations: flow.operations,
                blocks: flow.blocks,
                edges: flow.edges,
                exits: flow.exits,
                coverage: flow.coverage,
            }),
        });
    }
    FlowBuild::Ready(flow)
}

impl<'a, 'd> Builder<'a, 'd> {
    fn new(decoded: &'d DecodedMethod<'a>) -> Result<Self, FlowError> {
        let mut instructions = Vec::new();
        let mut coverage = Vec::new();
        for item in decoded.items() {
            match item {
                DecodedItem::Instruction(instruction) => {
                    push(&mut instructions, instruction).map_err(request_error)?;
                    push(&mut coverage, Coverage::Unreachable).map_err(request_error)?;
                }
                DecodedItem::Payload(_) => {
                    push(&mut coverage, Coverage::Payload).map_err(request_error)?;
                }
            }
        }
        Ok(Self {
            decoded,
            instructions,
            coverage,
            atoms: Vec::new(),
            slots: Vec::new(),
            operations: Vec::new(),
            blocks: Vec::new(),
            edges: Vec::new(),
            exits: Vec::new(),
        })
    }

    fn build(&mut self) -> Result<(), FlowError> {
        self.build_atoms()?;
        let leaders = self.leaders()?;
        self.build_blocks(&leaders)?;
        self.build_edges()?;
        Ok(())
    }

    fn build_atoms(&mut self) -> Result<(), FlowError> {
        for (index, instruction) in self.instructions.iter().enumerate() {
            if matches!(instruction.opcode(), 0x0a..=0x0c) && instruction.producer_pc().is_some() {
                let producer_pc = instruction
                    .producer_pc()
                    .ok_or(internal("move-result has no checked producer"))?;
                let atom = self
                    .atoms
                    .len()
                    .checked_sub(1)
                    .ok_or(internal("result has no preceding atom"))?;
                let producer = self.atoms[atom].producer;
                if self.instructions[producer].pc() != producer_pc {
                    return Err(internal("result does not follow its original producer"));
                }
                if self.atoms[atom].result.is_some() {
                    return Err(internal("producer has two move-result atoms"));
                }
                let producer_instruction = self.instructions[producer];
                if producer_instruction.result_pc() != Some(instruction.pc()) {
                    return Err(internal("move-result adjacency disagrees with P1"));
                }
                self.atoms[atom].result = Some(index);
                continue;
            }
            if instruction.producer_pc().is_some() {
                return Err(internal("non-result instruction names a producer"));
            }
            push(
                &mut self.atoms,
                Atom {
                    producer: index,
                    result: None,
                },
            )
            .map_err(request_error)?;
        }
        for atom in &self.atoms {
            let producer = self.instructions[atom.producer];
            if producer.result_pc().is_some() != atom.result.is_some() {
                return Err(internal("P1 producer/result association is incomplete"));
            }
        }
        Ok(())
    }

    fn leaders(&mut self) -> Result<Vec<CodeUnitOffset>, FlowError> {
        let mut leaders = Vec::new();
        if self.decoded.is_reachable(CodeUnitOffset(0)) {
            push(&mut leaders, CodeUnitOffset(0)).map_err(request_error)?;
        }
        for target in self.decoded.handler_targets() {
            let target = CodeUnitOffset(target);
            if self.decoded.is_reachable(target) {
                push(&mut leaders, target).map_err(request_error)?;
            }
        }
        for instruction in &self.instructions {
            let pc = instruction.pc();
            if !self.decoded.is_reachable(pc) {
                continue;
            }
            match instruction.spec().flow {
                FlowKind::Goto | FlowKind::Conditional => {
                    if let Some(target) = instruction.branch_target()
                        && self.decoded.is_reachable(target)
                    {
                        push(&mut leaders, target).map_err(request_error)?;
                    }
                }
                FlowKind::PackedSwitch | FlowKind::SparseSwitch => {
                    for (_, target) in self.switch_cases(instruction)? {
                        if self.decoded.is_reachable(target) {
                            push(&mut leaders, target).map_err(request_error)?;
                        }
                    }
                }
                _ => {}
            }
            if matches!(
                instruction.spec().flow,
                FlowKind::Conditional | FlowKind::PackedSwitch | FlowKind::SparseSwitch
            ) && let Some(next) = pc.checked_advance(instruction.width())
                && self.decoded.is_reachable(next)
            {
                push(&mut leaders, next).map_err(request_error)?;
            }
        }
        leaders.sort_unstable();
        let mut unique = 0;
        for index in 0..leaders.len() {
            if unique == 0 || leaders[unique - 1] != leaders[index] {
                leaders[unique] = leaders[index];
                unique += 1;
            }
        }
        leaders.truncate(unique);
        for leader in &leaders {
            if self
                .instruction_at(*leader)
                .is_some_and(|instruction| matches!(instruction.opcode(), 0x0a..=0x0c))
            {
                return Err(internal("a leader points inside a producer/result atom"));
            }
        }
        Ok(leaders)
    }

    fn build_blocks(&mut self, leaders: &[CodeUnitOffset]) -> Result<(), FlowError> {
        let mut current: Option<usize> = None;
        for atom_index in 0..self.atoms.len() {
            let atom = self.atoms[atom_index];
            let producer = self.instructions[atom.producer];
            let pc = producer.pc();
            if !self.decoded.is_reachable(pc) {
                continue;
            }
            let starts_block = leaders.binary_search(&pc).is_ok() || current.is_none();
            if starts_block {
                let block_index = self.blocks.len();
                let block_id =
                    BlockId(u32::try_from(block_index).map_err(|_| FlowError::PlatformCapacity)?);
                let operation_start = u32::try_from(self.operations.len())
                    .map_err(|_| FlowError::PlatformCapacity)?;
                push(
                    &mut self.blocks,
                    Block {
                        id: block_id,
                        start_pc: pc,
                        operations: operation_start..operation_start,
                        terminal: None,
                        terminal_pc: None,
                        exceptional: None,
                        terminal_reads: SlotSpan::new(0, 0),
                    },
                )
                .map_err(request_error)?;
                current = Some(block_index);
            }
            let block_index = current.ok_or(internal("reachable atom has no block"))?;
            if self.blocks[block_index].terminal.is_some() {
                return Err(internal("block continues after a terminal"));
            }
            let terminal = matches!(
                producer.spec().flow,
                FlowKind::Return
                    | FlowKind::Throw
                    | FlowKind::Goto
                    | FlowKind::Conditional
                    | FlowKind::PackedSwitch
                    | FlowKind::SparseSwitch
            );
            if terminal {
                let (terminal, reads) = self.terminal(&atom, producer)?;
                let block_id = self.blocks[block_index].id;
                self.blocks[block_index].terminal = Some(terminal);
                self.blocks[block_index].terminal_pc = Some(pc);
                if producer.opcode() == 0x27 {
                    self.blocks[block_index].exceptional = Some(ExceptionCompletion {
                        throw_pc: pc,
                        selection_pc: pc,
                        frame: ExceptionFrame::PreserveBeforeOperation,
                        heap: HeapTransfer::Preserve,
                        lock: LockTransfer::Preserve,
                    });
                }
                self.blocks[block_index].set_terminal_reads(reads);
                let item = self.item_index(pc)?;
                self.coverage[item] = Coverage::Terminal(block_id);
            } else {
                let operation = self.operation(&atom, producer, block_index)?;
                let id = operation.id;
                let start = self.blocks[block_index].operations.start;
                push(&mut self.operations, operation).map_err(request_error)?;
                let end = u32::try_from(self.operations.len())
                    .map_err(|_| FlowError::PlatformCapacity)?;
                self.blocks[block_index].operations = start..end;
                let item = self.item_index(pc)?;
                self.coverage[item] = Coverage::Operation(id);
                if let Some(result) = atom.result {
                    let result_pc = self.instructions[result].pc();
                    let result_item = self.item_index(result_pc)?;
                    self.coverage[result_item] = Coverage::MergedResult(id);
                }
            }
        }
        // After a terminal or before a leader, the next atom must start a block.
        Ok(())
    }

    fn operation(
        &mut self,
        atom: &Atom,
        instruction: &DecodedInstruction,
        block_index: usize,
    ) -> Result<Operation, FlowError> {
        let pc = instruction.pc();
        let spec = instruction.spec();
        let opcode = spec.opcode;
        let block = self.blocks[block_index].id;
        let id =
            OpId(u32::try_from(self.operations.len()).map_err(|_| FlowError::PlatformCapacity)?);
        let (kind, reads, mut writes, mut effects, _, _) = match opcode {
            0x00 => (
                OpKind::Nop,
                self.empty_span(),
                self.empty_span(),
                Effects::default(),
                CommitRule::None,
                None::<CodeUnitOffset>,
            ),
            0x01..=0x09 => {
                let kind = match opcode {
                    0x01..=0x03 => MoveKind::Word,
                    0x04..=0x06 => MoveKind::Wide,
                    _ => MoveKind::Object,
                };
                let (dest, source) = self.two_regs(instruction)?;
                let width = match kind {
                    MoveKind::Wide => SlotWidth::Two,
                    _ => SlotWidth::One,
                };
                let reads = self.slots_for([RegisterSlot::new(source, width)])?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)])?;
                (
                    OpKind::Move { kind },
                    reads,
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x0a..=0x0c => {
                if self.decoded.result_issue_at(pc).is_none() {
                    return Err(internal("unmerged move-result lost its P1 issue"));
                }
                let width = if opcode == 0x0b {
                    SlotWidth::Two
                } else {
                    SlotWidth::One
                };
                let dest = self.register(instruction, 0)?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)])?;
                (
                    OpKind::UnresolvedResult { width },
                    self.empty_span(),
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x0d => {
                let dest = self.register(instruction, 0)?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::MoveException,
                    self.empty_span(),
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x12..=0x19 => {
                let dest = self.register(instruction, 0)?;
                let width = if opcode >= 0x16 {
                    SlotWidth::Two
                } else {
                    SlotWidth::One
                };
                let bits = self.literal_bits(instruction)?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)])?;
                (
                    OpKind::Const { bits, width },
                    self.empty_span(),
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x1a | 0x1b => {
                let dest = self.register(instruction, 0)?;
                let value = match self.first_reference(instruction) {
                    Some(OperandReference::String(value)) => value,
                    _ => return Err(internal("const-string has no string reference")),
                };
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::ConstString { value },
                    self.empty_span(),
                    writes,
                    Effects::MAY_THROW.with(Effects::CLASS_INIT),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x1c => {
                let dest = self.register(instruction, 0)?;
                let type_id = match self.first_reference(instruction) {
                    Some(OperandReference::Type(value)) => value,
                    _ => return Err(internal("const-class has no type reference")),
                };
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::ConstClass { type_id },
                    self.empty_span(),
                    writes,
                    Effects::MAY_THROW,
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x1d | 0x1e => {
                let reg = self.register(instruction, 0)?;
                let reads = self.slots_for([RegisterSlot::new(reg, SlotWidth::One)])?;
                let effects = Effects::MONITOR.with(Effects::MAY_THROW);
                (
                    OpKind::Monitor {
                        enter: opcode == 0x1d,
                    },
                    reads,
                    self.empty_span(),
                    effects,
                    CommitRule::None,
                    None,
                )
            }
            0x1f => {
                let reg = self.register(instruction, 0)?;
                let type_id = match self.first_reference(instruction) {
                    Some(OperandReference::Type(value)) => value,
                    _ => return Err(internal("check-cast has no type reference")),
                };
                let reads = self.slots_for([RegisterSlot::new(reg, SlotWidth::One)])?;
                (
                    OpKind::Cast { type_id },
                    reads,
                    self.empty_span(),
                    Effects::MAY_THROW,
                    CommitRule::None,
                    None,
                )
            }
            0x20 => {
                let (dest, source) = self.two_regs(instruction)?;
                let type_id = match self.first_reference(instruction) {
                    Some(OperandReference::Type(value)) => value,
                    _ => return Err(internal("instance-of has no type reference")),
                };
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)])?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::InstanceOf { type_id },
                    reads,
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x21 => {
                let (dest, source) = self.two_regs(instruction)?;
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)])?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::ArrayLength,
                    reads,
                    writes,
                    Effects::MAY_THROW.with(Effects::HEAP_READ),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x22 => {
                let dest = self.register(instruction, 0)?;
                let type_id = match self.first_reference(instruction) {
                    Some(OperandReference::Type(value)) => value,
                    _ => return Err(internal("new-instance has no type reference")),
                };
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::NewInstance { type_id },
                    self.empty_span(),
                    writes,
                    Effects::MAY_THROW
                        .with(Effects::CLASS_INIT)
                        .with(Effects::ALLOCATION),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x23 => {
                let (dest, size) = self.two_regs(instruction)?;
                let type_id = match self.first_reference(instruction) {
                    Some(OperandReference::Type(value)) => value,
                    _ => return Err(internal("new-array has no type reference")),
                };
                let reads = self.slots_for([RegisterSlot::new(size, SlotWidth::One)])?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::NewArray { type_id },
                    reads,
                    writes,
                    Effects::MAY_THROW.with(Effects::ALLOCATION),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x24 | 0x25 => {
                let type_id = match self.first_reference(instruction) {
                    Some(OperandReference::Type(value)) => value,
                    _ => return Err(internal("filled-new-array has no type reference")),
                };
                let reads = self.all_register_slots(instruction, SlotWidth::One)?;
                (
                    OpKind::FilledNewArray { type_id },
                    reads,
                    self.empty_span(),
                    Effects::MAY_THROW
                        .with(Effects::ALLOCATION)
                        .with(Effects::HEAP_WRITE),
                    CommitRule::None,
                    None,
                )
            }
            0x26 => {
                let array = self.register(instruction, 0)?;
                let payload = instruction
                    .payload_target()
                    .ok_or(internal("fill-array-data has no checked payload"))?;
                let reads = self.slots_for([RegisterSlot::new(array, SlotWidth::One)])?;
                (
                    OpKind::FillArrayData { payload },
                    reads,
                    self.empty_span(),
                    Effects::MAY_THROW.with(Effects::HEAP_WRITE),
                    CommitRule::None,
                    None,
                )
            }
            0x2d..=0x31 => {
                let (dest, left, right) = self.three_regs(instruction)?;
                let op = match opcode {
                    0x2d => CompareOp::CmplFloat,
                    0x2e => CompareOp::CmpgFloat,
                    0x2f => CompareOp::CmplDouble,
                    0x30 => CompareOp::CmpgDouble,
                    _ => CompareOp::CmpLong,
                };
                let left_width = match op {
                    CompareOp::CmpLong | CompareOp::CmplDouble | CompareOp::CmpgDouble => {
                        SlotWidth::Two
                    }
                    _ => SlotWidth::One,
                };
                let reads = self.slots_for([
                    RegisterSlot::new(left, left_width),
                    RegisterSlot::new(right, left_width),
                ])?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::Compare { op },
                    reads,
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x44..=0x51 => {
                let (dest, array, index) = self.three_regs(instruction)?;
                let get = matches!(opcode, 0x44..=0x4a);
                let kind = self.array_kind(opcode);
                let width = kind.width();
                let reads = if get {
                    self.slots_for([
                        RegisterSlot::new(array, SlotWidth::One),
                        RegisterSlot::new(index, SlotWidth::One),
                    ])?
                } else {
                    self.slots_for([
                        RegisterSlot::new(dest, width),
                        RegisterSlot::new(array, SlotWidth::One),
                        RegisterSlot::new(index, SlotWidth::One),
                    ])?
                };
                let writes = if get {
                    self.slots_for([RegisterSlot::new(dest, width)])?
                } else {
                    self.empty_span()
                };
                (
                    OpKind::Array {
                        access: if get {
                            ArrayAccess::Get
                        } else {
                            ArrayAccess::Put
                        },
                        kind,
                    },
                    reads,
                    writes,
                    Effects::MAY_THROW.with(if get {
                        Effects::HEAP_READ
                    } else {
                        Effects::HEAP_WRITE
                    }),
                    if get {
                        CommitRule::AtOperation
                    } else {
                        CommitRule::None
                    },
                    None,
                )
            }
            0x52..=0x6d => {
                let static_field = opcode >= 0x60;
                let (get, kind) = if static_field {
                    (opcode < 0x67, self.field_kind((opcode - 0x60) % 7))
                } else {
                    (opcode < 0x59, self.field_kind((opcode - 0x52) % 7))
                };
                let field = match self.first_reference(instruction) {
                    Some(OperandReference::Field(value)) => value,
                    _ => return Err(internal("field operation has no field reference")),
                };
                let dest = self.register(instruction, 0)?;
                let width = kind.width();
                let mut reads = Vec::new();
                if !static_field {
                    push(
                        &mut reads,
                        RegisterSlot::new(self.register(instruction, 1)?, SlotWidth::One),
                    )
                    .map_err(request_error)?;
                }
                if !get {
                    crate::substrate::owned::reserve(&mut reads, 1).map_err(request_error)?;
                    reads.insert(0, RegisterSlot::new(dest, width));
                }
                let reads = self.slots_vec(reads)?;
                let writes = if get {
                    self.slots_for([RegisterSlot::new(dest, width)])?
                } else {
                    self.empty_span()
                };
                let mut effects = Effects::MAY_THROW.with(if get {
                    Effects::HEAP_READ
                } else {
                    Effects::HEAP_WRITE
                });
                if static_field {
                    effects = effects.with(Effects::CLASS_INIT);
                }
                (
                    OpKind::Field {
                        access: if get {
                            FieldAccess::Get
                        } else {
                            FieldAccess::Put
                        },
                        static_field,
                        field,
                        kind,
                    },
                    reads,
                    writes,
                    effects,
                    if get {
                        CommitRule::AtOperation
                    } else {
                        CommitRule::None
                    },
                    None,
                )
            }
            0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd => {
                let target = match (opcode, self.first_reference(instruction)) {
                    (0xfc | 0xfd, Some(OperandReference::CallSite(value))) => {
                        OperandReference::CallSite(value)
                    }
                    (_, Some(OperandReference::Method(value))) => OperandReference::Method(value),
                    _ => return Err(internal("invoke has no checked target reference")),
                };
                let kind = self.invoke_kind(opcode);
                let prototype = if kind == InvokeKind::Polymorphic {
                    match instruction
                        .references()
                        .find(|reference| reference.slot == crate::raw::ReferenceSlot::Second)
                        .map(|reference| reference.target)
                    {
                        Some(OperandReference::Proto(id)) => Some(id),
                        _ => return Err(internal("polymorphic call has no checked prototype")),
                    }
                } else {
                    None
                };
                let layout = self
                    .decoded
                    .invoke_layout_at(pc)
                    .ok_or(internal("invoke has no P1 layout state"))?;
                let (kind, reads) = match layout {
                    Ok(layout) => {
                        let reads = self.invoke_reads(instruction, layout)?;
                        (
                            OpKind::Invoke {
                                kind,
                                target,
                                prototype,
                                result: layout.result(),
                            },
                            reads,
                        )
                    }
                    Err(_) => (
                        OpKind::UnresolvedInvoke {
                            kind,
                            target,
                            prototype,
                            result: self
                                .decoded
                                .invoke_result_at(pc)
                                .and_then(Result::ok)
                                .map(InvokeResult::from)
                                .unwrap_or(InvokeResult::Unavailable),
                        },
                        self.empty_span(),
                    ),
                };
                (
                    kind,
                    reads,
                    self.empty_span(),
                    Effects::CALL
                        .with(Effects::MAY_THROW)
                        .with(Effects::HEAP_READ)
                        .with(Effects::HEAP_WRITE)
                        .with(Effects::CLASS_INIT),
                    CommitRule::None,
                    None,
                )
            }
            0x7b..=0x8f => {
                let (dest, source) = self.two_regs(instruction)?;
                let op = self.unary_op(opcode)?;
                let reads = self.slots_for([RegisterSlot::new(source, op.source_width())])?;
                let writes = self.slots_for([RegisterSlot::new(dest, op.dest_width())])?;
                (
                    OpKind::Unary { op },
                    reads,
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x90..=0xaf => {
                let (dest, left, right) = self.three_regs(instruction)?;
                let op = binary_op((opcode - 0x90) % 32)?;
                let width = op.width();
                let reads = self.slots_for([
                    RegisterSlot::new(left, op.left_width()),
                    RegisterSlot::new(right, op.right_width()),
                ])?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)])?;
                (
                    OpKind::Binary {
                        op,
                        form: BinaryForm::ThreeAddress,
                        literal: None,
                    },
                    reads,
                    writes,
                    self.binary_effects(op),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0xb0..=0xcf => {
                let (dest, right) = self.two_regs(instruction)?;
                let op = binary_op((opcode - 0xb0) % 32)?;
                let width = op.width();
                let reads = self.slots_for([
                    RegisterSlot::new(dest, op.left_width()),
                    RegisterSlot::new(right, op.right_width()),
                ])?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)])?;
                (
                    OpKind::Binary {
                        op,
                        form: BinaryForm::TwoAddress,
                        literal: None,
                    },
                    reads,
                    writes,
                    self.binary_effects(op),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0xd0..=0xd7 => {
                let (dest, source) = self.two_regs(instruction)?;
                let op = lit16_op((opcode - 0xd0) % 8)?;
                let literal = self.signed_literal(instruction)?;
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)])?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::Binary {
                        op,
                        form: BinaryForm::Literal,
                        literal: Some(literal),
                    },
                    reads,
                    writes,
                    self.binary_effects(op),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0xd8..=0xe2 => {
                let (dest, source) = self.two_regs(instruction)?;
                let op = lit8_op((opcode - 0xd8) % 11)?;
                let literal = self.signed_literal(instruction)?;
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)])?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::Binary {
                        op,
                        form: BinaryForm::Literal,
                        literal: Some(literal),
                    },
                    reads,
                    writes,
                    self.binary_effects(op),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0xfe => {
                let dest = self.register(instruction, 0)?;
                let handle = match self.first_reference(instruction) {
                    Some(OperandReference::MethodHandle(value)) => value,
                    _ => {
                        return Err(internal(
                            "const-method-handle has no method-handle reference",
                        ));
                    }
                };
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::ConstMethodHandle { handle },
                    self.empty_span(),
                    writes,
                    Effects::MAY_THROW,
                    CommitRule::AtOperation,
                    None,
                )
            }
            0xff => {
                let dest = self.register(instruction, 0)?;
                let proto = match self.first_reference(instruction) {
                    Some(OperandReference::Proto(value)) => value,
                    _ => return Err(internal("const-method-type has no proto reference")),
                };
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)])?;
                (
                    OpKind::ConstMethodType { proto },
                    self.empty_span(),
                    writes,
                    Effects::MAY_THROW,
                    CommitRule::AtOperation,
                    None,
                )
            }
            _ => {
                return Err(FlowError::Unsupported {
                    pc,
                    opcode,
                    reason: "opcode has no P2 operation translation",
                });
            }
        };
        if instruction.spec().may_throw {
            effects = effects.with(Effects::MAY_THROW);
        }
        let result_pc = atom.result.map(|index| self.instructions[index].pc());
        if let Some(result_index) = atom.result {
            let result_instruction = self.instructions[result_index];
            let dest = self.register(result_instruction, 0)?;
            let width = match result_instruction.opcode() {
                0x0b => SlotWidth::Two,
                _ => SlotWidth::One,
            };
            writes = self.slots_for([RegisterSlot::new(dest, width)])?;
        }
        let commit = if atom.result.is_some() {
            CommitRule::AfterResult
        } else if writes.is_empty() {
            CommitRule::None
        } else {
            CommitRule::AtOperation
        };
        let commit_pc = match commit {
            CommitRule::None => None,
            CommitRule::AtOperation => Some(pc),
            CommitRule::AfterResult => result_pc,
        };
        let heap = if effects.contains(Effects::HEAP_WRITE)
            || effects.contains(Effects::ALLOCATION)
            || effects.contains(Effects::CLASS_INIT)
            || effects.contains(Effects::MONITOR)
        {
            HeapTransfer::EventOutcome
        } else {
            HeapTransfer::Preserve
        };
        let lock = match kind {
            OpKind::Monitor { enter: true } => LockTransfer::Acquire,
            OpKind::Monitor { enter: false } => LockTransfer::Release,
            _ => LockTransfer::Preserve,
        };
        let exceptional = if effects.may_throw() {
            Some(ExceptionCompletion {
                throw_pc: pc,
                selection_pc: if opcode == 0x1e {
                    pc.checked_advance(instruction.width())
                        .ok_or(FlowError::PlatformCapacity)?
                } else {
                    pc
                },
                frame: ExceptionFrame::PreserveBeforeOperation,
                heap,
                lock: if opcode == 0x1e {
                    LockTransfer::ReleaseOutcome
                } else {
                    LockTransfer::Preserve
                },
            })
        } else {
            None
        };
        Ok(Operation {
            id,
            block,
            pc,
            result_pc,
            kind,
            reads,
            writes,
            effects,
            normal: NormalCompletion {
                reads: ReadOrder::CaptureAllBeforeCommit,
                commit,
                commit_pc,
                heap,
                lock,
            },
            exceptional,
        })
    }

    fn terminal(
        &mut self,
        _atom: &Atom,
        instruction: &DecodedInstruction,
    ) -> Result<(Terminal, SlotSpan), FlowError> {
        let pc = instruction.pc();
        match instruction.spec().flow {
            FlowKind::Goto => Ok((
                Terminal::Goto {
                    target: instruction
                        .branch_target()
                        .ok_or(internal("goto has no checked target"))?,
                },
                self.empty_span(),
            )),
            FlowKind::Conditional => {
                let op = branch_op(instruction.opcode())?;
                let target = instruction
                    .branch_target()
                    .ok_or(internal("conditional has no checked target"))?;
                let count = if instruction.opcode() < 0x38 { 2 } else { 1 };
                let mut slots = Vec::new();
                for index in 0..count {
                    push(
                        &mut slots,
                        RegisterSlot::new(self.register(instruction, index)?, SlotWidth::One),
                    )
                    .map_err(request_error)?;
                }
                let fallthrough = pc
                    .checked_advance(instruction.width())
                    .ok_or(FlowError::PlatformCapacity)?;
                Ok((
                    Terminal::Branch {
                        op,
                        target,
                        fallthrough,
                    },
                    self.slots_vec(slots)?,
                ))
            }
            FlowKind::PackedSwitch | FlowKind::SparseSwitch => {
                let selector = self.register(instruction, 0)?;
                let payload = instruction
                    .payload_target()
                    .ok_or(internal("switch has no checked payload"))?;
                let default = pc
                    .checked_advance(instruction.width())
                    .ok_or(FlowError::PlatformCapacity)?;
                let cases = self.switch_cases(instruction)?;
                Ok((
                    Terminal::Switch {
                        kind: if instruction.opcode() == 0x2b {
                            SwitchKind::Packed
                        } else {
                            SwitchKind::Sparse
                        },
                        payload,
                        default,
                        cases,
                    },
                    self.slots_for([RegisterSlot::new(selector, SlotWidth::One)])?,
                ))
            }
            FlowKind::Return => {
                let width = match instruction.opcode() {
                    0x0e => None,
                    0x10 => Some(SlotWidth::Two),
                    _ => Some(SlotWidth::One),
                };
                let reads = match width {
                    Some(width) => {
                        self.slots_for([RegisterSlot::new(self.register(instruction, 0)?, width)])?
                    }
                    None => self.empty_span(),
                };
                Ok((Terminal::Return { width, pc }, reads))
            }
            FlowKind::Throw => {
                let reads = self.slots_for([RegisterSlot::new(
                    self.register(instruction, 0)?,
                    SlotWidth::One,
                )])?;
                Ok((Terminal::Throw { pc }, reads))
            }
            _ => Err(internal(
                "non-terminal instruction entered terminal builder",
            )),
        }
    }

    fn build_edges(&mut self) -> Result<(), FlowError> {
        for index in 0..self.blocks.len() {
            let id = self.blocks[index].id;
            match &self.blocks[index].terminal {
                Some(Terminal::Goto { target }) => {
                    self.normal_edge(id, NormalEdge::Goto, self.block_at_pc(*target)?)?;
                }
                Some(Terminal::Branch {
                    target,
                    fallthrough,
                    ..
                }) => {
                    let target = self.block_at_pc(*target)?;
                    let fallthrough = self.block_at_pc(*fallthrough)?;
                    self.normal_edge(id, NormalEdge::ConditionalTrue, target)?;
                    self.normal_edge(id, NormalEdge::ConditionalFalse, fallthrough)?;
                }
                Some(Terminal::Switch { default, cases, .. }) => {
                    let default = self.block_at_pc(*default)?;
                    let count = cases.len();
                    self.normal_edge(id, NormalEdge::SwitchDefault, default)?;
                    for ordinal in 0..count {
                        let Some(Terminal::Switch { cases, .. }) = &self.blocks[index].terminal
                        else {
                            return Err(internal("switch terminal changed while building edges"));
                        };
                        let (key, target) = cases[ordinal];
                        let target = self.block_at_pc(target)?;
                        let case_index =
                            u32::try_from(ordinal).map_err(|_| FlowError::PlatformCapacity)?;
                        self.normal_edge(
                            id,
                            NormalEdge::SwitchCase {
                                key,
                                index: case_index,
                            },
                            target,
                        )?;
                    }
                }
                Some(Terminal::Return { pc, .. }) => {
                    self.exit(id, *pc, ExitKind::Return)?;
                }
                Some(Terminal::Throw { pc }) => {
                    self.exception_edges(id, None, *pc, *pc)?;
                }
                None => {
                    let target = self.next_block(&self.blocks[index])?;
                    self.normal_edge(id, NormalEdge::Fallthrough, target)?;
                }
            }
        }
        for index in 0..self.operations.len() {
            let operation = &self.operations[index];
            if !operation.effects.may_throw() {
                continue;
            }
            let pc = operation.pc;
            let id = operation.id;
            let block = operation.block;
            let selection = operation
                .exceptional
                .ok_or(internal("throwing operation lacks transfer"))?
                .selection_pc;
            self.exception_edges(block, Some(id), pc, selection)?;
        }
        Ok(())
    }

    fn exception_edges(
        &mut self,
        block: BlockId,
        source_op: Option<OpId>,
        throw_pc: CodeUnitOffset,
        selection_pc: CodeUnitOffset,
    ) -> Result<(), FlowError> {
        let clauses = self
            .decoded
            .catch_clauses_at(selection_pc)
            .map_err(request_error)?;
        if clauses.is_empty() {
            let exit = self.exit(block, throw_pc, ExitKind::Uncaught)?;
            self.push_edge(Edge {
                id: self.next_edge_id()?,
                from: block,
                source_op,
                role: EdgeRole::Exception {
                    throw_pc,
                    selection_pc,
                    order: 0,
                    catch: None,
                },
                target: EdgeTarget::Exit(exit),
            })?;
            return Ok(());
        }
        let clause_count = clauses.len();
        let mut has_catch_all = false;
        for (order, (catch, target)) in clauses.into_iter().enumerate() {
            if catch.is_none() {
                has_catch_all = true;
            }
            let target = self.block_at_pc(CodeUnitOffset(target))?;
            self.push_edge(Edge {
                id: self.next_edge_id()?,
                from: block,
                source_op,
                role: EdgeRole::Exception {
                    throw_pc,
                    selection_pc,
                    order: u32::try_from(order).map_err(|_| FlowError::PlatformCapacity)?,
                    catch: catch.map(TypeId),
                },
                target: EdgeTarget::Block(target),
            })?;
        }
        if !has_catch_all {
            let exit = self.exit(block, throw_pc, ExitKind::Uncaught)?;
            self.push_edge(Edge {
                id: self.next_edge_id()?,
                from: block,
                source_op,
                role: EdgeRole::Exception {
                    throw_pc,
                    selection_pc,
                    order: u32::try_from(clause_count).map_err(|_| FlowError::PlatformCapacity)?,
                    catch: None,
                },
                target: EdgeTarget::Exit(exit),
            })?;
        }
        Ok(())
    }

    fn normal_edge(
        &mut self,
        from: BlockId,
        role: NormalEdge,
        target: BlockId,
    ) -> Result<(), FlowError> {
        self.push_edge(Edge {
            id: self.next_edge_id()?,
            from,
            source_op: None,
            role: EdgeRole::Normal(role),
            target: EdgeTarget::Block(target),
        })
    }

    fn exit(
        &mut self,
        block: BlockId,
        pc: CodeUnitOffset,
        kind: ExitKind,
    ) -> Result<ExitId, FlowError> {
        let id = ExitId(u32::try_from(self.exits.len()).map_err(|_| FlowError::PlatformCapacity)?);
        push(
            &mut self.exits,
            Exit {
                id,
                block,
                pc,
                kind,
            },
        )
        .map_err(request_error)?;
        Ok(id)
    }

    fn push_edge(&mut self, edge: Edge) -> Result<(), FlowError> {
        push(&mut self.edges, edge).map_err(request_error)
    }

    fn next_edge_id(&self) -> Result<EdgeId, FlowError> {
        Ok(EdgeId(
            u32::try_from(self.edges.len()).map_err(|_| FlowError::PlatformCapacity)?,
        ))
    }

    fn next_block(&self, block: &Block) -> Result<BlockId, FlowError> {
        if block.terminal.is_some() {
            return Err(internal("terminal block has no fallthrough"));
        }
        let end = block.operations.end;
        if end == 0 {
            return Err(internal("empty non-terminal block has no fallthrough"));
        }
        let last = &self.operations[(end - 1) as usize];
        let after_pc = last.result_pc.unwrap_or(last.pc);
        let instruction = self
            .instruction_at(after_pc)
            .ok_or(internal("fallthrough site has no instruction"))?;
        let pc = after_pc
            .checked_advance(instruction.width())
            .ok_or(FlowError::PlatformCapacity)?;
        self.block_at_pc(pc)
    }
    fn block_at_pc(&self, pc: CodeUnitOffset) -> Result<BlockId, FlowError> {
        self.blocks
            .binary_search_by_key(&pc, |block| block.start_pc)
            .map(|index| self.blocks[index].id)
            .map_err(|_| internal("checked target has no block"))
    }

    fn switch_cases(
        &self,
        instruction: &DecodedInstruction,
    ) -> Result<Vec<(i64, CodeUnitOffset)>, FlowError> {
        let payload_pc = instruction
            .payload_target()
            .ok_or(internal("switch has no checked payload"))?;
        let index = self.item_index(payload_pc)?;
        let DecodedItem::Payload(payload) = &self.decoded.items()[index] else {
            return Err(internal("switch target is not a P1 payload"));
        };
        let cases = payload
            .switch_cases(self.decoded.source())
            .map_err(request_error)?
            .ok_or(internal("switch payload has no cases"))?;
        let mut result = Vec::new();
        for case in cases {
            let target = i64::from(instruction.pc().0)
                .checked_add(i64::from(case.offset.0))
                .and_then(|value| u32::try_from(value).ok())
                .map(CodeUnitOffset)
                .ok_or(FlowError::PlatformCapacity)?;
            push(&mut result, (case.key, target)).map_err(request_error)?;
        }
        Ok(result)
    }

    fn instruction_at(&self, pc: CodeUnitOffset) -> Option<&DecodedInstruction> {
        self.decoded.instruction_at(pc)
    }

    fn item_index(&self, pc: CodeUnitOffset) -> Result<usize, FlowError> {
        self.decoded
            .items()
            .binary_search_by_key(&pc, DecodedItem::pc)
            .map_err(|_| internal("coverage pc is not a P1 item"))
    }

    fn empty_span(&self) -> SlotSpan {
        // Empty spans need no arena address. Nonempty spans use checked IDs.
        SlotSpan::new(0, 0)
    }

    fn slots_for<const N: usize>(
        &mut self,
        slots: [RegisterSlot; N],
    ) -> Result<SlotSpan, FlowError> {
        let start = u32::try_from(self.slots.len()).map_err(|_| FlowError::PlatformCapacity)?;
        for slot in slots {
            push(&mut self.slots, slot).map_err(request_error)?;
        }
        let len = u32::try_from(self.slots.len())
            .map_err(|_| FlowError::PlatformCapacity)?
            .checked_sub(start)
            .ok_or(FlowError::PlatformCapacity)?;
        Ok(SlotSpan::new(start, len))
    }

    fn slots_vec(&mut self, slots: Vec<RegisterSlot>) -> Result<SlotSpan, FlowError> {
        let start = u32::try_from(self.slots.len()).map_err(|_| FlowError::PlatformCapacity)?;
        for slot in slots {
            push(&mut self.slots, slot).map_err(request_error)?;
        }
        let len = u32::try_from(self.slots.len())
            .map_err(|_| FlowError::PlatformCapacity)?
            .checked_sub(start)
            .ok_or(FlowError::PlatformCapacity)?;
        Ok(SlotSpan::new(start, len))
    }

    fn register(
        &self,
        instruction: &DecodedInstruction,
        index: usize,
    ) -> Result<RegisterId, FlowError> {
        instruction
            .registers()
            .nth(index)
            .ok_or(internal("instruction lost a checked register operand"))
    }

    fn two_regs(
        &self,
        instruction: &DecodedInstruction,
    ) -> Result<(RegisterId, RegisterId), FlowError> {
        Ok((
            self.register(instruction, 0)?,
            self.register(instruction, 1)?,
        ))
    }

    fn three_regs(
        &self,
        instruction: &DecodedInstruction,
    ) -> Result<(RegisterId, RegisterId, RegisterId), FlowError> {
        Ok((
            self.register(instruction, 0)?,
            self.register(instruction, 1)?,
            self.register(instruction, 2)?,
        ))
    }

    fn first_reference(&self, instruction: &DecodedInstruction) -> Option<OperandReference> {
        instruction
            .references()
            .next()
            .map(|reference| reference.target)
    }

    fn literal_bits(&self, instruction: &DecodedInstruction) -> Result<u64, FlowError> {
        match instruction.operands().literal() {
            Some(RawLiteral::Bits32(value)) => Ok(u64::from(value)),
            Some(RawLiteral::Bits64(value)) => Ok(value),
            Some(RawLiteral::Signed8(value)) => Ok(u64::from(value as u32)),
            Some(RawLiteral::Signed16(value)) => Ok(u64::from(value as u32)),
            None => Err(internal("constant instruction has no literal")),
        }
    }

    fn signed_literal(&self, instruction: &DecodedInstruction) -> Result<i64, FlowError> {
        match instruction.operands().literal() {
            Some(RawLiteral::Signed8(value)) => Ok(i64::from(value)),
            Some(RawLiteral::Signed16(value)) => Ok(i64::from(value)),
            Some(RawLiteral::Bits32(value)) => Ok(i64::from(value as i32)),
            Some(RawLiteral::Bits64(value)) => Ok(value as i64),
            None => Err(internal("literal operation has no literal")),
        }
    }

    fn all_register_slots(
        &mut self,
        instruction: &DecodedInstruction,
        width: SlotWidth,
    ) -> Result<SlotSpan, FlowError> {
        let start = u32::try_from(self.slots.len()).map_err(|_| FlowError::PlatformCapacity)?;
        for register in instruction.registers() {
            push(&mut self.slots, RegisterSlot::new(register, width)).map_err(request_error)?;
        }
        let end = u32::try_from(self.slots.len()).map_err(|_| FlowError::PlatformCapacity)?;
        Ok(SlotSpan::new(start, end - start))
    }

    fn invoke_reads(
        &mut self,
        instruction: &DecodedInstruction,
        layout: &crate::p1::InvokeLayout,
    ) -> Result<SlotSpan, FlowError> {
        let mut regs = instruction.registers();
        let mut slots = Vec::new();
        if layout.has_receiver() {
            push(
                &mut slots,
                RegisterSlot::new(
                    regs.next()
                        .ok_or(internal("invoke lost its checked receiver"))?,
                    SlotWidth::One,
                ),
            )
            .map_err(request_error)?;
        }
        for width in layout.arguments() {
            let register = regs
                .next()
                .ok_or(internal("invoke lost a checked argument"))?;
            push(&mut slots, RegisterSlot::new(register, *width)).map_err(request_error)?;
            if *width == SlotWidth::Two {
                let second = regs
                    .next()
                    .ok_or(internal("invoke lost a wide argument word"))?;
                if register.0.checked_add(1) != Some(second.0) {
                    return Err(internal("invoke wide argument is not adjacent"));
                }
            }
        }
        if regs.next().is_some() {
            return Err(internal("invoke has unclaimed register words"));
        }
        self.slots_vec(slots)
    }

    fn unary_op(&self, opcode: u8) -> Result<UnaryOp, FlowError> {
        let index = opcode - 0x7b;
        Ok(match index {
            0 => UnaryOp::NegInt,
            1 => UnaryOp::NotInt,
            2 => UnaryOp::NegLong,
            3 => UnaryOp::NotLong,
            4 => UnaryOp::NegFloat,
            5 => UnaryOp::NegDouble,
            6 => UnaryOp::IntToLong,
            7 => UnaryOp::IntToFloat,
            8 => UnaryOp::IntToDouble,
            9 => UnaryOp::LongToInt,
            10 => UnaryOp::LongToFloat,
            11 => UnaryOp::LongToDouble,
            12 => UnaryOp::FloatToInt,
            13 => UnaryOp::FloatToLong,
            14 => UnaryOp::FloatToDouble,
            15 => UnaryOp::DoubleToInt,
            16 => UnaryOp::DoubleToLong,
            17 => UnaryOp::DoubleToFloat,
            18 => UnaryOp::IntToByte,
            19 => UnaryOp::IntToChar,
            20 => UnaryOp::IntToShort,
            _ => return Err(internal("unary opcode index is not checked")),
        })
    }

    fn binary_effects(&self, op: BinaryOp) -> Effects {
        if op.integer()
            && matches!(
                op,
                BinaryOp::DivInt | BinaryOp::RemInt | BinaryOp::DivLong | BinaryOp::RemLong
            )
        {
            Effects::MAY_THROW
        } else {
            Effects::default()
        }
    }

    fn array_kind(&self, opcode: u8) -> ValueKind {
        match (opcode - 0x44) % 7 {
            0 => ValueKind::Word,
            1 => ValueKind::Wide,
            2 => ValueKind::Object,
            3 => ValueKind::Boolean,
            4 => ValueKind::Byte,
            5 => ValueKind::Char,
            _ => ValueKind::Short,
        }
    }

    fn field_kind(&self, index: u8) -> ValueKind {
        match index {
            0 => ValueKind::Word,
            1 => ValueKind::Wide,
            2 => ValueKind::Object,
            3 => ValueKind::Boolean,
            4 => ValueKind::Byte,
            5 => ValueKind::Char,
            _ => ValueKind::Short,
        }
    }

    fn invoke_kind(&self, opcode: u8) -> InvokeKind {
        match opcode {
            0x6e | 0x74 => InvokeKind::Virtual,
            0x6f | 0x75 => InvokeKind::Super,
            0x70 | 0x76 => InvokeKind::Direct,
            0x71 | 0x77 => InvokeKind::Static,
            0x72 | 0x78 => InvokeKind::Interface,
            0xfa | 0xfb => InvokeKind::Polymorphic,
            _ => InvokeKind::Custom,
        }
    }

    fn finish(self) -> FlowParts {
        FlowParts {
            slots: self.slots,
            operations: self.operations,
            blocks: self.blocks,
            edges: self.edges,
            exits: self.exits,
            coverage: self.coverage,
        }
    }
}

fn binary_op(index: u8) -> Result<BinaryOp, FlowError> {
    let op = match index {
        0 => BinaryOp::AddInt,
        1 => BinaryOp::SubInt,
        2 => BinaryOp::MulInt,
        3 => BinaryOp::DivInt,
        4 => BinaryOp::RemInt,
        5 => BinaryOp::AndInt,
        6 => BinaryOp::OrInt,
        7 => BinaryOp::XorInt,
        8 => BinaryOp::ShlInt,
        9 => BinaryOp::ShrInt,
        10 => BinaryOp::UshrInt,
        11 => BinaryOp::AddLong,
        12 => BinaryOp::SubLong,
        13 => BinaryOp::MulLong,
        14 => BinaryOp::DivLong,
        15 => BinaryOp::RemLong,
        16 => BinaryOp::AndLong,
        17 => BinaryOp::OrLong,
        18 => BinaryOp::XorLong,
        19 => BinaryOp::ShlLong,
        20 => BinaryOp::ShrLong,
        21 => BinaryOp::UshrLong,
        22 => BinaryOp::AddFloat,
        23 => BinaryOp::SubFloat,
        24 => BinaryOp::MulFloat,
        25 => BinaryOp::DivFloat,
        26 => BinaryOp::RemFloat,
        27 => BinaryOp::AddDouble,
        28 => BinaryOp::SubDouble,
        29 => BinaryOp::MulDouble,
        30 => BinaryOp::DivDouble,
        31 => BinaryOp::RemDouble,
        _ => return Err(internal("binary opcode index is not checked")),
    };
    Ok(op)
}

fn lit16_op(index: u8) -> Result<BinaryOp, FlowError> {
    Ok(match index {
        0 => BinaryOp::AddInt,
        1 => BinaryOp::RsubInt,
        2 => BinaryOp::MulInt,
        3 => BinaryOp::DivInt,
        4 => BinaryOp::RemInt,
        5 => BinaryOp::AndInt,
        6 => BinaryOp::OrInt,
        7 => BinaryOp::XorInt,
        _ => return Err(internal("lit16 opcode index is not checked")),
    })
}

fn lit8_op(index: u8) -> Result<BinaryOp, FlowError> {
    Ok(match index {
        0 => BinaryOp::AddInt,
        1 => BinaryOp::RsubInt,
        2 => BinaryOp::MulInt,
        3 => BinaryOp::DivInt,
        4 => BinaryOp::RemInt,
        5 => BinaryOp::AndInt,
        6 => BinaryOp::OrInt,
        7 => BinaryOp::XorInt,
        8 => BinaryOp::ShlInt,
        9 => BinaryOp::ShrInt,
        10 => BinaryOp::UshrInt,
        _ => return Err(internal("lit8 opcode index is not checked")),
    })
}

fn branch_op(opcode: u8) -> Result<BranchOp, FlowError> {
    Ok(match opcode {
        0x32 => BranchOp::Eq,
        0x33 => BranchOp::Ne,
        0x34 => BranchOp::Lt,
        0x35 => BranchOp::Ge,
        0x36 => BranchOp::Gt,
        0x37 => BranchOp::Le,
        0x38 => BranchOp::Eqz,
        0x39 => BranchOp::Nez,
        0x3a => BranchOp::Ltz,
        0x3b => BranchOp::Gez,
        0x3c => BranchOp::Gtz,
        0x3d => BranchOp::Lez,
        _ => return Err(internal("branch opcode is not checked")),
    })
}
