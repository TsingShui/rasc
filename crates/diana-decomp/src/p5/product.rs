use crate::SlotWidth;
use crate::p2::{BlockId, CommitRule, ExecutionFacts, MethodEvidence, OpId};
use crate::p3::{EntryRole, PhiId, PointId, SsaFacts, StateId, TransferId, UseSite, ValueId};
use crate::p4::{
    Bits, ConstructorFact, Families, Family, Initialization, Interpretation, Problem, TypeFacts,
    TypedMethod,
};

macro_rules! identity {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub u32);
    )*};
}
identity!(LocalId, PlanId, CopyId, SnapshotId, GroupId);

/// Existing P4 representation. Numeric bit storage does not choose int/float
/// or long/double and never permits an arbitrary numeric/reference union.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageType {
    Family {
        family: Family,
        width: SlotWidth,
    },
    NumericBits {
        families: Families,
        width: SlotWidth,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalKind {
    Parameter { value: ValueId, role: EntryRole },
    Value(ValueId),
    ReferenceCarrier(ValueId),
    AllocationCarrier { value: ValueId, operation: OpId },
    Snapshot { phi: PhiId, entry: PointId },
    ParallelTemporary { plan: PlanId, saved: LocalId },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Local {
    pub(super) id: LocalId,
    pub(super) storage_type: StorageType,
    pub(super) kind: LocalKind,
}
impl Local {
    pub fn id(&self) -> LocalId {
        self.id
    }
    pub fn storage_type(&self) -> StorageType {
        self.storage_type
    }
    pub fn kind(&self) -> LocalKind {
        self.kind
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageGap {
    Upstream(Problem),
    Representation { value: ValueId, families: Families },
    SourceUnavailable(StateId),
    InputLayout(OpId),
    Dependency { plan: PlanId, copy: CopyId },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageBinding {
    Parameter { local: LocalId, role: EntryRole },
    Local(LocalId),
    ReferenceCarrier(LocalId),
    Uninitialized { local: LocalId, allocation: OpId },
    Literal(Bits),
    Deferred { local: LocalId, group: GroupId },
    Unavailable(StorageGap),
}
impl StorageBinding {
    pub fn local(self) -> Option<LocalId> {
        match self {
            Self::Parameter { local, .. }
            | Self::Deferred { local, .. }
            | Self::Local(local)
            | Self::ReferenceCarrier(local)
            | Self::Uninitialized { local, .. } => Some(local),
            Self::Literal(_) | Self::Unavailable(_) => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageRead {
    Value {
        local: LocalId,
        value: ValueId,
        state: StateId,
        view: Interpretation,
    },
    Literal {
        value: ValueId,
        state: StateId,
        bits: Bits,
        view: Interpretation,
    },
    /// Receives the already adapted save value, without applying its view twice.
    Snapshot {
        snapshot: SnapshotId,
        local: LocalId,
        phi: PhiId,
    },
}
impl StorageRead {
    pub fn local(self) -> Option<LocalId> {
        match self {
            Self::Value { local, .. } | Self::Snapshot { local, .. } => Some(local),
            Self::Literal { .. } => None,
        }
    }
    pub(super) fn redirect(&mut self, to: LocalId) {
        match self {
            Self::Value { local, .. } | Self::Snapshot { local, .. } => *local = to,
            Self::Literal { .. } => {}
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct UseAccess {
    pub(super) site: UseSite,
    pub(super) point: PointId,
    pub(super) state: StateId,
    pub(super) read: Result<StorageRead, StorageGap>,
    pub(super) gap: Option<StorageGap>,
    pub(super) initialization: Option<Initialization>,
}
impl UseAccess {
    pub fn site(&self) -> UseSite {
        self.site
    }
    pub fn point(&self) -> PointId {
        self.point
    }
    pub fn state(&self) -> StateId {
        self.state
    }
    pub fn read(&self) -> Result<StorageRead, StorageGap> {
        self.gap.map_or(self.read, Err)
    }
    pub fn original_source(&self) -> Result<StorageRead, StorageGap> {
        self.read
    }
    /// Copied from the original P4 use; absence does not imply initialized.
    pub fn initialization(&self) -> Option<Initialization> {
        self.initialization
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Materialization {
    Store(LocalId),
    AllocationCarrier(LocalId),
    ExceptionCapture(LocalId),
    Deferred { group: GroupId, local: LocalId },
    Literal(Bits),
    Unavailable(StorageGap),
}
#[derive(Clone, Copy, Debug)]
pub struct DefinitionPlan {
    pub(super) operation: OpId,
    pub(super) output: u32,
    pub(super) state: StateId,
    pub(super) point: PointId,
    pub(super) commit: CommitRule,
    pub(super) commit_pc: Option<crate::CodeUnitOffset>,
    pub(super) materialization: Materialization,
}
impl DefinitionPlan {
    pub fn operation(&self) -> OpId {
        self.operation
    }
    pub fn output(&self) -> u32 {
        self.output
    }
    pub fn state(&self) -> StateId {
        self.state
    }
    pub fn point(&self) -> PointId {
        self.point
    }
    pub fn commit(&self) -> CommitRule {
        self.commit
    }
    pub fn commit_pc(&self) -> Option<crate::CodeUnitOffset> {
        self.commit_pc
    }
    pub fn materialization(&self) -> Materialization {
        self.materialization
    }
}
#[derive(Clone, Copy)]
pub(super) struct Span {
    pub start: u32,
    pub len: u32,
}
#[derive(Clone, Copy)]
pub(super) struct OperationAccess {
    pub inputs: Result<Span, StorageGap>,
    pub outputs: Span,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ThrowSite {
    Operation(OpId),
    Terminal(BlockId),
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TransferSite {
    InitialEntry,
    NormalEdge(crate::p2::EdgeId),
    AfterCommit(OpId),
    BeforeOperation(OpId),
    BeforeTerminal(BlockId),
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CopyLocation {
    /// Executes only when this exact original state transfer is selected,
    /// after its normal commit and result materialization where applicable.
    Transfer {
        transfer: TransferId,
        site: TransferSite,
    },
    BeforeThrow(ThrowSite),
    /// Exception arrivals only, never an unconditional block prologue. An
    /// original exception capture remains before these receives when present.
    HandlerEntry {
        point: PointId,
        block: BlockId,
        capture: Option<OpId>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyOrigin {
    Incoming { phi: PhiId, transfer: TransferId },
    HandlerReceive(PhiId),
}
#[derive(Clone, Copy, Debug)]
pub struct CopyAssignment {
    pub(super) id: CopyId,
    pub(super) origin: CopyOrigin,
    pub(super) destination: Result<LocalId, StorageGap>,
    pub(super) source: Result<StorageRead, StorageGap>,
    /// The original storage read dependency remains known even if its view fails.
    pub(super) dependency: Option<LocalId>,
    pub(super) issue: Option<StorageGap>,
}
impl CopyAssignment {
    pub fn id(&self) -> CopyId {
        self.id
    }
    pub fn origin(&self) -> CopyOrigin {
        self.origin
    }
    pub fn destination(&self) -> Result<LocalId, StorageGap> {
        self.destination
    }
    pub fn source(&self) -> Result<StorageRead, StorageGap> {
        self.source
    }
    pub fn issue(&self) -> Option<StorageGap> {
        self.issue
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyStep {
    Save {
        destination: LocalId,
        source: LocalId,
    },
    Assign {
        copy: CopyId,
        destination: LocalId,
        source: StorageRead,
    },
}
#[derive(Debug)]
pub struct CopyPlan {
    pub(super) id: PlanId,
    pub(super) location: CopyLocation,
    pub(super) assignments: Vec<CopyAssignment>,
    pub(super) steps: Vec<CopyStep>,
}
impl CopyPlan {
    pub fn id(&self) -> PlanId {
        self.id
    }
    pub fn location(&self) -> CopyLocation {
        self.location
    }
    pub fn assignments(&self) -> &[CopyAssignment] {
        &self.assignments
    }
    pub fn steps(&self) -> &[CopyStep] {
        &self.steps
    }
    pub fn is_complete(&self) -> bool {
        self.assignments.iter().all(|a| a.issue.is_none())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub(super) id: SnapshotId,
    pub(super) phi: PhiId,
    pub(super) entry: PointId,
    pub(super) local: LocalId,
}
impl Snapshot {
    pub fn id(&self) -> SnapshotId {
        self.id
    }
    pub fn phi(&self) -> PhiId {
        self.phi
    }
    pub fn entry(&self) -> PointId {
        self.entry
    }
    pub fn local(&self) -> LocalId {
        self.local
    }
}
#[derive(Clone, Copy, Debug)]
pub enum TransferRoute {
    Literal,
    Normal {
        plan: PlanId,
        copy: CopyId,
    },
    Exception {
        snapshot: SnapshotId,
        save_plan: PlanId,
        save: CopyId,
        receive_plan: PlanId,
        receive: CopyId,
    },
    Unavailable(StorageGap),
}
#[derive(Clone, Copy, Debug)]
pub struct PhiTransfer {
    pub(super) phi: PhiId,
    pub(super) transfer: TransferId,
    pub(super) source: StateId,
    pub(super) view: Interpretation,
    pub(super) route: TransferRoute,
}
impl PhiTransfer {
    pub fn phi(&self) -> PhiId {
        self.phi
    }
    pub fn transfer(&self) -> TransferId {
        self.transfer
    }
    pub fn source(&self) -> StateId {
        self.source
    }
    pub fn view(&self) -> Interpretation {
        self.view
    }
    pub fn route(&self) -> TransferRoute {
        self.route
    }
}
/// Original construction events with explicit reference carriers. This does not
/// move allocation to the call or promise a source-language new-expression merge.
#[derive(Clone, Copy, Debug)]
pub struct ConstructionPlan {
    pub(super) fact: ConstructorFact,
    pub(super) receiver: Option<Result<StorageRead, StorageGap>>,
    pub(super) mode: ConstructionMode,
}
impl ConstructionPlan {
    pub fn fact(&self) -> ConstructorFact {
        self.fact
    }
    pub fn receiver(&self) -> Option<Result<StorageRead, StorageGap>> {
        self.receiver
    }
    pub fn mode(&self) -> ConstructionMode {
        self.mode
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Separation {
    NoEstablishedConstructor,
    NoAllocationBinding,
    NonAllocationConstructor,
    InstanceNotEstablished,
    Initialization(Initialization),
    SharedAllocation,
    ControlBoundary(BlockId),
    InterveningOperation(OpId),
    ObservedBeforeSuccess(UseSite),
    DifferentProtection {
        allocation: OpId,
        initialization: OpId,
    },
    SnapshotMismatch(PlanId),
    MissingStorage(StorageGap),
    ArgumentNotAvailable(UseSite),
    TransferCopy(PlanId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionMode {
    Natural(GroupId),
    Explicit(Separation),
}
#[derive(Clone, Copy, Debug)]
pub struct AllocationPlan {
    pub(super) operation: OpId,
    pub(super) mode: ConstructionMode,
}
impl AllocationPlan {
    pub fn operation(&self) -> OpId {
        self.operation
    }
    pub fn mode(&self) -> ConstructionMode {
        self.mode
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionRole {
    Allocation,
    Alias,
    Literal,
    Initialization,
    /// Prepares a constructor argument. It keeps its own statement and position;
    /// only the allocation moves to the constructor call.
    Argument,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationEmission {
    Ordinary,
    Construction {
        group: GroupId,
        role: ConstructionRole,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyEmission {
    Original,
    Construction { group: GroupId },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Publication {
    pub value: ValueId,
    pub local: LocalId,
}
/// One selected compound materialization, not another execution graph. Original
/// events/commits stay inspectable; members are emitted once through the group.
#[derive(Debug)]
pub struct ConstructionGroup {
    pub(super) id: GroupId,
    pub(super) allocation: OpId,
    pub(super) initialization: OpId,
    pub(super) normal: PointId,
    pub(super) members: Vec<OpId>,
    pub(super) interior_edges: Vec<crate::p2::EdgeId>,
    pub(super) arguments: Vec<StorageRead>,
    pub(super) publications: Vec<Publication>,
    pub(super) prelude: Option<PlanId>,
    pub(super) snapshot_members: Vec<PlanId>,
}
impl ConstructionGroup {
    pub fn id(&self) -> GroupId {
        self.id
    }
    pub fn allocation(&self) -> OpId {
        self.allocation
    }
    pub fn initialization(&self) -> OpId {
        self.initialization
    }
    pub fn normal(&self) -> PointId {
        self.normal
    }
    pub fn members(&self) -> &[OpId] {
        &self.members
    }
    pub fn interior_edges(&self) -> &[crate::p2::EdgeId] {
        &self.interior_edges
    }
    pub fn arguments(&self) -> &[StorageRead] {
        &self.arguments
    }
    pub fn publications(&self) -> &[Publication] {
        &self.publications
    }
    pub fn prelude(&self) -> Option<PlanId> {
        self.prelude
    }
    pub fn snapshot_members(&self) -> &[PlanId] {
        &self.snapshot_members
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StorageIssueSite {
    Value(ValueId),
    Use(UseSite),
    Output { operation: OpId, output: u32 },
    InputLayout(OpId),
    Copy { plan: PlanId, copy: CopyId },
    Incoming { phi: PhiId, transfer: TransferId },
}
#[derive(Clone, Copy, Debug)]
pub struct StorageIssue {
    pub(super) site: StorageIssueSite,
    pub(super) gap: StorageGap,
}
impl StorageIssue {
    pub fn site(&self) -> StorageIssueSite {
        self.site
    }
    pub fn gap(&self) -> StorageGap {
        self.gap
    }
}

/// Published storage coverage with explicit local gaps. A binding or route does
/// not override an unavailable use or a referenced copy assignment's issue.
pub struct StorageFacts {
    pub(super) bindings: Vec<StorageBinding>,
    pub(super) locals: Vec<Local>,
    pub(super) local_gaps: Vec<Option<StorageGap>>,
    pub(super) uses: Vec<UseAccess>,
    pub(super) definitions: Vec<DefinitionPlan>,
    pub(super) operations: Vec<OperationAccess>,
    pub(super) terminals: Vec<Option<Span>>,
    pub(super) copies: Vec<CopyPlan>,
    pub(super) snapshots: Vec<Snapshot>,
    pub(super) transfers: Vec<PhiTransfer>,
    pub(super) constructions: Vec<ConstructionPlan>,
    pub(super) allocation_plans: Vec<AllocationPlan>,
    pub(super) groups: Vec<ConstructionGroup>,
    pub(super) operation_emission: Vec<OperationEmission>,
    pub(super) copy_emission: Vec<CopyEmission>,
    pub(super) issues: Vec<StorageIssue>,
}
impl StorageFacts {
    pub(super) fn empty() -> Self {
        Self {
            bindings: Vec::new(),
            locals: Vec::new(),
            local_gaps: Vec::new(),
            uses: Vec::new(),
            definitions: Vec::new(),
            operations: Vec::new(),
            terminals: Vec::new(),
            copies: Vec::new(),
            snapshots: Vec::new(),
            transfers: Vec::new(),
            constructions: Vec::new(),
            allocation_plans: Vec::new(),
            groups: Vec::new(),
            operation_emission: Vec::new(),
            copy_emission: Vec::new(),
            issues: Vec::new(),
        }
    }
    pub fn bindings(&self) -> &[StorageBinding] {
        &self.bindings
    }
    pub fn binding(&self, value: ValueId) -> Option<StorageBinding> {
        self.bindings.get(value.0 as usize).copied()
    }
    pub fn locals(&self) -> &[Local] {
        &self.locals
    }
    pub fn local_gap(&self, local: LocalId) -> Option<StorageGap> {
        self.local_gaps.get(local.0 as usize).copied().flatten()
    }
    pub fn uses(&self) -> &[UseAccess] {
        &self.uses
    }
    pub fn definitions(&self) -> &[DefinitionPlan] {
        &self.definitions
    }
    pub fn copy_plans(&self) -> &[CopyPlan] {
        &self.copies
    }
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }
    pub fn transfers(&self) -> &[PhiTransfer] {
        &self.transfers
    }
    pub fn constructions(&self) -> &[ConstructionPlan] {
        &self.constructions
    }
    pub fn allocation_plans(&self) -> &[AllocationPlan] {
        &self.allocation_plans
    }
    pub fn construction_groups(&self) -> &[ConstructionGroup] {
        &self.groups
    }
    pub fn operation_emission(&self, operation: OpId) -> Option<OperationEmission> {
        self.operation_emission.get(operation.0 as usize).copied()
    }
    pub fn copy_emission(&self, plan: PlanId) -> Option<CopyEmission> {
        self.copy_emission.get(plan.0 as usize).copied()
    }
    pub fn issues(&self) -> &[StorageIssue] {
        &self.issues
    }
    pub fn operation_uses(&self, op: OpId) -> Option<Result<&[UseAccess], StorageGap>> {
        Some(match self.operations.get(op.0 as usize)?.inputs {
            Ok(span) => Ok(self
                .uses
                .get(span.start as usize..span.start.checked_add(span.len)? as usize)?),
            Err(gap) => Err(gap),
        })
    }
    pub fn operation_definitions(&self, op: OpId) -> Option<&[DefinitionPlan]> {
        let span = self.operations.get(op.0 as usize)?.outputs;
        self.definitions
            .get(span.start as usize..span.start.checked_add(span.len)? as usize)
    }
    pub fn terminal_uses(&self, block: BlockId) -> Option<&[UseAccess]> {
        let span = self.terminals.get(block.0 as usize)?.as_ref()?;
        self.uses
            .get(span.start as usize..span.start.checked_add(span.len)? as usize)
    }
}

/// Mature storage plans owning P4. Local gaps remain part of this product;
/// predecessor extraction and reconstruction are not consumer operations.
pub struct StorageMethod<'a> {
    pub(super) typed: TypedMethod<'a>,
    pub(super) facts: StorageFacts,
}
impl<'a> StorageMethod<'a> {
    pub fn facts(&self) -> &StorageFacts {
        &self.facts
    }
    pub fn type_facts(&self) -> &TypeFacts {
        self.typed.facts()
    }
    pub fn execution(&self) -> ExecutionFacts<'_, 'a> {
        self.typed.execution()
    }
    pub fn evidence(&self) -> MethodEvidence<'_, 'a> {
        self.typed.evidence()
    }
    pub fn analysis_evidence(&self) -> AnalysisEvidence<'_, 'a> {
        AnalysisEvidence { typed: &self.typed }
    }
    pub fn storage_evidence(&self) -> StorageEvidence<'_, 'a> {
        StorageEvidence { storage: self }
    }
    pub fn method_id(&self) -> crate::MethodId {
        self.typed.method_id()
    }
    pub fn class_id(&self) -> crate::ClassDefId {
        self.typed.class_id()
    }
}
/// Borrowed P5 inspection through later products; no predecessor extraction.
#[derive(Clone, Copy)]
pub struct StorageEvidence<'f, 'a> {
    storage: &'f StorageMethod<'a>,
}
impl<'f, 'a> StorageEvidence<'f, 'a> {
    pub fn facts(self) -> &'f StorageFacts {
        self.storage.facts()
    }
    pub fn type_facts(self) -> &'f TypeFacts {
        self.storage.type_facts()
    }
    pub fn execution(self) -> ExecutionFacts<'f, 'a> {
        self.storage.execution()
    }
    pub fn class_id(self) -> crate::ClassDefId {
        self.storage.class_id()
    }
    pub fn method_id(self) -> crate::MethodId {
        self.storage.method_id()
    }
    pub fn analysis_evidence(self) -> AnalysisEvidence<'f, 'a> {
        self.storage.analysis_evidence()
    }
}

/// Read-only analysis inspection, not a predecessor extraction or build interface.
#[derive(Clone, Copy)]
pub struct AnalysisEvidence<'f, 'a> {
    typed: &'f TypedMethod<'a>,
}
impl<'f, 'a> AnalysisEvidence<'f, 'a> {
    pub fn class_id(self) -> crate::ClassDefId {
        self.typed.class_id()
    }
    pub fn method_id(self) -> crate::MethodId {
        self.typed.method_id()
    }
    pub fn facts(self) -> &'f TypeFacts {
        self.typed.facts()
    }
    pub fn ssa_facts(self) -> &'f SsaFacts {
        self.typed.ssa_facts()
    }
    pub fn execution(self) -> ExecutionFacts<'f, 'a> {
        self.typed.execution()
    }
}
pub struct StorageStop<'a> {
    pub(super) typed: TypedMethod<'a>,
    pub(super) error: super::StorageError,
    pub(super) partial: StoragePartial,
}
impl<'a> StorageStop<'a> {
    pub fn error(&self) -> &super::StorageError {
        &self.error
    }
    pub fn type_facts(&self) -> &TypeFacts {
        self.typed.facts()
    }
    pub fn execution(&self) -> ExecutionFacts<'_, 'a> {
        self.typed.execution()
    }
    pub fn evidence(&self) -> MethodEvidence<'_, 'a> {
        self.typed.evidence()
    }
    pub fn analysis_evidence(&self) -> AnalysisEvidence<'_, 'a> {
        AnalysisEvidence { typed: &self.typed }
    }
    pub fn partial(&self) -> &StoragePartial {
        &self.partial
    }
}
/// Unpublished owning-stage records; never satisfies StorageFacts guarantees.
pub struct StoragePartial {
    pub(super) facts: StorageFacts,
}
impl StoragePartial {
    pub fn bindings(&self) -> &[StorageBinding] {
        &self.facts.bindings
    }
    pub fn locals(&self) -> &[Local] {
        &self.facts.locals
    }
    pub fn issues(&self) -> &[StorageIssue] {
        &self.facts.issues
    }
}
