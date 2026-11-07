use super::references::SourceFailure;
use crate::p2::{BinaryOp, BranchOp, CompareOp, InvokeKind, UnaryOp, ValueKind};
use crate::p3::{PointId, StateId, UseSite};
use crate::p4::{Bits, Initialization, Interpretation};
use crate::p5::{DefinitionPlan, StorageGap, StorageRead};
use serde::Serialize;

macro_rules! ids {($($id:ident),*$(,)?)=>{$(#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd,Serialize)]#[serde(transparent)]pub struct $id(pub u32);)*};}
ids!(
    ExprId,
    StatementId,
    ReferenceId,
    TypeUseId,
    StorageId,
    CopyPlanId,
    LabelId,
    TransferId,
    ControlId,
    ConstructionId,
    PayloadId,
    SiteId
);

/// Original read evidence is not a second source for executing an unavailable use.
#[derive(Debug, Serialize)]
pub struct ReadOrigin {
    #[serde(serialize_with = "super::debug")]
    pub site: UseSite,
    #[serde(serialize_with = "super::debug")]
    pub point: PointId,
    #[serde(serialize_with = "super::debug")]
    pub state: StateId,
    #[serde(serialize_with = "super::debug")]
    pub read: Result<StorageRead, StorageGap>,
    #[serde(serialize_with = "super::debug")]
    pub initialization: Option<Initialization>,
}
#[derive(Debug, Serialize)]
pub struct Expression {
    pub id: ExprId,
    pub kind: ExprKind,
    pub origin: Option<ReadOrigin>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Arguments {
    Known(Vec<ExprId>),
    Unavailable {
        #[serde(serialize_with = "super::debug")]
        gap: StorageGap,
        words: Vec<crate::RegisterId>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Qualification {
    Natural,
    Parameters { owner: bool },
    FullSignature { owner: bool },
}
#[derive(Debug, Serialize)]
pub struct Call {
    #[serde(serialize_with = "super::debug")]
    pub kind: InvokeKind,
    pub target: ReferenceId,
    pub invocation_prototype: Option<ReferenceId>,
    pub receiver: Option<ExprId>,
    pub arguments: Arguments,
    pub qualification: Qualification,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExprKind {
    Read {
        storage: StorageId,
        #[serde(serialize_with = "super::debug")]
        view: Interpretation,
    },
    Snapshot {
        storage: StorageId,
    },
    Literal {
        #[serde(serialize_with = "super::debug")]
        bits: Bits,
        #[serde(serialize_with = "super::debug")]
        view: Interpretation,
    },
    Unavailable {
        #[serde(serialize_with = "super::debug")]
        gap: StorageGap,
    },
    Unary {
        #[serde(serialize_with = "super::debug")]
        op: UnaryOp,
        value: ExprId,
    },
    Binary {
        #[serde(serialize_with = "super::debug")]
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },
    Compare {
        #[serde(serialize_with = "super::debug")]
        op: CompareOp,
        left: ExprId,
        right: ExprId,
    },
    String(ReferenceId),
    Class(ReferenceId),
    MethodHandle(ReferenceId),
    MethodType(ReferenceId),
    Field {
        reference: ReferenceId,
        receiver: Option<ExprId>,
    },
    Array {
        array: ExprId,
        index: ExprId,
        #[serde(serialize_with = "super::debug")]
        kind: ValueKind,
    },
    ArrayLength(ExprId),
    InstanceOf {
        value: ExprId,
        reference: ReferenceId,
    },
    AllocateInstance {
        reference: ReferenceId,
    },
    NewArray {
        reference: ReferenceId,
        length: ExprId,
    },
    FilledArray {
        reference: ReferenceId,
        elements: Arguments,
    },
    Call(Call),
    New {
        reference: ReferenceId,
        constructor: ReferenceId,
        arguments: Vec<ExprId>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InitializationSyntax {
    This,
    Super,
    Explicit,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementKind {
    Nop,
    Move {
        #[serde(serialize_with = "super::debug")]
        kind: crate::p2::MoveKind,
        expression: ExprId,
    },
    Evaluate {
        expression: ExprId,
    },
    FieldStore {
        reference: ReferenceId,
        receiver: Option<ExprId>,
        value: ExprId,
    },
    ArrayStore {
        array: ExprId,
        index: ExprId,
        value: ExprId,
        #[serde(serialize_with = "super::debug")]
        kind: ValueKind,
    },
    CheckCast {
        value: ExprId,
        reference: ReferenceId,
    },
    FillArray {
        array: ExprId,
        payload: PayloadId,
    },
    Monitor {
        enter: bool,
        lock: ExprId,
    },
    CaptureException,
    Initialize {
        expression: ExprId,
        syntax: InitializationSyntax,
        #[serde(serialize_with = "super::debug")]
        relation: crate::p4::ConstructorRelation,
    },
    UnresolvedResult {
        #[serde(serialize_with = "super::debug")]
        width: crate::SlotWidth,
    },
    Construction(ConstructionId),
}
#[derive(Debug, Serialize)]
pub struct Statement {
    pub id: StatementId,
    pub kind: StatementKind,
    #[serde(serialize_with = "super::debug")]
    pub outputs: Vec<DefinitionPlan>,
    pub inputs: Arguments,
    pub before: Vec<CopyPlanId>,
    pub snapshots: Vec<CopyPlanId>,
    pub after: Vec<CopyPlanId>,
    pub sites: Vec<SiteId>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Emission {
    Block { node: crate::p6::NodeId },
    Literal,
    Construction(ConstructionId),
    Catch(ControlId),
    Acquire(ControlId),
    Release(ControlId),
}
#[derive(Debug, Serialize)]
pub struct Operation {
    pub node: crate::p6::NodeId,
    pub original_block: crate::p2::BlockId,
    #[serde(serialize_with = "super::debug")]
    pub source: crate::p2::OpId,
    pub pc: crate::CodeUnitOffset,
    pub result_pc: Option<crate::CodeUnitOffset>,
    #[serde(serialize_with = "super::debug")]
    pub original: crate::p2::OpKind,
    #[serde(serialize_with = "super::debug")]
    pub effects: crate::p2::Effects,
    #[serde(serialize_with = "super::debug")]
    pub normal: crate::p2::NormalCompletion,
    #[serde(serialize_with = "super::debug")]
    pub exceptional: Option<crate::p2::ExceptionCompletion>,
    #[serde(serialize_with = "super::debug")]
    pub reads: Result<Vec<crate::RegisterSlot>, SourceFailure>,
    #[serde(serialize_with = "super::debug")]
    pub writes: Vec<crate::RegisterSlot>,
    pub statement: Option<StatementId>,
    pub emission: Emission,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    Pair {
        #[serde(serialize_with = "super::debug")]
        op: BranchOp,
        left: ExprId,
        right: ExprId,
    },
    Zero {
        #[serde(serialize_with = "super::debug")]
        op: BranchOp,
        value: ExprId,
        meaning: ZeroMeaning,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZeroMeaning {
    Null,
    IntegralZero,
    BitsZero,
    Unavailable,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKind {
    None,
    Goto(TransferId),
    If {
        condition: Condition,
        when_true: TransferId,
        when_false: TransferId,
    },
    Switch {
        selector: ExprId,
        arms: Vec<TransferId>,
    },
    Return(Option<ExprId>),
    Throw(ExprId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalEmission {
    Block,
    If(ControlId),
    Loop(ControlId),
    Switch(ControlId),
    SyncExit(ControlId),
    Construction(ConstructionId),
}
#[derive(Debug, Serialize)]
pub struct Terminal {
    pub block: crate::p2::BlockId,
    pub pc: Option<crate::CodeUnitOffset>,
    pub kind: TerminalKind,
    pub emission: TerminalEmission,
    pub inputs: Vec<ExprId>,
    pub before: Vec<CopyPlanId>,
    pub snapshots: Vec<CopyPlanId>,
    pub site: Option<SiteId>,
}
