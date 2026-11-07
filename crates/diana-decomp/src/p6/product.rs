use crate::p2::{BlockId, EdgeId, ExceptionCompletion, ExitId, OpId};
use crate::p5::{CopyLocation, GroupId, PlanId, StorageGap, StorageRead, ThrowSite};

macro_rules! ids { ($($n:ident),*) => {$(#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd,serde::Serialize)] #[serde(transparent)] pub struct $n(pub u32);)*}; }
ids!(
    NodeId,
    LinkId,
    RegionId,
    LoopId,
    ChainId,
    ConditionId,
    TryId,
    SyncId,
    LabelId,
    ThrowSiteId
);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LabelKind {
    BlockEntry,
    Handler,
    Continuation,
}
#[derive(Clone, Copy, Debug)]
pub struct LabelPlan {
    pub(super) id: LabelId,
    pub(super) node: NodeId,
    pub(super) block: BlockId,
    pub(super) kind: LabelKind,
    pub(super) owner: RegionId,
}
impl LabelPlan {
    pub fn id(&self) -> LabelId {
        self.id
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn block(&self) -> BlockId {
        self.block
    }
    pub fn kind(&self) -> LabelKind {
        self.kind
    }
    pub fn owner(&self) -> RegionId {
        self.owner
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonitorRole {
    Acquire,
    Release,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationOwner {
    Ordinary {
        node: NodeId,
        region: RegionId,
    },
    Construction {
        group: GroupId,
        role: crate::p5::ConstructionRole,
    },
    CatchCapture(TryId),
    Synchronized {
        scope: SyncId,
        role: MonitorRole,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalOwner {
    Ordinary(RegionId),
    IfCondition(RegionId),
    LoopCondition(LoopId),
    SwitchSelector(RegionId),
    SynchronizedExit(SyncId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Continuation {
    None,
    Parent,
    Next(NodeId),
    Goto(LabelId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SiteProtection {
    Explicit,
    Natural,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkOrigin {
    WithinBlock(BlockId),
    Edge(EdgeId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalLink {
    pub(super) id: LinkId,
    pub(super) from: NodeId,
    pub(super) to: NodeId,
    pub(super) origin: LinkOrigin,
}
impl NormalLink {
    pub fn id(&self) -> LinkId {
        self.id
    }
    pub fn from(&self) -> NodeId {
        self.from
    }
    pub fn to(&self) -> NodeId {
        self.to
    }
    pub fn origin(&self) -> LinkOrigin {
        self.origin
    }
}
/// An ordered portion of one original block. Splits isolate monitor and protection
/// boundaries; they add no original edge, instruction, effect or SSA definition.
#[derive(Clone, Debug)]
pub struct Node {
    pub(super) id: NodeId,
    pub(super) block: BlockId,
    pub(super) operations: std::ops::Range<u32>,
    pub(super) terminal: bool,
    pub(super) reachable: bool,
}
impl Node {
    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn block(&self) -> BlockId {
        self.block
    }
    pub fn operations(&self) -> std::ops::Range<u32> {
        self.operations.clone()
    }
    pub fn has_terminal(&self) -> bool {
        self.terminal
    }
    pub fn reachable(&self) -> bool {
        self.reachable
    }
}
#[derive(Debug)]
pub struct BlockPlan {
    pub(super) block: BlockId,
    pub(super) parts: Vec<NodeId>,
}
impl BlockPlan {
    pub fn block(&self) -> BlockId {
        self.block
    }
    pub fn parts(&self) -> &[NodeId] {
        &self.parts
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopForm {
    While(LoopCondition),
    DoWhile(LoopCondition),
    Infinite,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopCondition {
    pub(super) node: NodeId,
    pub(super) repeat: LinkId,
    pub(super) exit: LinkId,
}
impl LoopCondition {
    pub fn node(self) -> NodeId {
        self.node
    }
    pub fn repeat(self) -> LinkId {
        self.repeat
    }
    pub fn exit(self) -> LinkId {
        self.exit
    }
}
#[derive(Debug)]
pub struct LoopPlan {
    pub(super) id: LoopId,
    pub(super) header: NodeId,
    pub(super) members: Vec<NodeId>,
    pub(super) evaluation: Vec<NodeId>,
    pub(super) lexical_members: Vec<NodeId>,
    pub(super) private_exits: Vec<LinkId>,
    pub(super) latches: Vec<LinkId>,
    pub(super) exits: Vec<LinkId>,
    pub(super) follow: Option<NodeId>,
    pub(super) form: LoopForm,
}
impl LoopPlan {
    pub fn id(&self) -> LoopId {
        self.id
    }
    pub fn header(&self) -> NodeId {
        self.header
    }
    pub fn members(&self) -> &[NodeId] {
        &self.members
    }
    /// Original ordered predicate evaluation, separate from repeated body work.
    pub fn evaluation(&self) -> &[NodeId] {
        &self.evaluation
    }
    /// Backedge core plus proved private lexical exit content.
    pub fn lexical_members(&self) -> &[NodeId] {
        &self.lexical_members
    }
    /// Original exits whose single-entry abrupt corridors are lexical body content.
    pub fn private_exits(&self) -> &[LinkId] {
        &self.private_exits
    }
    pub fn latches(&self) -> &[LinkId] {
        &self.latches
    }
    pub fn exits(&self) -> &[LinkId] {
        &self.exits
    }
    pub fn follow(&self) -> Option<NodeId> {
        self.follow
    }
    pub fn condition(&self) -> Option<NodeId> {
        match self.form {
            LoopForm::While(c) | LoopForm::DoWhile(c) => Some(c.node),
            LoopForm::Infinite => None,
        }
    }
    pub fn continue_target(&self) -> NodeId {
        match self.form {
            LoopForm::DoWhile(c) => c.node,
            _ => self.header,
        }
    }
    pub fn form(&self) -> LoopForm {
        self.form
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CatchTarget {
    Handler(NodeId),
    Uncaught,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HandlerClause {
    pub order: u32,
    pub catch: Option<crate::TypeId>,
    pub target: CatchTarget,
}
#[derive(Debug)]
pub struct HandlerChain {
    pub(super) id: ChainId,
    pub(super) clauses: Vec<HandlerClause>,
}
impl HandlerChain {
    pub fn id(&self) -> ChainId {
        self.id
    }
    pub fn clauses(&self) -> &[HandlerClause] {
        &self.clauses
    }
}
#[derive(Debug)]
pub struct ThrowPlan {
    pub(super) id: ThrowSiteId,
    pub(super) site: ThrowSite,
    pub(super) node: NodeId,
    pub(super) completion: ExceptionCompletion,
    pub(super) chain: ChainId,
    pub(super) edges: Vec<EdgeId>,
    pub(super) snapshots: Vec<PlanId>,
    pub(super) construction: Option<GroupId>,
    pub(super) protection: SiteProtection,
}
impl ThrowPlan {
    pub fn protection(&self) -> SiteProtection {
        self.protection
    }
    pub fn id(&self) -> ThrowSiteId {
        self.id
    }
    pub fn site(&self) -> ThrowSite {
        self.site
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn completion(&self) -> ExceptionCompletion {
        self.completion
    }
    pub fn chain(&self) -> ChainId {
        self.chain
    }
    pub fn edges(&self) -> &[EdgeId] {
        &self.edges
    }
    pub fn snapshots(&self) -> &[PlanId] {
        &self.snapshots
    }
    pub fn construction(&self) -> Option<GroupId> {
        self.construction
    }
}
#[derive(Debug)]
pub struct HandlerEntry {
    pub(super) node: NodeId,
    pub(super) capture: Option<OpId>,
    pub(super) receives: Vec<PlanId>,
}
impl HandlerEntry {
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn capture(&self) -> Option<OpId> {
        self.capture
    }
    pub fn receives(&self) -> &[PlanId] {
        &self.receives
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferAction {
    Goto(LabelId),
    Next(NodeId),
    IfArm { region: RegionId, taken: bool },
    SwitchArm { region: RegionId },
    LoopCondition { region: RegionId, repeat: bool },
    Break(RegionId),
    Continue(LoopId),
    EnterSynchronized(RegionId),
    LeaveSynchronized { region: RegionId, release: OpId },
    SynchronizedReleased { region: RegionId, release: OpId },
}
#[derive(Debug)]
pub struct LinkPlan {
    pub(super) link: LinkId,
    pub(super) action: TransferAction,
    pub(super) copies: Vec<PlanId>,
}
impl LinkPlan {
    pub fn link(&self) -> LinkId {
        self.link
    }
    pub fn action(&self) -> TransferAction {
        self.action
    }
    pub fn copies(&self) -> &[PlanId] {
        &self.copies
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExceptionDelivery {
    Explicit,
    Catch(TryId),
    ImpossibleRelease(OpId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeRoute {
    Normal(LinkId),
    Exception {
        site: ThrowSiteId,
        order: u32,
        delivery: ExceptionDelivery,
    },
}
#[derive(Clone, Debug)]
pub struct EdgePlan {
    pub(super) edge: EdgeId,
    pub(super) route: EdgeRoute,
}
impl EdgePlan {
    pub fn edge(&self) -> EdgeId {
        self.edge
    }
    pub fn route(&self) -> EdgeRoute {
        self.route
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyOwner {
    Original,
    Construction(GroupId),
    SynchronizedExit(SyncId),
}
#[derive(Clone, Debug)]
pub struct CopyPlacement {
    pub plan: PlanId,
    pub location: CopyLocation,
    pub owner: CopyOwner,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryReason {
    Irreducible,
    MultipleEntry,
    Overlap,
    ProtectionBoundary,
    LockBoundary,
    NoCommonFollow,
    ConstructionSplit,
    Unstructured,
}
/// One original predicate of a condition region. `select` is the delivered link
/// that reaches the shared result arm; `other` leaves the test. The zone between
/// the previous test and this predicate stays in the region as its own part.
#[derive(Clone, Copy, Debug)]
pub struct ConditionTest {
    pub(super) predicate: NodeId,
    pub(super) select: LinkId,
    pub(super) other: LinkId,
}
impl ConditionTest {
    pub fn predicate(&self) -> NodeId {
        self.predicate
    }
    pub fn select(&self) -> LinkId {
        self.select
    }
    pub fn other(&self) -> LinkId {
        self.other
    }
}
/// Ordered short-circuit tests. The shared arm is entered by every `select` link;
/// the last `other` link reaches the other result arm or the follow directly.
#[derive(Debug)]
pub struct ConditionPlan {
    pub(super) id: ConditionId,
    pub(super) tests: Vec<ConditionTest>,
    pub(super) shared: NodeId,
    pub(super) else_arm: bool,
    pub(super) explicit: Option<ConditionSchedule>,
}
/// Explicit alternative of an accepted condition. Child interiors, entry and
/// abrupt-exit scopes retain their source plans. `completions` supplies every
/// child's normal delivery under this order; `gotos` supplies original links
/// whose implicit delivery crosses the changed child boundaries.
#[derive(Debug)]
pub struct ConditionSchedule {
    pub(super) children: Vec<RegionId>,
    pub(super) completions: Vec<ConditionCompletion>,
    pub(super) gotos: Vec<LinkId>,
}
/// Normal completion of an intact child under the explicit parent layout.
/// This carries no edge copies: those remain on their original selected links.
#[derive(Clone, Copy, Debug)]
pub struct ConditionCompletion {
    pub(super) child: RegionId,
    pub(super) continuation: Continuation,
}
impl ConditionCompletion {
    pub fn child(&self) -> RegionId {
        self.child
    }
    pub fn continuation(&self) -> Continuation {
        self.continuation
    }
}
impl ConditionSchedule {
    pub fn children(&self) -> &[RegionId] {
        &self.children
    }
    pub fn completions(&self) -> &[ConditionCompletion] {
        &self.completions
    }
    pub fn gotos(&self) -> &[LinkId] {
        &self.gotos
    }
}
impl ConditionPlan {
    pub fn id(&self) -> ConditionId {
        self.id
    }
    pub fn tests(&self) -> &[ConditionTest] {
        &self.tests
    }
    pub fn shared(&self) -> NodeId {
        self.shared
    }
    /// Present for every selected Condition region; rejected proposals have no
    /// layout. Entry, follow and exact predicate portions retain their source IDs.
    pub fn explicit(&self) -> Option<&ConditionSchedule> {
        self.explicit.as_ref()
    }
    /// Whether the last test's other link reaches a second result arm rather
    /// than leaving the region directly.
    pub fn else_arm(&self) -> bool {
        self.else_arm
    }
}
#[derive(Debug)]
pub enum RegionKind {
    Block(NodeId),
    Sequence,
    If {
        true_link: LinkId,
        false_link: LinkId,
    },
    /// Ordered tests whose selected edges share one result arm. Each test keeps
    /// its original predicate, links and evaluation order.
    Condition(ConditionId),
    Loop(LoopId),
    Switch {
        arms: Vec<LinkId>,
        private_cases: bool,
    },
    LabelledBlock {
        target: NodeId,
    },
    Try(TryId),
    Synchronized(SyncId),
    Graph {
        reason: RecoveryReason,
        entries: Vec<NodeId>,
        exits: Vec<LinkId>,
    },
}
/// Children are positional: If=(header,true,false), Loop=(body),
/// Switch=(header,body), Try=(body,handlers...), Sync=(enter,body,cleanup...).
#[derive(Debug)]
pub struct Region {
    pub(super) id: RegionId,
    pub(super) kind: RegionKind,
    pub(super) entry: Option<NodeId>,
    pub(super) follow: Option<NodeId>,
    pub(super) children: Vec<RegionId>,
    pub(super) nodes: Vec<NodeId>,
    pub(super) continuation: Continuation,
}
impl Region {
    pub fn continuation(&self) -> Continuation {
        self.continuation
    }
    pub fn id(&self) -> RegionId {
        self.id
    }
    pub fn kind(&self) -> &RegionKind {
        &self.kind
    }
    pub fn entry(&self) -> Option<NodeId> {
        self.entry
    }
    pub fn follow(&self) -> Option<NodeId> {
        self.follow
    }
    pub fn children(&self) -> &[RegionId] {
        &self.children
    }
    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }
}
#[derive(Debug)]
pub struct TryPlan {
    pub(super) id: TryId,
    pub(super) chain: ChainId,
    pub(super) clause_range: std::ops::Range<u32>,
    pub(super) outer_sync: Option<SyncId>,
    pub(super) entry: NodeId,
    pub(super) body: Vec<NodeId>,
    pub(super) handlers: Vec<NodeId>,
    pub(super) handler_bodies: Vec<Vec<NodeId>>,
    pub(super) members: Vec<NodeId>,
    pub(super) follow: Option<NodeId>,
}
impl TryPlan {
    pub fn id(&self) -> TryId {
        self.id
    }
    pub fn chain(&self) -> ChainId {
        self.chain
    }
    pub fn clauses(&self) -> std::ops::Range<u32> {
        self.clause_range.clone()
    }
    pub fn outer_sync(&self) -> Option<SyncId> {
        self.outer_sync
    }
    pub fn entry(&self) -> NodeId {
        self.entry
    }
    pub fn body(&self) -> &[NodeId] {
        &self.body
    }
    pub fn handlers(&self) -> &[NodeId] {
        &self.handlers
    }
    pub fn handler_bodies(&self) -> &[Vec<NodeId>] {
        &self.handler_bodies
    }
    pub fn members(&self) -> &[NodeId] {
        &self.members
    }
    pub fn follow(&self) -> Option<NodeId> {
        self.follow
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockGap {
    Storage(StorageGap),
    Merge(NodeId),
    NoLocalAcquire(OpId),
    Reference { enter: OpId, exit: OpId },
    RepeatedAcquire(OpId),
}
#[derive(Debug, Eq, PartialEq)]
pub enum LockState {
    Unreachable,
    Known(Vec<OpId>),
    Unknown(LockGap),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockSeparation {
    Read(StorageGap),
    State(LockGap),
    Unreachable,
    ReenteredAcquire,
    NotHeld(NodeId),
    AbruptExit(NodeId),
    EscapingException(ThrowSiteId),
    IncompatibleContinuation { release: OpId, target: NodeId },
    PostReleaseCopy(PlanId),
    LoopContext(LoopId),
    ExternalEntry,
    Region(RecoveryReason),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonitorMode {
    Natural(SyncId),
    Explicit(LockSeparation),
}
#[derive(Clone, Copy, Debug)]
pub struct MonitorPlan {
    pub(super) enter: OpId,
    pub(super) mode: MonitorMode,
}
impl MonitorPlan {
    pub fn enter(&self) -> OpId {
        self.enter
    }
    pub fn mode(&self) -> MonitorMode {
        self.mode
    }
}
#[derive(Debug)]
pub struct LockEvent {
    pub(super) operation: OpId,
    pub(super) receiver: Result<StorageRead, StorageGap>,
    pub(super) before: LockState,
    pub(super) normal: LockState,
    pub(super) exception: LockState,
    pub(super) matched: Option<OpId>,
    pub(super) release_cannot_throw: bool,
}
impl LockEvent {
    pub fn operation(&self) -> OpId {
        self.operation
    }
    pub fn receiver(&self) -> Result<StorageRead, StorageGap> {
        self.receiver
    }
    pub fn before(&self) -> &LockState {
        &self.before
    }
    pub fn normal(&self) -> &LockState {
        &self.normal
    }
    pub fn exception(&self) -> &LockState {
        &self.exception
    }
    pub fn matched(&self) -> Option<OpId> {
        self.matched
    }
    pub fn release_cannot_throw(&self) -> bool {
        self.release_cannot_throw
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncLoopTransfer {
    Break,
    Continue,
}
/// The outlet's original continuation. Abrupt intents carry no new dispatcher
/// state; a non-representable continuation keeps the scope explicit instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncExitKind {
    Follow(NodeId),
    Loop {
        loop_id: LoopId,
        kind: SyncLoopTransfer,
        target: NodeId,
    },
    ReturnVoid {
        terminal: NodeId,
    },
    Rethrow {
        terminal: NodeId,
        /// Original exception-value provenance, not an ambient/current handler token.
        capture: OpId,
        /// The exact original terminal read; do not substitute the capture local.
        exception: StorageRead,
    },
}
#[derive(Debug)]
pub struct SyncExit {
    pub(super) release: OpId,
    pub(super) link: LinkId,
    pub(super) target: NodeId,
    pub(super) kind: SyncExitKind,
    pub(super) routing: Vec<NodeId>,
    pub(super) before: Vec<PlanId>,
    pub(super) after: Vec<PlanId>,
}
impl SyncExit {
    pub fn release(&self) -> OpId {
        self.release
    }
    pub fn release_link(&self) -> LinkId {
        self.link
    }
    pub fn target(&self) -> NodeId {
        self.target
    }
    pub fn kind(&self) -> SyncExitKind {
        self.kind
    }
    pub fn routing(&self) -> &[NodeId] {
        &self.routing
    }
    pub fn before_release(&self) -> &[PlanId] {
        &self.before
    }
    pub fn after_release(&self) -> &[PlanId] {
        &self.after
    }
}
/// Body nodes include syntax-owned exit anchors. `held_nodes` separately records
/// original pre-release execution; exit routing must not become ordinary locked work.
#[derive(Debug)]
pub struct SyncPlan {
    pub(super) id: SyncId,
    pub(super) enter: OpId,
    pub(super) enter_link: LinkId,
    pub(super) lock: StorageRead,
    pub(super) held: Vec<NodeId>,
    pub(super) body: Vec<NodeId>,
    pub(super) exits: Vec<SyncExit>,
    pub(super) members: Vec<NodeId>,
    pub(super) follow: Option<NodeId>,
    pub(super) separation: Option<LockSeparation>,
}
impl SyncPlan {
    pub fn id(&self) -> SyncId {
        self.id
    }
    pub fn enter(&self) -> OpId {
        self.enter
    }
    pub fn enter_link(&self) -> LinkId {
        self.enter_link
    }
    pub fn lock(&self) -> StorageRead {
        self.lock
    }
    pub fn held_nodes(&self) -> &[NodeId] {
        &self.held
    }
    pub fn body(&self) -> &[NodeId] {
        &self.body
    }
    pub fn exits(&self) -> &[SyncExit] {
        &self.exits
    }
    pub fn members(&self) -> &[NodeId] {
        &self.members
    }
    pub fn follow(&self) -> Option<NodeId> {
        self.follow
    }
    pub fn separation(&self) -> Option<LockSeparation> {
        self.separation
    }
}
#[derive(Clone, Debug)]
pub enum PlanningIssue {
    Graph {
        entry: NodeId,
        reason: RecoveryReason,
    },
    Storage {
        operation: Option<OpId>,
        block: BlockId,
        gap: StorageGap,
    },
    Lock {
        operation: OpId,
        gap: LockGap,
    },
    Protection {
        site: ThrowSite,
        reason: RecoveryReason,
    },
}
pub struct RegionFacts {
    pub(super) nodes: Vec<Node>,
    pub(super) blocks: Vec<BlockPlan>,
    pub(super) links: Vec<NormalLink>,
    pub(super) link_plans: Vec<LinkPlan>,
    pub(super) edges: Vec<EdgePlan>,
    pub(super) loops: Vec<LoopPlan>,
    pub(super) regions: Vec<Region>,
    pub(super) root: Option<RegionId>,
    pub(super) parents: Vec<Option<RegionId>>,
    pub(super) chains: Vec<HandlerChain>,
    pub(super) throws: Vec<ThrowPlan>,
    pub(super) handlers: Vec<HandlerEntry>,
    pub(super) tries: Vec<TryPlan>,
    pub(super) conditions: Vec<ConditionPlan>,
    pub(super) locks: Vec<LockEvent>,
    pub(super) monitors: Vec<MonitorPlan>,
    pub(super) synchronized: Vec<SyncPlan>,
    pub(super) copies: Vec<CopyPlacement>,
    pub(super) issues: Vec<PlanningIssue>,
    pub(super) exits: Vec<ExitId>,
    pub(super) labels: Vec<LabelPlan>,
    pub(super) operation_owners: Vec<OperationOwner>,
    pub(super) terminal_owners: Vec<TerminalOwner>,
}
impl RegionFacts {
    pub(super) fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            blocks: Vec::new(),
            links: Vec::new(),
            link_plans: Vec::new(),
            edges: Vec::new(),
            loops: Vec::new(),
            regions: Vec::new(),
            root: None,
            parents: Vec::new(),
            chains: Vec::new(),
            throws: Vec::new(),
            handlers: Vec::new(),
            tries: Vec::new(),
            conditions: Vec::new(),
            locks: Vec::new(),
            monitors: Vec::new(),
            synchronized: Vec::new(),
            copies: Vec::new(),
            issues: Vec::new(),
            exits: Vec::new(),
            labels: Vec::new(),
            operation_owners: Vec::new(),
            terminal_owners: Vec::new(),
        }
    }
    pub fn labels(&self) -> &[LabelPlan] {
        &self.labels
    }
    pub fn operation_owners(&self) -> &[OperationOwner] {
        &self.operation_owners
    }
    pub fn terminal_owners(&self) -> &[TerminalOwner] {
        &self.terminal_owners
    }
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    pub fn blocks(&self) -> &[BlockPlan] {
        &self.blocks
    }
    pub fn links(&self) -> &[NormalLink] {
        &self.links
    }
    pub fn link_plans(&self) -> &[LinkPlan] {
        &self.link_plans
    }
    pub fn edges(&self) -> &[EdgePlan] {
        &self.edges
    }
    pub fn loops(&self) -> &[LoopPlan] {
        &self.loops
    }
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }
    pub fn root(&self) -> Option<RegionId> {
        self.root
    }
    pub fn parent(&self, id: RegionId) -> Option<RegionId> {
        self.parents.get(id.0 as usize).copied().flatten()
    }
    pub fn handler_chains(&self) -> &[HandlerChain] {
        &self.chains
    }
    pub fn throws(&self) -> &[ThrowPlan] {
        &self.throws
    }
    pub fn handler_entries(&self) -> &[HandlerEntry] {
        &self.handlers
    }
    pub fn tries(&self) -> &[TryPlan] {
        &self.tries
    }
    pub fn conditions(&self) -> &[ConditionPlan] {
        &self.conditions
    }
    pub fn locks(&self) -> &[LockEvent] {
        &self.locks
    }
    pub fn monitor_plans(&self) -> &[MonitorPlan] {
        &self.monitors
    }
    pub fn synchronized(&self) -> &[SyncPlan] {
        &self.synchronized
    }
    pub fn copy_placements(&self) -> &[CopyPlacement] {
        &self.copies
    }
    pub fn issues(&self) -> &[PlanningIssue] {
        &self.issues
    }
    pub fn exits(&self) -> &[ExitId] {
        &self.exits
    }
}
pub struct PlannedMethod<'a> {
    pub(super) storage: crate::p5::StorageMethod<'a>,
    pub(super) facts: RegionFacts,
}
impl<'a> PlannedMethod<'a> {
    pub fn facts(&self) -> &RegionFacts {
        &self.facts
    }
    pub fn storage_facts(&self) -> &crate::p5::StorageFacts {
        self.storage.facts()
    }
    pub fn type_facts(&self) -> &crate::p4::TypeFacts {
        self.storage.type_facts()
    }
    pub fn execution(&self) -> crate::p2::ExecutionFacts<'_, 'a> {
        self.storage.execution()
    }
    pub fn evidence(&self) -> crate::p2::MethodEvidence<'_, 'a> {
        self.storage.evidence()
    }
    pub fn analysis_evidence(&self) -> crate::p5::AnalysisEvidence<'_, 'a> {
        self.storage.analysis_evidence()
    }
    pub fn storage_evidence(&self) -> crate::p5::StorageEvidence<'_, 'a> {
        self.storage.storage_evidence()
    }
    pub fn method_id(&self) -> crate::MethodId {
        self.storage.method_id()
    }
    pub fn class_id(&self) -> crate::ClassDefId {
        self.storage.class_id()
    }
}
pub struct RegionStop<'a> {
    pub(super) storage: crate::p5::StorageMethod<'a>,
    pub(super) error: super::RegionError,
    pub(super) partial: RegionPartial,
}
impl<'a> RegionStop<'a> {
    pub fn error(&self) -> &super::RegionError {
        &self.error
    }
    pub fn partial(&self) -> &RegionPartial {
        &self.partial
    }
    pub fn storage_facts(&self) -> &crate::p5::StorageFacts {
        self.storage.facts()
    }
    pub fn type_facts(&self) -> &crate::p4::TypeFacts {
        self.storage.type_facts()
    }
    pub fn execution(&self) -> crate::p2::ExecutionFacts<'_, 'a> {
        self.storage.execution()
    }
    pub fn evidence(&self) -> crate::p2::MethodEvidence<'_, 'a> {
        self.storage.evidence()
    }
    pub fn analysis_evidence(&self) -> crate::p5::AnalysisEvidence<'_, 'a> {
        self.storage.analysis_evidence()
    }
    pub fn storage_evidence(&self) -> crate::p5::StorageEvidence<'_, 'a> {
        self.storage.storage_evidence()
    }
}
pub struct RegionPartial {
    pub(super) facts: RegionFacts,
}
impl RegionPartial {
    pub fn nodes(&self) -> &[Node] {
        &self.facts.nodes
    }
    pub fn regions(&self) -> &[Region] {
        &self.facts.regions
    }
    pub fn issues(&self) -> &[PlanningIssue] {
        &self.facts.issues
    }
}
