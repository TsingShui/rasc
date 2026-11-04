use std::collections::TryReserveError;
use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use super::{
    ArrayAccess, BinaryForm, BinaryOp, Block, BlockId, BranchOp, CommitRule, CompareOp, Coverage,
    Edge, EdgeId, EdgeRole, EdgeTarget, Effects, ExceptionSelection, Exit, ExitId, ExitKind,
    FieldAccess, FlowMethod, InvokeKind, MoveKind, NormalEdge, OpId, OpKind, Operation, SlotSpan,
    SwitchKind, Terminal, UnaryOp, ValueKind,
};
use crate::input::formats::FlowKind;
use crate::p1::{DecodedInstruction, DecodedItem, DecodedMethod};
use crate::raw::OperandReference;
use crate::raw::RawLiteral;
use crate::substrate::owned::push;
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{
    CodeUnitOffset, ControlError, FailureKind, RegisterId, RequestError, RunContext, TypeId,
};

#[derive(Debug, Error)]
pub enum FlowError {
    #[error(transparent)]
    Control(#[from] ControlError),
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
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}

impl FlowError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Control(error) => error.kind(),
            Self::Allocation(_) | Self::PlatformCapacity => FailureKind::ResourceExhausted,
            Self::Unsupported { .. } => FailureKind::Unsupported,
            Self::InternalInvariant(_) => FailureKind::InternalInvariant,
        }
    }
}

pub(crate) fn request_error(error: RequestError) -> FlowError {
    match error {
        RequestError::Control(error) => FlowError::Control(error),
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
    instruction_by_pc: BTreeMap<CodeUnitOffset, usize>,
    item_by_pc: BTreeMap<CodeUnitOffset, usize>,
    atoms: Vec<Atom>,
    atom_by_pc: BTreeMap<CodeUnitOffset, usize>,
    slots: Vec<RegisterSlot>,
    operations: Vec<Operation>,
    blocks: Vec<Block>,
    block_by_pc: BTreeMap<CodeUnitOffset, BlockId>,
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

pub fn build_flow<'a>(
    decoded: DecodedMethod<'a>,
    control: &mut RunContext<'_>,
) -> Result<FlowMethod<'a>, FlowError> {
    let parts = {
        let mut builder = Builder::new(&decoded)?;
        builder.build(control)?;
        builder.finish()
    };
    let flow = FlowMethod {
        decoded,
        slots: parts.slots,
        operations: parts.operations,
        blocks: parts.blocks,
        edges: parts.edges,
        exits: parts.exits,
        coverage: parts.coverage,
    };
    super::check::verify(&flow, control)?;
    Ok(flow)
}

impl<'a, 'd> Builder<'a, 'd> {
    fn new(decoded: &'d DecodedMethod<'a>) -> Result<Self, FlowError> {
        let instructions: Vec<&DecodedInstruction> = decoded.instructions().collect();
        let mut instruction_by_pc = BTreeMap::new();
        for (index, instruction) in instructions.iter().enumerate() {
            instruction_by_pc.insert(instruction.pc(), index);
        }
        let mut item_by_pc = BTreeMap::new();
        for (index, item) in decoded.items().iter().enumerate() {
            item_by_pc.insert(item.pc(), index);
        }
        let coverage = decoded
            .items()
            .iter()
            .map(|item| match item {
                DecodedItem::Instruction(_) => Coverage::Unreachable,
                DecodedItem::Payload(_) => Coverage::Payload,
            })
            .collect();
        Ok(Self {
            decoded,
            instructions,
            instruction_by_pc,
            item_by_pc,
            atoms: Vec::new(),
            atom_by_pc: BTreeMap::new(),
            slots: Vec::new(),
            operations: Vec::new(),
            blocks: Vec::new(),
            block_by_pc: BTreeMap::new(),
            edges: Vec::new(),
            exits: Vec::new(),
            coverage,
        })
    }

    fn build(&mut self, control: &mut RunContext<'_>) -> Result<(), FlowError> {
        self.build_atoms()?;
        let leaders = self.leaders(control)?;
        self.build_blocks(&leaders, control)?;
        self.build_edges(control)?;
        Ok(())
    }

