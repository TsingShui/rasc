use std::ops::Range;

use crate::p1::DecodedMethod;
use crate::raw::{OperandReference, ReferenceSlot};
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{CodeUnitOffset, FieldId, MethodHandleId, ProtoId, StringId, TypeId};

use super::transfer::*;
use super::{FlowError, MethodEvidence};

static INVALID_OPERATION_INPUTS: crate::p1::MethodError = crate::p1::MethodError::InternalInvariant(
    "operation inputs do not belong to this execution product",
);

/// A readable return category survives an unavailable argument grouping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvokeResult {
    Void,
    Value(SlotWidth),
    Unavailable,
}
impl From<Option<SlotWidth>> for InvokeResult {
    fn from(value: Option<SlotWidth>) -> Self {
        match value {
            Some(width) => Self::Value(width),
            None => Self::Void,
        }
    }
}

/// Identity of one operation in this method product.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct OpId(pub u32);
/// Identity of one basic block in this method product.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct BlockId(pub u32);
/// Identity of one CFG edge. Equal source/target pairs never share an ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct EdgeId(pub u32);
/// Identity of one abrupt method exit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExitId(pub u32);

/// A contiguous span in the method's slot arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotSpan {
    pub(super) start: u32,
    pub(super) len: u32,
}

impl OpKind {
    /// Whether the result origin is established independently of argument grouping.
    /// Consumers must separately check that the encoded destination is in-frame.
    pub fn output_is_established(&self) -> bool {
        !matches!(
            self,
            Self::UnresolvedInvoke {
                result: InvokeResult::Unavailable,
                ..
            } | Self::UnresolvedResult { .. }
        )
    }
}

