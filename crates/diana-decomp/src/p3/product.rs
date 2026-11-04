use crate::p2::{BlockId, EdgeId, ExitId, OpId};
use crate::{RegisterSlot, SlotWidth};

macro_rules! identity {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub u32);
    )*};
}
identity!(PointId, TransferId, SlotId, StateId, ValueId, PhiId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointKind {
    Entry,
    BlockEntry(BlockId),
    Execute(OpId),
    Commit(OpId),
    Terminal(BlockId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferOrigin {
    Entry,
    Continue,
    NormalCommit(OpId),
    Edge(EdgeId),
    Return(ExitId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Point(PointId),
    Exit(ExitId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryRole {
    This,
    Parameter(u32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueOrigin {
    Entry(EntryRole),
    Operation { operation: OpId, output: u32 },
    Exception(OpId),
    Phi(PhiId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateKind {
    Value(ValueId),
    Undefined,
    Invalidated {
        operation: OpId,
    },
    /// The original operand interval is outside the method's register window.
    OutsideFrame,
    Unavailable {
        operation: OpId,
    },
    Phi(PhiId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseSite {
    Operation { operation: OpId, argument: u32 },
    Terminal { block: BlockId, argument: u32 },
}

/// Possible source categories, not a claim that an abstract path executes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Availability(pub(super) u8);
impl Availability {
    pub fn is_available(self) -> bool {
        self.0 == 1
    }
    pub fn may_have_value(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn may_be_undefined(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn may_be_unavailable(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn may_be_invalidated(self) -> bool {
        self.0 & 4 != 0
    }
}

macro_rules! record {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name { $(pub(super) $field: $ty),* }
        impl $name { $(pub fn $field(&self) -> $ty { self.$field })* }
    };
}
record!(StatePoint { id: PointId, kind: PointKind, idom: Option<PointId>, before: u32, after: u32 });
record!(StateTransfer {
    id: TransferId,
    from: PointId,
    target: Target,
    origin: TransferOrigin
});
record!(StateDefinition {
    id: StateId,
    slot: SlotId,
    point: PointId,
    kind: StateKind,
    availability: Availability
});
record!(ValueDefinition {
    id: ValueId,
    slot: SlotId,
    point: PointId,
    origin: ValueOrigin
});
record!(UseBinding {
    site: UseSite,
    point: PointId,
    slot: SlotId,
    state: StateId
});
record!(OutputBinding {
    operation: OpId,
    output: u32,
    point: PointId,
    slot: SlotId,
    state: StateId
});
record!(PhiIncoming {
    transfer: TransferId,
    state: StateId
});
record!(OperationSite { operation: OpId, execute: PointId, commit: Option<PointId>,
    commit_rule: crate::p2::CommitRule, commit_pc: Option<crate::CodeUnitOffset>,
    use_start: u32, use_len: u32, inputs_available: bool, output_start: u32, output_len: u32 });
record!(TerminalSite {
    block: BlockId,
    point: PointId,
    use_start: u32,
    use_len: u32
});

#[derive(Debug)]
pub struct Phi {
    pub(super) id: PhiId,
    pub(super) point: PointId,
    pub(super) slot: SlotId,
    pub(super) state: StateId,
    pub(super) value: ValueId,
    pub(super) incoming: Vec<PhiIncoming>,
}
impl Phi {
    pub fn id(&self) -> PhiId {
        self.id
    }
    pub fn point(&self) -> PointId {
        self.point
    }
    pub fn slot(&self) -> SlotId {
        self.slot
    }
    pub fn state(&self) -> StateId {
        self.state
    }
    pub fn value(&self) -> ValueId {
        self.value
    }
    pub fn incoming(&self) -> &[PhiIncoming] {
        &self.incoming
    }
}

pub struct SsaFacts {
    pub(super) slots: Vec<RegisterSlot>,
    pub(super) points: Vec<StatePoint>,
    pub(super) transfers: Vec<StateTransfer>,
    pub(super) states: Vec<StateDefinition>,
    pub(super) values: Vec<ValueDefinition>,
    pub(super) uses: Vec<UseBinding>,
    pub(super) outputs: Vec<OutputBinding>,
    pub(super) phis: Vec<Phi>,
    pub(super) operations: Vec<OperationSite>,
    pub(super) block_entries: Vec<PointId>,
    pub(super) terminals: Vec<Option<TerminalSite>>,
}
impl SsaFacts {
    pub(super) fn empty() -> Self {
        Self {
            slots: Vec::new(),
            points: Vec::new(),
            transfers: Vec::new(),
            states: Vec::new(),
            values: Vec::new(),
            uses: Vec::new(),
            outputs: Vec::new(),
            phis: Vec::new(),
            operations: Vec::new(),
            block_entries: Vec::new(),
            terminals: Vec::new(),
        }
    }
    pub fn slots(&self) -> &[RegisterSlot] {
        &self.slots
    }
    pub fn points(&self) -> &[StatePoint] {
        &self.points
    }
    pub fn transfers(&self) -> &[StateTransfer] {
        &self.transfers
    }
    pub fn states(&self) -> &[StateDefinition] {
        &self.states
    }
    pub fn values(&self) -> &[ValueDefinition] {
        &self.values
    }
    pub fn uses(&self) -> &[UseBinding] {
        &self.uses
    }
    pub fn outputs(&self) -> &[OutputBinding] {
        &self.outputs
    }
    pub fn phis(&self) -> &[Phi] {
        &self.phis
    }
    pub fn operation_sites(&self) -> &[OperationSite] {
        &self.operations
    }
    pub fn operation_site(&self, op: OpId) -> Option<&OperationSite> {
        self.operations.get(op.0 as usize)
    }
    /// Err identifies an operation whose ordered input layout is unavailable;
    /// the original cause and ungrouped words remain in the execution product.
    pub fn operation_uses(&self, op: OpId) -> Option<Result<&[UseBinding], OpId>> {
        let site = self.operation_site(op)?;
        if !site.inputs_available {
            return Some(Err(op));
        }
        self.uses
            .get(site.use_start as usize..(site.use_start.checked_add(site.use_len)?) as usize)
            .map(Ok)
    }
    /// The state is the single source of truth; an unavailable output has no ValueId.
    pub fn output_value(&self, output: &OutputBinding) -> Option<ValueId> {
        match self.state(output.state)?.kind {
            StateKind::Value(value) => Some(value),
            _ => None,
        }
    }
    pub fn operation_outputs(&self, op: OpId) -> Option<&[OutputBinding]> {
        let site = self.operation_site(op)?;
        self.outputs.get(
            site.output_start as usize..(site.output_start.checked_add(site.output_len)?) as usize,
        )
    }
    pub fn terminal_site(&self, block: BlockId) -> Option<TerminalSite> {
        self.terminals.get(block.0 as usize).copied().flatten()
    }
    pub fn terminal_uses(&self, block: BlockId) -> Option<&[UseBinding]> {
        let site = self.terminal_site(block)?;
        self.uses
            .get(site.use_start as usize..(site.use_start.checked_add(site.use_len)?) as usize)
    }
    pub fn block_entry(&self, block: BlockId) -> Option<PointId> {
        self.block_entries.get(block.0 as usize).copied()
    }
    pub fn state(&self, state: StateId) -> Option<&StateDefinition> {
        self.states.get(state.0 as usize)
    }
    pub fn slot(&self, slot: SlotId) -> Option<RegisterSlot> {
        self.slots.get(slot.0 as usize).copied()
    }
    pub fn dominates(&self, a: PointId, b: PointId) -> bool {
        match (self.points.get(a.0 as usize), self.points.get(b.0 as usize)) {
            (Some(a), Some(b)) => a.before <= b.before && b.before < a.after,
            _ => false,
        }
    }
    pub fn readable_value(&self, usage: &UseBinding) -> Option<ValueId> {
        let state = self.state(usage.state)?;
        if !state.availability.is_available() {
            return None;
        }
        match state.kind {
            StateKind::Value(id) => Some(id),
            StateKind::Phi(id) => self.phis.get(id.0 as usize).map(|p| p.value),
            _ => None,
        }
    }
    pub fn width(&self, value: ValueId) -> Option<SlotWidth> {
        self.values
            .get(value.0 as usize)
            .and_then(|v| self.slot(v.slot))
            .map(|s| s.width)
    }
}

pub struct SsaMethod<'a> {
    pub(super) flow: crate::p2::FlowMethod<'a>,
    pub(super) facts: SsaFacts,
}
impl<'a> SsaMethod<'a> {
    pub fn facts(&self) -> &SsaFacts {
        &self.facts
    }
    pub fn execution(&self) -> crate::p2::ExecutionFacts<'_, 'a> {
        self.flow.facts()
    }
    pub fn evidence(&self) -> crate::p2::MethodEvidence<'_, 'a> {
        self.flow.evidence()
    }
    pub fn method_id(&self) -> crate::MethodId {
        self.flow.method_id()
    }
    pub fn class_id(&self) -> crate::ClassDefId {
        self.flow.class_id()
    }
}

pub struct SsaStop<'a> {
    pub(super) flow: crate::p2::FlowMethod<'a>,
    pub(super) error: super::SsaError,
    pub(super) partial: SsaPartial,
}
pub struct SsaPartial {
    pub(super) facts: SsaFacts,
}
impl<'a> SsaStop<'a> {
    pub fn error(&self) -> &super::SsaError {
        &self.error
    }
    pub fn execution(&self) -> crate::p2::ExecutionFacts<'_, 'a> {
        self.flow.facts()
    }
    pub fn evidence(&self) -> crate::p2::MethodEvidence<'_, 'a> {
        self.flow.evidence()
    }
    pub fn partial(&self) -> &SsaPartial {
        &self.partial
    }
}
impl SsaPartial {
    pub fn points(&self) -> &[StatePoint] {
        &self.facts.points
    }
    pub fn states(&self) -> &[StateDefinition] {
        &self.facts.states
    }
    pub fn uses(&self) -> &[UseBinding] {
        &self.facts.uses
    }
    pub fn phis(&self) -> &[Phi] {
        &self.facts.phis
    }
}