    fn build_atoms(&mut self) -> Result<(), FlowError> {
        for (index, instruction) in self.instructions.iter().enumerate() {
            if matches!(instruction.opcode(), 0x0a..=0x0c) {
                let producer_pc = instruction
                    .producer_pc()
                    .ok_or(internal("move-result has no checked producer"))?;
                let producer = *self
                    .instruction_by_pc
                    .get(&producer_pc)
                    .ok_or(internal("move-result producer is not an instruction"))?;
                let atom = self
                    .atom_by_pc
                    .get(&producer_pc)
                    .copied()
                    .ok_or(internal("move-result producer has no atom"))?;
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
            let atom = self.atoms.len();
            self.atoms.push(Atom {
                producer: index,
                result: None,
            });
            self.atom_by_pc.insert(instruction.pc(), atom);
        }
        for atom in &self.atoms {
            let producer = self.instructions[atom.producer];
            if producer.result_pc().is_some() != atom.result.is_some() {
                return Err(internal("P1 producer/result association is incomplete"));
            }
        }
        Ok(())
    }

    fn leaders(
        &mut self,
        control: &mut RunContext<'_>,
    ) -> Result<BTreeSet<CodeUnitOffset>, FlowError> {
        let mut leaders = BTreeSet::new();
        if self.decoded.is_reachable(CodeUnitOffset(0)) {
            leaders.insert(CodeUnitOffset(0));
        }
        for target in self.decoded.handler_targets() {
            let target = CodeUnitOffset(target);
            if self.decoded.is_reachable(target) {
                leaders.insert(target);
            }
        }
        for instruction in &self.instructions {
            let pc = instruction.pc();
            if !self.decoded.is_reachable(pc) {
                continue;
            }
            control.checkpoint()?;
            match instruction.spec().flow {
                FlowKind::Goto | FlowKind::Conditional => {
                    if let Some(target) = instruction.branch_target()
                        && self.decoded.is_reachable(target)
                    {
                        leaders.insert(target);
                    }
                }
                FlowKind::PackedSwitch | FlowKind::SparseSwitch => {
                    for (_, target) in self.switch_cases(instruction, control)? {
                        if self.decoded.is_reachable(target) {
                            leaders.insert(target);
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
                leaders.insert(next);
            }
        }
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

    fn build_blocks(
        &mut self,
        leaders: &BTreeSet<CodeUnitOffset>,
        control: &mut RunContext<'_>,
    ) -> Result<(), FlowError> {
        let mut current: Option<usize> = None;
        for atom_index in 0..self.atoms.len() {
            let atom = self.atoms[atom_index];
            let producer = self.instructions[atom.producer];
            let pc = producer.pc();
            if !self.decoded.is_reachable(pc) {
                continue;
            }
            control.checkpoint()?;
            let starts_block = leaders.contains(&pc) || current.is_none();
            if starts_block {
                if self.block_by_pc.contains_key(&pc) {
                    return Err(internal("two blocks start at the same pc"));
                }
                let block_index = self.blocks.len();
                let block_id =
                    BlockId(u32::try_from(block_index).map_err(|_| FlowError::PlatformCapacity)?);
                let operation_start = u32::try_from(self.operations.len())
                    .map_err(|_| FlowError::PlatformCapacity)?;
                self.blocks.push(Block {
                    id: block_id,
                    start_pc: pc,
                    operations: operation_start..operation_start,
                    terminal: None,
                    terminal_pc: None,
                    terminal_reads: SlotSpan::new(0, 0),
                });
                self.block_by_pc.insert(pc, block_id);
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
                let (terminal, reads) = self.terminal(&atom, producer, control)?;
                let block_id = self.blocks[block_index].id;
                self.blocks[block_index].terminal = Some(terminal);
                self.blocks[block_index].terminal_pc = Some(pc);
                self.blocks[block_index].set_terminal_reads(reads);
                let item = self.item_index(pc)?;
                self.coverage[item] = Coverage::Terminal(block_id);
            } else {
                let operation = self.operation(&atom, producer, block_index, control)?;
                let id = operation.id;
                let block_id = operation.block;
                let start = self.blocks[block_index].operations.start;
                self.operations.push(operation);
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
                let _ = block_id;
            }
            let _ = atom_index;
        }
        // After a terminal or before a leader, the next atom must start a block.
        Ok(())
    }

    fn operation(
        &mut self,
        atom: &Atom,
        instruction: &DecodedInstruction,
        block_index: usize,
        control: &mut RunContext<'_>,
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
                let reads = self.slots_for([RegisterSlot::new(source, width)], control)?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)], control)?;
                (
                    OpKind::Move { kind },
                    reads,
                    writes,
                    Effects::default(),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x0d => {
                let dest = self.register(instruction, 0)?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let writes = self.slots_for([RegisterSlot::new(dest, width)], control)?;
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
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
                (
                    OpKind::ConstClass { type_id },
                    self.empty_span(),
                    writes,
                    Effects::MAY_THROW.with(Effects::CLASS_INIT),
                    CommitRule::AtOperation,
                    None,
                )
            }
            0x1d | 0x1e => {
                let reg = self.register(instruction, 0)?;
                let reads = self.slots_for([RegisterSlot::new(reg, SlotWidth::One)], control)?;
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
                let reads = self.slots_for([RegisterSlot::new(reg, SlotWidth::One)], control)?;
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
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)], control)?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)], control)?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let reads = self.slots_for([RegisterSlot::new(size, SlotWidth::One)], control)?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let reads = self.all_register_slots(instruction, SlotWidth::One, control)?;
                (
                    OpKind::FilledNewArray { type_id },
                    reads,
                    self.empty_span(),
                    Effects::MAY_THROW.with(Effects::ALLOCATION),
                    CommitRule::None,
                    None,
                )
            }
            0x26 => {
                let array = self.register(instruction, 0)?;
                let payload = instruction
                    .payload_target()
                    .ok_or(internal("fill-array-data has no checked payload"))?;
                let reads = self.slots_for([RegisterSlot::new(array, SlotWidth::One)], control)?;
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
                let reads = self.slots_for(
                    [
                        RegisterSlot::new(left, left_width),
                        RegisterSlot::new(right, left_width),
                    ],
                    control,
                )?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let get = opcode < 0x4c;
                let kind = self.array_kind(opcode);
                let width = kind.width();
                let mut reads = vec![
                    RegisterSlot::new(array, SlotWidth::One),
                    RegisterSlot::new(index, SlotWidth::One),
                ];
                if !get {
                    reads.insert(0, RegisterSlot::new(dest, width));
                }
                let reads = self.slots_vec(reads, control)?;
                let writes = if get {
                    self.slots_for([RegisterSlot::new(dest, width)], control)?
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
                    Effects::MAY_THROW
                        .with(Effects::HEAP_READ)
                        .with(Effects::HEAP_WRITE),
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
                    reads.push(RegisterSlot::new(
                        self.register(instruction, 1)?,
                        SlotWidth::One,
                    ));
                }
                if !get {
                    reads.insert(0, RegisterSlot::new(dest, width));
                }
                let reads = self.slots_vec(reads, control)?;
                let writes = if get {
                    self.slots_for([RegisterSlot::new(dest, width)], control)?
                } else {
                    self.empty_span()
                };
                let mut effects = Effects::MAY_THROW
                    .with(Effects::HEAP_READ)
                    .with(Effects::HEAP_WRITE);
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
                let layout = self
                    .decoded
                    .invoke_layout_at(pc)
                    .ok_or(internal("invoke has no checked P1 layout"))?;
                let reads = self.invoke_reads(instruction, layout, control)?;
                let result = layout.result();
                (
                    OpKind::Invoke {
                        kind,
                        target,
                        result,
                    },
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
                let reads =
                    self.slots_for([RegisterSlot::new(source, op.source_width())], control)?;
                let writes = self.slots_for([RegisterSlot::new(dest, op.dest_width())], control)?;
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
                let reads = self.slots_for(
                    [
                        RegisterSlot::new(left, op.left_width()),
                        RegisterSlot::new(right, op.right_width()),
                    ],
                    control,
                )?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)], control)?;
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
                let reads = self.slots_for(
                    [
                        RegisterSlot::new(dest, op.left_width()),
                        RegisterSlot::new(right, op.right_width()),
                    ],
                    control,
                )?;
                let writes = self.slots_for([RegisterSlot::new(dest, width)], control)?;
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
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)], control)?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let reads = self.slots_for([RegisterSlot::new(source, SlotWidth::One)], control)?;
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
                let writes = self.slots_for([RegisterSlot::new(dest, SlotWidth::One)], control)?;
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
            writes = self.slots_for([RegisterSlot::new(dest, width)], control)?;
        }
        let commit = if atom.result.is_some() {
            CommitRule::AfterResult
        } else if writes.is_empty() {
            CommitRule::None
        } else {
            CommitRule::AtOperation
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
            commit,
            exception_selection: if opcode == 0x1e {
                ExceptionSelection::Advanced
            } else {
                ExceptionSelection::Operation
            },
        })
    }

    fn terminal(
        &mut self,
        _atom: &Atom,
        instruction: &DecodedInstruction,
        control: &mut RunContext<'_>,
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
                    slots.push(RegisterSlot::new(
                        self.register(instruction, index)?,
                        SlotWidth::One,
                    ));
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
                    self.slots_vec(slots, control)?,
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
                let cases = self.switch_cases(instruction, control)?;
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
                    self.slots_for([RegisterSlot::new(selector, SlotWidth::One)], control)?,
                ))
            }
            FlowKind::Return => {
                let width = match instruction.opcode() {
                    0x0e => None,
                    0x10 => Some(SlotWidth::Two),
                    _ => Some(SlotWidth::One),
                };
                let reads = match width {
                    Some(width) => self.slots_for(
                        [RegisterSlot::new(self.register(instruction, 0)?, width)],
                        control,
                    )?,
                    None => self.empty_span(),
                };
                Ok((Terminal::Return { width, pc }, reads))
            }
            FlowKind::Throw => {
                let reads = self.slots_for(
                    [RegisterSlot::new(
                        self.register(instruction, 0)?,
                        SlotWidth::One,
                    )],
                    control,
                )?;
                Ok((Terminal::Throw { pc }, reads))
            }
            _ => Err(internal(
                "non-terminal instruction entered terminal builder",
            )),
        }
    }

    fn build_edges(&mut self, control: &mut RunContext<'_>) -> Result<(), FlowError> {
        let block_count = self.blocks.len();
        for index in 0..block_count {
            control.checkpoint()?;
            let block = self.blocks[index].clone();
            if let Some(terminal) = &block.terminal {
                match terminal {
                    Terminal::Goto { target } => {
                        let target = self.block_at_pc(*target)?;
                        self.normal_edge(block.id, NormalEdge::Goto, target)?;
                    }
                    Terminal::Branch {
                        target,
                        fallthrough,
                        ..
                    } => {
                        let target_block = self.block_at_pc(*target)?;
                        self.normal_edge(block.id, NormalEdge::ConditionalTrue, target_block)?;
                        let fallthrough = self.block_at_pc(*fallthrough)?;
                        self.normal_edge(block.id, NormalEdge::ConditionalFalse, fallthrough)?;
                    }
                    Terminal::Switch { default, cases, .. } => {
                        let default_block = self.block_at_pc(*default)?;
                        self.normal_edge(block.id, NormalEdge::SwitchDefault, default_block)?;
                        for (index, (key, target)) in cases.iter().enumerate() {
                            let target_block = self.block_at_pc(*target)?;
                            self.normal_edge(
                                block.id,
                                NormalEdge::SwitchCase {
                                    key: *key,
                                    index: u32::try_from(index)
                                        .map_err(|_| FlowError::PlatformCapacity)?,
                                },
                                target_block,
                            )?;
                        }
                    }
                    Terminal::Return { pc, .. } => {
                        self.exit(block.id, *pc, ExitKind::Return)?;
                    }
                    Terminal::Throw { pc } => {
                        self.exception_edges(block.id, None, *pc, *pc, control)?;
                    }
                }
                continue;
            }
            let next = self.next_block(&block)?;
            self.normal_edge(block.id, NormalEdge::Fallthrough, next)?;
        }
        // Every may-throw operation gets its own ordered exception exits.
        for block in self.blocks.clone() {
            for operation_index in block.operations.clone() {
                let operation = &self.operations[operation_index as usize];
                if !operation.effects.may_throw() {
                    continue;
                }
                let selection = match operation.exception_selection {
                    ExceptionSelection::Operation => operation.pc,
                    ExceptionSelection::Advanced => {
                        let instruction = self
                            .instruction_at(operation.pc)
                            .ok_or(internal("throwing operation has no instruction"))?;
                        operation
                            .pc
                            .checked_advance(instruction.width())
                            .ok_or(FlowError::PlatformCapacity)?
                    }
                };
                self.exception_edges(
                    block.id,
                    Some(operation.id),
                    operation.pc,
                    selection,
                    control,
                )?;
            }
        }
        Ok(())
    }

    fn exception_edges(
        &mut self,
        block: BlockId,
        source_op: Option<OpId>,
        throw_pc: CodeUnitOffset,
        selection_pc: CodeUnitOffset,
        control: &mut RunContext<'_>,
    ) -> Result<(), FlowError> {
        let clauses = self
            .decoded
            .catch_clauses_at(selection_pc, control)
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
            control.checkpoint()?;
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
        self.exits.push(Exit {
            id,
            block,
            pc,
            kind,
        });
        Ok(id)
    }

    fn push_edge(&mut self, edge: Edge) -> Result<(), FlowError> {
        self.edges.try_reserve(1).map_err(FlowError::Allocation)?;
        self.edges.push(edge);
        Ok(())
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
        self.block_by_pc
            .get(&pc)
            .copied()
            .ok_or(internal("checked target has no block"))
    }

    fn switch_cases(
        &self,
        instruction: &DecodedInstruction,
        control: &mut RunContext<'_>,
    ) -> Result<Vec<(i64, CodeUnitOffset)>, FlowError> {
        let payload_pc = instruction
            .payload_target()
            .ok_or(internal("switch has no checked payload"))?;
        let payload = self
            .decoded
            .payloads()
            .find(|payload| payload.pc() == payload_pc)
            .ok_or(internal("switch payload is not in the P1 directory"))?;
        let cases = payload
            .switch_cases(self.decoded.source())
            .map_err(request_error)?
            .ok_or(internal("switch payload has no cases"))?;
        let mut result = Vec::new();
        for case in cases {
            control.checkpoint()?;
            let target = i64::from(instruction.pc().0)
                .checked_add(i64::from(case.offset.0))
                .and_then(|value| u32::try_from(value).ok())
                .map(CodeUnitOffset)
                .ok_or(FlowError::PlatformCapacity)?;
            push(&mut result, (case.key, target), control).map_err(request_error)?;
        }
        Ok(result)
    }

    fn instruction_at(&self, pc: CodeUnitOffset) -> Option<&DecodedInstruction> {
        self.decoded.instruction_at(pc)
    }

    fn item_index(&self, pc: CodeUnitOffset) -> Result<usize, FlowError> {
        self.item_by_pc
            .get(&pc)
            .copied()
            .ok_or(internal("coverage pc is not a P1 item"))
    }

    fn empty_span(&self) -> SlotSpan {
        SlotSpan::new(u32::try_from(self.slots.len()).unwrap_or(u32::MAX), 0)
    }

    fn slots_for<const N: usize>(
        &mut self,
        slots: [RegisterSlot; N],
        control: &mut RunContext<'_>,
    ) -> Result<SlotSpan, FlowError> {
        let start = u32::try_from(self.slots.len()).map_err(|_| FlowError::PlatformCapacity)?;
        for slot in slots {
            push(&mut self.slots, slot, control).map_err(request_error)?;
        }
        let len = u32::try_from(self.slots.len())
            .map_err(|_| FlowError::PlatformCapacity)?
            .checked_sub(start)
            .ok_or(FlowError::PlatformCapacity)?;
        Ok(SlotSpan::new(start, len))
    }

    fn slots_vec(
        &mut self,
        slots: Vec<RegisterSlot>,
        control: &mut RunContext<'_>,
    ) -> Result<SlotSpan, FlowError> {
        let start = u32::try_from(self.slots.len()).map_err(|_| FlowError::PlatformCapacity)?;
        for slot in slots {
            push(&mut self.slots, slot, control).map_err(request_error)?;
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
        control: &mut RunContext<'_>,
    ) -> Result<SlotSpan, FlowError> {
        let slots: Vec<RegisterSlot> = instruction
            .registers()
            .map(|register| RegisterSlot::new(register, width))
            .collect();
        self.slots_vec(slots, control)
    }

    fn invoke_reads(
        &mut self,
        instruction: &DecodedInstruction,
        layout: &crate::p1::InvokeLayout,
        control: &mut RunContext<'_>,
    ) -> Result<SlotSpan, FlowError> {
        let mut regs = instruction.registers();
        let mut slots = Vec::new();
        if layout.has_receiver() {
            slots.push(RegisterSlot::new(
                regs.next()
                    .ok_or(internal("invoke lost its checked receiver"))?,
                SlotWidth::One,
            ));
        }
        for width in layout.arguments() {
            let register = regs
                .next()
                .ok_or(internal("invoke lost a checked argument"))?;
            slots.push(RegisterSlot::new(register, *width));
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
        self.slots_vec(slots, control)
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
