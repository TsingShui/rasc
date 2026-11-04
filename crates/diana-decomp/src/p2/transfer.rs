//! Register, heap and lock obligations delivered to SSA/storage consumers.

use crate::CodeUnitOffset;

/// Normal definitions are simultaneous after capturing all old input slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadOrder {
    CaptureAllBeforeCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitRule {
    /// No register definition is produced.
    None,
    /// Definitions commit atomically at the operation on normal completion only.
    AtOperation,
    /// Definitions commit at the associated move-result after producer success.
    AfterResult,
}

/// A symbolic event outcome preserves possible heap activity even on failure.
/// It is not rollback, a concrete heap state or a claim that a write occurred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeapTransfer {
    Preserve,
    EventOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockTransfer {
    Preserve,
    Acquire,
    Release,
    /// An exceptional monitor-exit can leave a release outcome unproved.
    ReleaseOutcome,
}

/// Exceptional ingress does not apply normal definitions or overlap invalidation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExceptionFrame {
    PreserveBeforeOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalCompletion {
    pub(super) reads: ReadOrder,
    pub(super) commit: CommitRule,
    pub(super) commit_pc: Option<CodeUnitOffset>,
    pub(super) heap: HeapTransfer,
    pub(super) lock: LockTransfer,
}

impl NormalCompletion {
    pub fn read_order(self) -> ReadOrder {
        self.reads
    }
    pub fn commit(self) -> CommitRule {
        self.commit
    }
    pub fn commit_pc(self) -> Option<CodeUnitOffset> {
        self.commit_pc
    }
    pub fn heap(self) -> HeapTransfer {
        self.heap
    }
    pub fn lock(self) -> LockTransfer {
        self.lock
    }
}

/// A precise throwing position and pre-normal-commit frame transfer. Handler
/// identity/priority is on the associated exception edge, not recomputed here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExceptionCompletion {
    pub(super) throw_pc: CodeUnitOffset,
    pub(super) selection_pc: CodeUnitOffset,
    pub(super) frame: ExceptionFrame,
    pub(super) heap: HeapTransfer,
    pub(super) lock: LockTransfer,
}

impl ExceptionCompletion {
    pub fn throw_pc(self) -> CodeUnitOffset {
        self.throw_pc
    }
    pub fn selection_pc(self) -> CodeUnitOffset {
        self.selection_pc
    }
    pub fn frame(self) -> ExceptionFrame {
        self.frame
    }
    pub fn heap(self) -> HeapTransfer {
        self.heap
    }
    pub fn lock(self) -> LockTransfer {
        self.lock
    }
}
