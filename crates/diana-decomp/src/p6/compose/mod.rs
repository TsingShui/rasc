//! Composes the established flow facts into regions, transfers, protection
//! deliveries and lock scopes.
//!
//! Construction is one fixed pass order: block regions, then every proposal in
//! its acceptance order, then the root, then protection, lock contexts,
//! ownership and transfers. A proposal names original nodes and parts; a part
//! becomes a region through `stack`, which either keeps one contiguous owner or
//! assembles a sequence. Only an unstructured part becomes an explicit graph,
//! and only a graph keeps a labelled block.
//!
//! Only shape planning lives here. The flow graph, dominance, loop, lock,
//! protection and delivery facts stay in their own modules and are consumed, not
//! recomputed: `compose` never reopens P5 or resolves a new edge.

mod accept;
mod candidates;
mod conditions;
mod loop_exits;
pub(super) mod parts;
mod stack;
mod switch;

use parts::{Part, PartCompletion, add, copy};
use stack::compose;

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
    for mut c in candidates::candidates(storage, f, g, s)? {
        let acceptance = match accept::accept(storage, f, g, &owners, &c)? {
            Ok(()) => match accept::match_branch_loops(f, &c)? {
                Ok(()) => accept::match_successor_boundary(f, &owners, &c)?,
                Err(reason) => Err(reason),
            },
            Err(reason) => Err(reason),
        };
        if let Err(reason) = acceptance {
            push(
                &mut f.issues,
                PlanningIssue::Graph {
                    entry: c.entry,
                    reason,
                },
            )?;
            continue;
        }
        if let Some(parts) = switch::private_switch_parts(storage, f, g, &owners, &c)? {
            let old = std::mem::replace(&mut c.parts, parts);
            if accept::accept(storage, f, g, &owners, &c)?.is_ok() {
                if let RegionKind::Switch { private_cases, .. } = &mut c.kind {
                    *private_cases = true;
                }
            } else {
                c.parts = old;
            }
        }
        let mut children = Vec::new();
        for (i, mut p) in c.parts.into_iter().enumerate() {
            if i == 0
                && let RegionKind::Loop(id) = c.kind
            {
                p.completion = PartCompletion::LoopBody(id);
            }
            push(&mut children, compose(storage, f, g, &mut owners, p)?)?;
        }
        let region = add(
            f,
            c.kind,
            Some(c.entry),
            c.follow,
            children,
            copy(&c.members)?,
        )?;
        for n in c.members {
            owners[n.0 as usize] = region;
        }
    }
    let mut all = Vec::new();
    for n in 0..f.nodes.len() {
        push(&mut all, NodeId(id(n)?))?;
    }
    f.root = Some(compose(
        storage,
        f,
        g,
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
