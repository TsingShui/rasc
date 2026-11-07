//! Composes the established flow facts into regions, transfers, protection
//! deliveries and lock scopes.
//!
//! Original-target interfaces select enclosing candidates before children are
//! constructed; commitment unwinds that one bounded hierarchy. `stack` assembles
//! each selected part as a Sequence or explicit Graph. Protection, scope contexts,
//! ownership, transfers and complete condition alternatives then finalize once.
//!
//! Only shape planning lives here. The flow graph, dominance, loop, lock,
//! protection and delivery facts stay in their own modules and are consumed, not
//! recomputed: `compose` never reopens P5 or resolves a new edge.

mod accept;
mod candidates;
mod completion;
mod conditions;
mod hierarchy;
mod loop_exits;
pub(super) mod parts;
mod stack;
mod switch;
pub(super) mod trace;

use parts::{Part, PartCompletion, add};

use super::build::*;
use super::dominance::Structure;
use super::graph::Graph;
use super::ownership::{own, protect, scope_contexts};
use super::transfers::transfers;
use super::*;
use crate::p5::StorageMethod;

pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    s: &Structure,
) -> Result<(), RegionError> {
    loop_exits::construct(storage, f, g)?;
    let mut owners = Vec::new();
    for n in 0..f.nodes.len() {
        let node = NodeId(id(n)?);
        let mut members = Vec::new();
        push(&mut members, node)?;
        let follow = if g.outgoing[n].len() == 1 {
            Some(f.links[g.outgoing[n][0].0 as usize].to)
        } else {
            None
        };
        let region = add(
            f,
            RegionKind::Block(node),
            Some(node),
            follow,
            Vec::new(),
            members,
        )?;
        f.labels[n].owner = region;
        push(&mut owners, region)?;
    }
    let mut all = Vec::new();
    for n in 0..f.nodes.len() {
        push(&mut all, NodeId(id(n)?))?;
    }
    f.root = Some(hierarchy::construct(
        storage,
        f,
        g,
        s,
        &mut owners,
        Part {
            completion: PartCompletion::Ordinary,
            nodes: all,
            entry: Some(NodeId(0)),
            follow: None,
        },
    )?);
    protect(storage, f, g)?;
    scope_contexts(storage, f, g)?;
    for monitor in &mut f.monitors {
        if let MonitorMode::Natural(scope) = monitor.mode
            && !f
                .regions
                .iter()
                .any(|r| matches!(r.kind,RegionKind::Synchronized(id)if id==scope))
        {
            let entry = g.op_node[monitor.enter.0 as usize];
            let reason = f
                .issues
                .iter()
                .rev()
                .find_map(|issue| match issue {
                    PlanningIssue::Graph { entry: at, reason } if *at == entry => Some(*reason),
                    _ => None,
                })
                .ok_or(bug("unselected lock scope lost its separation reason"))?;
            let plan = &mut f.synchronized[scope.0 as usize];
            let reason = plan.separation.unwrap_or(LockSeparation::Region(reason));
            plan.separation = Some(reason);
            monitor.mode = MonitorMode::Explicit(reason);
        }
    }
    own(storage, f, g)?;
    transfers(f, g)?;
    super::condition_layout::construct(f, g)
}
