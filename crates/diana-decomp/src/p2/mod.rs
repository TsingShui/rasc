//! P2 execution model: operations, normal/exception control flow and coverage.

mod build;
mod check;
mod oracle;

use std::ops::Range;

use crate::p1::DecodedMethod;
use crate::raw::{OperandReference, ReferenceSlot};
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{CodeUnitOffset, FieldId, MethodHandleId, ProtoId, RunContext, StringId, TypeId};

pub use build::{FlowError, build_flow};

/// Identity of one operation in this method product.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpId(pub u32);
/// Identity of one basic block in this method product.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(pub u32);
/// Identity of one CFG edge. Equal source/target pairs never share an ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId(pub u32);
/// Identity of one abrupt method exit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExitId(pub u32);

/// A contiguous span in the method's slot arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotSpan {
    start: u32,
    len: u32,
}

impl SlotSpan {
    pub(crate) fn new(start: u32, len: u32) -> Self {
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

#[derive(Clone, Debug)]
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
        result: Option<SlotWidth>,
    },
    Monitor {
        enter: bool,
    },
    MoveException,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects(u16);

impl Effects {
    pub const HEAP_READ: Self = Self(1 << 0);
    pub const HEAP_WRITE: Self = Self(1 << 1);
    pub const CALL: Self = Self(1 << 2);
    pub const MAY_THROW: Self = Self(1 << 3);
    pub const MONITOR: Self = Self(1 << 4);
    pub const CLASS_INIT: Self = Self(1 << 5);
    pub const ALLOCATION: Self = Self(1 << 6);

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
pub enum CommitRule {
    None,
    AtOperation,
    AfterResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExceptionSelection {
    Operation,
    Advanced,
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
    pub id: EdgeId,
    pub from: BlockId,
    pub source_op: Option<OpId>,
    pub role: EdgeRole,
    pub target: EdgeTarget,
}

#[derive(Clone, Debug)]
pub struct Operation {
    pub id: OpId,
    pub block: BlockId,
    pub pc: CodeUnitOffset,
    pub result_pc: Option<CodeUnitOffset>,
    pub kind: OpKind,
    reads: SlotSpan,
    writes: SlotSpan,
    pub effects: Effects,
    pub commit: CommitRule,
    pub exception_selection: ExceptionSelection,
}

impl Operation {
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
    pub id: BlockId,
    pub start_pc: CodeUnitOffset,
    pub operations: Range<u32>,
    pub terminal: Option<Terminal>,
    pub terminal_pc: Option<CodeUnitOffset>,
    terminal_reads: SlotSpan,
}

impl Block {
    pub fn terminal_reads(&self) -> SlotSpan {
        self.terminal_reads
    }
    pub(crate) fn set_terminal_reads(&mut self, span: SlotSpan) {
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
    pub id: ExitId,
    pub block: BlockId,
    pub pc: CodeUnitOffset,
    pub kind: ExitKind,
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
    decoded: DecodedMethod<'a>,
    slots: Vec<RegisterSlot>,
    operations: Vec<Operation>,
    blocks: Vec<Block>,
    edges: Vec<Edge>,
    exits: Vec<Exit>,
    coverage: Vec<Coverage>,
}

impl<'a> FlowMethod<'a> {
    pub fn decoded(&self) -> &DecodedMethod<'a> {
        &self.decoded
    }
    pub fn entry(&self) -> &crate::p1::EntryLayout {
        self.decoded.entry()
    }
    pub fn slots(&self) -> &[RegisterSlot] {
        &self.slots
    }
    pub fn reads(&self, operation: &Operation) -> &[RegisterSlot] {
        &self.slots
            [operation.reads.start as usize..(operation.reads.start + operation.reads.len) as usize]
    }
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

impl FlowMethod<'_> {
    pub fn reference_fact(
        &self,
        pc: CodeUnitOffset,
        slot: ReferenceSlot,
    ) -> Option<&crate::p1::CheckedReferenceFact<'_>> {
        self.decoded.reference_fact(pc, slot)
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

pub fn analyze<'a>(
    decoded: DecodedMethod<'a>,
    control: &mut RunContext<'_>,
) -> Result<FlowMethod<'a>, FlowError> {
    build_flow(decoded, control)
}
