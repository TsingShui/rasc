//! Why a proposal stopped being a region.
//!
//! P6 keeps a rejection reason only while planning; the published plan drops
//! them. Under the `trace-passes` feature the same decisions print as they are
//! made, so a reader can see which admission check refused a candidate and which
//! part fell back to an explicit graph. An ordinary build prints nothing.

/// Whether the `trace-passes` feature is compiled in.
pub const ENABLED: bool = cfg!(feature = "trace-passes");

/// One candidate that admission refused.
pub(super) fn rejected(
    entry: crate::p6::NodeId,
    reason: &crate::p6::RecoveryReason,
    members: usize,
    follow: Option<crate::p6::NodeId>,
    boundary: bool,
) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[p6] reject entry={} reason={:?} members={members} follow={} boundary={boundary}",
        entry.0,
        reason,
        follow.map_or(-1, |node| node.0 as i64)
    );
}

/// One part assembled as an explicit graph instead of a sequence.
pub(super) fn graph(
    entry: Option<crate::p6::NodeId>,
    reason: &crate::p6::RecoveryReason,
    nodes: usize,
    exported: usize,
    owners: usize,
) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[p6] graph entry={} reason={:?} nodes={nodes} exported={exported} owners={owners}",
        entry.map_or(-1, |node| node.0 as i64),
        reason
    );
}