impl SlotSpan {
    pub(super) fn new(start: u32, len: u32) -> Self {
        Self { start, len }
    }
    pub fn len(self) -> u32 {
        self.len
    }
    pub fn is_empty(self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveKind {
    Word,
    Wide,
    Object,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueKind {
    Word,
    Wide,
    Object,
    Boolean,
    Byte,
    Char,
    Short,
}

impl ValueKind {
    pub fn width(self) -> SlotWidth {
        match self {
            Self::Word | Self::Object | Self::Boolean | Self::Byte | Self::Char | Self::Short => {
                SlotWidth::One
            }
            Self::Wide => SlotWidth::Two,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    NegInt,
    NotInt,
    NegLong,
    NotLong,
    NegFloat,
    NegDouble,
    IntToLong,
    IntToFloat,
    IntToDouble,
    LongToInt,
    LongToFloat,
    LongToDouble,
    FloatToInt,
    FloatToLong,
    FloatToDouble,
    DoubleToInt,
    DoubleToLong,
    DoubleToFloat,
    IntToByte,
    IntToChar,
    IntToShort,
}

impl UnaryOp {
    pub fn source_width(self) -> SlotWidth {
        match self {
            Self::NegLong
            | Self::NegDouble
            | Self::NotLong
            | Self::LongToInt
            | Self::LongToFloat
            | Self::LongToDouble
            | Self::DoubleToInt
            | Self::DoubleToLong
            | Self::DoubleToFloat => SlotWidth::Two,
            _ => SlotWidth::One,
        }
    }
    pub fn dest_width(self) -> SlotWidth {
        match self {
            Self::NegLong
            | Self::NotLong
            | Self::NegDouble
            | Self::IntToLong
            | Self::IntToDouble
            | Self::FloatToLong
            | Self::FloatToDouble
            | Self::LongToDouble
            | Self::DoubleToLong => SlotWidth::Two,
            _ => SlotWidth::One,
        }
    }
    pub fn conversion(self) -> bool {
        matches!(
            self,
            Self::IntToLong
                | Self::IntToFloat
                | Self::IntToDouble
                | Self::LongToInt
                | Self::LongToFloat
                | Self::LongToDouble
                | Self::FloatToInt
                | Self::FloatToLong
                | Self::FloatToDouble
                | Self::DoubleToInt
                | Self::DoubleToLong
                | Self::DoubleToFloat
                | Self::IntToByte
                | Self::IntToChar
                | Self::IntToShort
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    AddInt,
    RsubInt,
    SubInt,
    MulInt,
    DivInt,
    RemInt,
    AndInt,
    OrInt,
    XorInt,
    ShlInt,
    ShrInt,
    UshrInt,
    AddLong,
    SubLong,
    MulLong,
    DivLong,
    RemLong,
    AndLong,
    OrLong,
    XorLong,
    ShlLong,
    ShrLong,
    UshrLong,
    AddFloat,
    SubFloat,
    MulFloat,
    DivFloat,
    RemFloat,
    AddDouble,
    SubDouble,
    MulDouble,
    DivDouble,
    RemDouble,
}

impl BinaryOp {
    pub fn width(self) -> SlotWidth {
        match self {
            Self::AddLong
            | Self::SubLong
            | Self::MulLong
            | Self::DivLong
            | Self::RemLong
            | Self::AndLong
            | Self::OrLong
            | Self::XorLong
            | Self::ShlLong
            | Self::ShrLong
            | Self::UshrLong
            | Self::AddDouble
            | Self::SubDouble
            | Self::MulDouble
            | Self::DivDouble
            | Self::RemDouble => SlotWidth::Two,
            _ => SlotWidth::One,
        }
    }
    pub fn left_width(self) -> SlotWidth {
        self.width()
    }
    pub fn right_width(self) -> SlotWidth {
        match self {
            Self::ShlLong | Self::ShrLong | Self::UshrLong => SlotWidth::One,
            _ => self.width(),
        }
    }
    pub fn integer(self) -> bool {
        matches!(
            self,
            Self::RsubInt
                | Self::AddInt
                | Self::SubInt
                | Self::MulInt
                | Self::DivInt
                | Self::RemInt
                | Self::AndInt
                | Self::OrInt
                | Self::XorInt
                | Self::ShlInt
                | Self::ShrInt
                | Self::UshrInt
                | Self::AddLong
                | Self::SubLong
                | Self::MulLong
                | Self::DivLong
                | Self::RemLong
                | Self::AndLong
                | Self::OrLong
                | Self::XorLong
                | Self::ShlLong
                | Self::ShrLong
                | Self::UshrLong
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryForm {
    ThreeAddress,
    TwoAddress,
    Literal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompareOp {
    CmplFloat,
    CmpgFloat,
    CmplDouble,
    CmpgDouble,
    CmpLong,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchOp {
    Eq,
    Ne,
    Lt,
    Ge,
    Gt,
    Le,
    Eqz,
    Nez,
    Ltz,
    Gez,
    Gtz,
    Lez,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwitchKind {
    Packed,
    Sparse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldAccess {
    Get,
    Put,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayAccess {
    Get,
    Put,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvokeKind {
    Virtual,
    Super,
    Direct,
    Static,
    Interface,
    Polymorphic,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpKind {
    Nop,
    Move {
        kind: MoveKind,
    },
    Const {
        bits: u64,
        width: SlotWidth,
    },
    ConstString {
        value: StringId,
    },
    ConstClass {
        type_id: TypeId,
    },
    ConstMethodHandle {
        handle: MethodHandleId,
    },
    ConstMethodType {
        proto: ProtoId,
    },
    Unary {
        op: UnaryOp,
    },
    Binary {
        op: BinaryOp,
        form: BinaryForm,
        literal: Option<i64>,
    },
    Compare {
        op: CompareOp,
    },
    Field {
        access: FieldAccess,
        static_field: bool,
        field: FieldId,
        kind: ValueKind,
    },
    Array {
        access: ArrayAccess,
        kind: ValueKind,
    },
    ArrayLength,
    Cast {
        type_id: TypeId,
    },
    InstanceOf {
        type_id: TypeId,
    },
    NewInstance {
        type_id: TypeId,
    },
    NewArray {
        type_id: TypeId,
    },
    FilledNewArray {
        type_id: TypeId,
    },
    FillArrayData {
        payload: CodeUnitOffset,
    },
    Invoke {
        kind: InvokeKind,
        target: OperandReference,
        prototype: Option<ProtoId>,
        result: Option<SlotWidth>,
    },
    /// Dispatch/reference identities and effects are known, argument grouping is not.
    UnresolvedInvoke {
        kind: InvokeKind,
        target: OperandReference,
        prototype: Option<ProtoId>,
        result: InvokeResult,
    },
    /// The encoded destination is known, but no valid producer result is established.
    UnresolvedResult {
        width: SlotWidth,
    },
    Monitor {
        enter: bool,
    },
    MoveException,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects(u16);

impl Effects {
    pub(super) const HEAP_READ: Self = Self(1 << 0);
    pub(super) const HEAP_WRITE: Self = Self(1 << 1);
    pub(super) const CALL: Self = Self(1 << 2);
    pub(super) const MAY_THROW: Self = Self(1 << 3);
    pub(super) const MONITOR: Self = Self(1 << 4);
    pub(super) const CLASS_INIT: Self = Self(1 << 5);
    pub(super) const ALLOCATION: Self = Self(1 << 6);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn may_throw(self) -> bool {
        self.contains(Self::MAY_THROW)
    }
    pub(crate) fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalEdge {
    Fallthrough,
    Goto,
    ConditionalTrue,
    ConditionalFalse,
    SwitchCase { key: i64, index: u32 },
    SwitchDefault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeRole {
    Normal(NormalEdge),
    Exception {
        throw_pc: CodeUnitOffset,
        selection_pc: CodeUnitOffset,
        order: u32,
        catch: Option<TypeId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeTarget {
    Block(BlockId),
    Exit(ExitId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    pub(super) id: EdgeId,
    pub(super) from: BlockId,
    pub(super) source_op: Option<OpId>,
    pub(super) role: EdgeRole,
    pub(super) target: EdgeTarget,
}

impl Edge {
    pub fn id(&self) -> EdgeId {
        self.id
    }
    pub fn from(&self) -> BlockId {
        self.from
    }
    pub fn source_op(&self) -> Option<OpId> {
        self.source_op
    }
    pub fn role(&self) -> EdgeRole {
        self.role
    }
    pub fn target(&self) -> EdgeTarget {
        self.target
    }
}

#[derive(Clone, Debug)]
pub struct Operation {
    pub(super) id: OpId,
    pub(super) block: BlockId,
    pub(super) pc: CodeUnitOffset,
    pub(super) result_pc: Option<CodeUnitOffset>,
    pub(super) kind: OpKind,
    pub(super) reads: SlotSpan,
    pub(super) writes: SlotSpan,
    pub(super) effects: Effects,
    pub(super) normal: NormalCompletion,
    pub(super) exceptional: Option<ExceptionCompletion>,
}

impl Operation {
    pub fn id(&self) -> OpId {
        self.id
    }
    pub fn block(&self) -> BlockId {
        self.block
    }
    pub fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub fn result_pc(&self) -> Option<CodeUnitOffset> {
        self.result_pc
    }
    pub fn kind(&self) -> &OpKind {
        &self.kind
    }
    pub fn effects(&self) -> Effects {
        self.effects
    }
    pub fn normal(&self) -> NormalCompletion {
        self.normal
    }
    pub fn exceptional(&self) -> Option<ExceptionCompletion> {
        self.exceptional
    }
    pub fn reads(&self) -> SlotSpan {
        self.reads
    }
    pub fn writes(&self) -> SlotSpan {
        self.writes
    }
}

#[derive(Clone, Debug)]
pub enum Terminal {
    Goto {
        target: CodeUnitOffset,
    },
    Branch {
        op: BranchOp,
        target: CodeUnitOffset,
        fallthrough: CodeUnitOffset,
    },
    Switch {
        kind: SwitchKind,
        payload: CodeUnitOffset,
        default: CodeUnitOffset,
        cases: Vec<(i64, CodeUnitOffset)>,
    },
    Return {
        width: Option<SlotWidth>,
        pc: CodeUnitOffset,
    },
    Throw {
        pc: CodeUnitOffset,
    },
}

#[derive(Clone, Debug)]
pub struct Block {
    pub(super) id: BlockId,
    pub(super) start_pc: CodeUnitOffset,
    pub(super) operations: Range<u32>,
    pub(super) terminal: Option<Terminal>,
    pub(super) terminal_pc: Option<CodeUnitOffset>,
    pub(super) exceptional: Option<ExceptionCompletion>,
    pub(super) terminal_reads: SlotSpan,
}

impl Block {
    pub fn id(&self) -> BlockId {
        self.id
    }
    pub fn start_pc(&self) -> CodeUnitOffset {
        self.start_pc
    }
    pub fn operations(&self) -> Range<u32> {
        self.operations.clone()
    }
    pub fn terminal(&self) -> Option<&Terminal> {
        self.terminal.as_ref()
    }
    pub fn terminal_pc(&self) -> Option<CodeUnitOffset> {
        self.terminal_pc
    }
    pub fn exceptional(&self) -> Option<ExceptionCompletion> {
        self.exceptional
    }
    pub fn terminal_reads(&self) -> SlotSpan {
        self.terminal_reads
    }
    pub(super) fn set_terminal_reads(&mut self, span: SlotSpan) {
        self.terminal_reads = span;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitKind {
    Return,
    Uncaught,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exit {
    pub(super) id: ExitId,
    pub(super) block: BlockId,
    pub(super) pc: CodeUnitOffset,
    pub(super) kind: ExitKind,
}

impl Exit {
    pub fn id(&self) -> ExitId {
        self.id
    }
    pub fn block(&self) -> BlockId {
        self.block
    }
    pub fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub fn kind(&self) -> ExitKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coverage {
    Operation(OpId),
    Terminal(BlockId),
    MergedResult(OpId),
    Payload,
    Unreachable,
}

/// The complete P2 product. It owns the P1 product it was built from.
pub struct FlowMethod<'a> {
    pub(super) decoded: DecodedMethod<'a>,
    pub(super) slots: Vec<RegisterSlot>,
    pub(super) operations: Vec<Operation>,
    pub(super) blocks: Vec<Block>,
    pub(super) edges: Vec<Edge>,
    pub(super) exits: Vec<Exit>,
    pub(super) coverage: Vec<Coverage>,
}

impl<'a> FlowMethod<'a> {
    #[cfg(test)]
    pub(super) fn decoded(&self) -> &DecodedMethod<'a> {
        &self.decoded
    }
    /// Original check/debug evidence, not an upstream analysis entry.
    pub fn evidence(&self) -> MethodEvidence<'_, 'a> {
        MethodEvidence::new(&self.decoded)
    }
    pub fn method_id(&self) -> crate::MethodId {
        self.decoded.method_id()
    }
    pub fn class_id(&self) -> crate::ClassDefId {
        self.decoded.class_id()
    }
    pub fn owner_facts(&self) -> &crate::p0::OwnerFacts {
        self.decoded.owner_facts()
    }
    pub fn declaration(&self) -> &crate::p0::MethodDeclaration {
        self.decoded.declaration()
    }
    pub fn access_flags(&self) -> u32 {
        self.decoded.access_flags()
    }
    pub fn entry(&self) -> &crate::p1::EntryLayout {
        self.decoded.entry()
    }
    pub fn slots(&self) -> &[RegisterSlot] {
        &self.slots
    }
    pub fn reads(&self, operation: &Operation) -> Result<&[RegisterSlot], &crate::p1::MethodError> {
        if matches!(operation.kind, OpKind::UnresolvedInvoke { .. }) {
            if let Some(Err(issue)) = self.decoded.invoke_layout_at(operation.pc) {
                return Err(issue);
            }
            return Err(&INVALID_OPERATION_INPUTS);
        }
        let end = operation
            .reads
            .start
            .checked_add(operation.reads.len)
            .ok_or(&INVALID_OPERATION_INPUTS)?;
        self.slots
            .get(operation.reads.start as usize..end as usize)
            .ok_or(&INVALID_OPERATION_INPUTS)
    }
    /// Original argument words, not inferred One/Two parameter slots.
    pub fn input_words(
        &self,
        operation: &Operation,
    ) -> Option<impl Iterator<Item = crate::RegisterId> + '_> {
        if !matches!(
            operation.kind,
            OpKind::Invoke { .. } | OpKind::UnresolvedInvoke { .. }
        ) {
            return None;
        }
        self.decoded
            .instruction_at(operation.pc)
            .map(|i| i.registers())
    }
    pub fn slot_is_valid(&self, slot: RegisterSlot) -> bool {
        u64::from(slot.start.0) + u64::from(slot.words()) <= self.decoded.registers_size().0
    }
    pub fn issues(&self) -> impl Iterator<Item = (CodeUnitOffset, &crate::p1::MethodError)> + '_ {
        self.decoded.issues()
    }
    /// Encoded destination intervals, also bounding unresolved possible writes.
    /// These are not established values when the result or slot is unavailable.
    pub fn writes(&self, operation: &Operation) -> &[RegisterSlot] {
        &self.slots[operation.writes.start as usize
            ..(operation.writes.start + operation.writes.len) as usize]
    }
    pub fn terminal_reads(&self, block: &Block) -> &[RegisterSlot] {
        &self.slots[block.terminal_reads.start as usize
            ..(block.terminal_reads.start + block.terminal_reads.len) as usize]
    }
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    pub fn operation(&self, id: OpId) -> Option<&Operation> {
        self.operations.get(id.0 as usize)
    }
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.get(id.0 as usize)
    }
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
    pub fn normal_edges(&self) -> impl Iterator<Item = &Edge> + '_ {
        self.edges
            .iter()
            .filter(|edge| matches!(edge.role, EdgeRole::Normal(_)))
    }
    pub fn exception_edges(&self) -> impl Iterator<Item = &Edge> + '_ {
        self.edges
            .iter()
            .filter(|edge| matches!(edge.role, EdgeRole::Exception { .. }))
    }
    pub fn exits(&self) -> &[Exit] {
        &self.exits
    }
    pub fn coverage(&self) -> &[Coverage] {
        &self.coverage
    }
    pub fn coverage_at(&self, pc: CodeUnitOffset) -> Option<Coverage> {
        self.decoded
            .items()
            .iter()
            .position(|item| item.pc() == pc)
            .map(|index| self.coverage[index])
    }
}

impl<'a> FlowMethod<'a> {
    pub fn reference_fact(
        &self,
        pc: CodeUnitOffset,
        slot: ReferenceSlot,
    ) -> Option<Result<&crate::p1::CheckedReferenceFact<'a>, &crate::p1::MethodError>> {
        self.decoded.reference_fact(pc, slot)
    }
    pub fn reference_use_issue(
        &self,
        pc: CodeUnitOffset,
        slot: ReferenceSlot,
    ) -> Option<&crate::p1::MethodError> {
        self.decoded.reference_use_issue(pc, slot)
    }
}

impl std::fmt::Debug for FlowMethod<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FlowMethod")
            .field("blocks", &self.blocks.len())
            .field("operations", &self.operations.len())
            .field("edges", &self.edges.len())
            .field("exits", &self.exits.len())
            .finish()
    }
}

/// Classified P2 stop with the last complete input retained. It cannot be
/// consumed as a FlowMethod or upgraded by the scheduler.
pub struct FlowStop<'a> {
    pub(super) input: DecodedMethod<'a>,
    pub(super) error: FlowError,
    pub(super) partial: Option<FlowPartial>,
}

/// Unvalidated construction snapshot retained on stop. This is not executable
/// FlowMethod input and cannot be promoted by a consumer.
pub struct FlowPartial {
    pub(super) slots: Vec<RegisterSlot>,
    pub(super) operations: Vec<Operation>,
    pub(super) blocks: Vec<Block>,
    pub(super) edges: Vec<Edge>,
    pub(super) exits: Vec<Exit>,
    pub(super) coverage: Vec<Coverage>,
}
impl FlowPartial {
    pub fn slots(&self) -> &[RegisterSlot] {
        &self.slots
    }
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
    pub fn exits(&self) -> &[Exit] {
        &self.exits
    }
    pub fn coverage(&self) -> &[Coverage] {
        &self.coverage
    }
}
impl FlowStop<'_> {
    pub fn evidence(&self) -> MethodEvidence<'_, '_> {
        MethodEvidence::new(&self.input)
    }
    pub fn error(&self) -> &FlowError {
        &self.error
    }
    pub fn partial(&self) -> Option<&FlowPartial> {
        self.partial.as_ref()
    }
}
