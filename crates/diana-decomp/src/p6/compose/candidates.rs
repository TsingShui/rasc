//! Enumerates region proposals from established facts: established loops, try
//! plans, lock scopes, and one proposal per conditional or switch terminal. A
//! proposal states its parts and its follow; whether it may be selected is
//! decided by `accept`.

use super::super::build::*;
use super::super::dominance::Structure;
use super::super::graph::Graph;
use super::super::protection;
use super::super::*;
use super::parts::*;
use crate::p2::{EdgeRole, NormalEdge, Terminal};
use crate::p5::StorageMethod;

pub(super) fn branch_proposal(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    entry: NodeId,
    true_link: LinkId,
    false_link: LinkId,
    follow: Option<NodeId>,
) -> Result<Option<Candidate>, RegionError> {
    let empty = filled(f.nodes.len(), false)?;
    let t = f.links[true_link.0 as usize].to;
    let e = f.links[false_link.0 as usize].to;
    let a = protection::reachable(storage, f, g, &[t], follow, &empty, false)?;
    let b = protection::reachable(storage, f, g, &[e], follow, &empty, false)?;
    if a[entry.0 as usize] || b[entry.0 as usize] || a.iter().zip(&b).any(|(a, b)| *a && *b) {
        return Ok(None);
    }
    let mut set = filled(f.nodes.len(), false)?;
    set[entry.0 as usize] = true;
    for i in 0..set.len() {
        set[i] |= a[i] || b[i];
    }
    let mut parts = Vec::new();
    push(
        &mut parts,
        Part {
            completion: PartCompletion::Ordinary,
            nodes: copy(&[entry])?,
            entry: Some(entry),
            follow: None,
        },
    )?;
    for (target, arm) in [(t, &a), (e, &b)] {
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: list(arm)?,
                entry: if Some(target) == follow {
                    None
                } else {
                    Some(target)
                },
                follow,
            },
        )?;
    }
    Ok(Some(Candidate {
        kind: RegionKind::If {
            true_link,
            false_link,
        },
        entry,
        follow,
        members: list(&set)?,
        parts,
        successor_boundary: None,
    }))
}
pub(super) fn candidates(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    s: &Structure,
) -> Result<Vec<Candidate>, RegionError> {
    let mut out = Vec::new();
    for l in &f.loops {
        // Backedge membership is not the lexical body: release/break corridors
        // can lie outside that core. Include a closed scope atomically; all
        // original entrances and edges are still checked by candidate acceptance.
        let mut members = copy(&l.members)?;
        for scope in &f.synchronized {
            if scope.separation.is_none()
                && l.members.contains(&g.op_node[scope.enter.0 as usize])
                && !scope.members.contains(&l.header)
                && l.follow.is_none_or(|n| !scope.members.contains(&n))
            {
                for &node in &scope.members {
                    if !members.contains(&node) {
                        push(&mut members, node)?;
                    }
                }
            }
        }
        members.sort_unstable();
        let mut parts = Vec::new();
        let mut body = Vec::new();
        for &node in &members {
            if Some(node) != l.condition() {
                push(&mut body, node)?;
            }
        }
        let entry = if matches!(l.form, LoopForm::While(_)) {
            g.outgoing[l.header.0 as usize]
                .iter()
                .map(|e| f.links[e.0 as usize].to)
                .find(|p| body.contains(p))
        } else {
            body.contains(&l.header).then_some(l.header)
        };
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: body,
                entry,
                follow: Some(l.continue_target()),
            },
        )?;
        if let Some(condition) = l.condition() {
            push(
                &mut parts,
                Part {
                    completion: PartCompletion::Ordinary,
                    nodes: copy(&[condition])?,
                    entry: Some(condition),
                    follow: None,
                },
            )?;
        }
        push(
            &mut out,
            Candidate {
                kind: RegionKind::Loop(l.id),
                entry: l.header,
                follow: l.follow,
                members,
                parts,
                successor_boundary: None,
            },
        )?;
    }
    for p in &f.tries {
        let mut parts = Vec::new();
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: copy(&p.body)?,
                entry: Some(p.entry),
                follow: p.follow,
            },
        )?;
        for (&entry, body) in p.handlers.iter().zip(&p.handler_bodies) {
            push(
                &mut parts,
                Part {
                    completion: PartCompletion::Ordinary,
                    nodes: copy(body)?,
                    entry: Some(entry),
                    follow: p.follow,
                },
            )?;
        }
        push(
            &mut out,
            Candidate {
                kind: RegionKind::Try(p.id),
                entry: p.entry,
                follow: p.follow,
                members: copy(&p.members)?,
                parts,
                successor_boundary: None,
            },
        )?;
    }
    for p in &f.synchronized {
        if p.separation.is_some() {
            continue;
        }
        let entry = g.op_node[p.enter.0 as usize];
        let mut parts = Vec::new();
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: copy(&[entry])?,
                entry: Some(entry),
                follow: None,
            },
        )?;
        let body_entry = f.links[p.enter_link.0 as usize].to;
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: copy(&p.body)?,
                entry: Some(body_entry),
                follow: p.follow,
            },
        )?;
        push(
            &mut out,
            Candidate {
                kind: RegionKind::Synchronized(p.id),
                entry,
                follow: p.follow,
                members: copy(&p.members)?,
                parts,
                successor_boundary: None,
            },
        )?;
    }
    for n in &f.nodes {
        if !n.terminal {
            continue;
        }
        let terminal = storage.execution().blocks()[n.block.0 as usize].terminal();
        if !matches!(
            terminal,
            Some(Terminal::Branch { .. } | Terminal::Switch { .. })
        ) {
            continue;
        }
        if storage
            .facts()
            .terminal_uses(n.block)
            .is_none_or(|uses| uses.iter().any(|u| u.read().is_err()))
        {
            continue;
        }
        if f.loops.iter().any(|l| l.condition() == Some(n.id)) {
            continue;
        }
        let follow = s.follow(n.id);
        if matches!(terminal, Some(Terminal::Branch { .. })) {
            let mut yes = None;
            let mut no = None;
            for &l in &g.outgoing[n.id.0 as usize] {
                if let LinkOrigin::Edge(e) = f.links[l.0 as usize].origin {
                    match storage.execution().edges()[e.0 as usize].role() {
                        EdgeRole::Normal(NormalEdge::ConditionalTrue) => yes = Some(l),
                        EdgeRole::Normal(NormalEdge::ConditionalFalse) => no = Some(l),
                        _ => {}
                    }
                }
            }
            let (Some(yes), Some(no)) = (yes, no) else {
                return Err(bug("conditional terminal lost its original arms"));
            };
            if let Some(proposal) = branch_proposal(storage, f, g, n.id, yes, no, follow)? {
                let set = mask(f.nodes.len(), &proposal.members)?;
                if protection::closed_entry(storage, f, g, &set, n.id) {
                    push(&mut out, proposal)?;
                    continue;
                }
            }
            let t = f.links[yes.0 as usize].to;
            let e = f.links[no.0 as usize].to;
            if t != e {
                for (boundary, link) in [
                    (SuccessorBoundary::True(yes), yes),
                    (SuccessorBoundary::False(no), no),
                ] {
                    let follow = Some(f.links[link.0 as usize].to);
                    if let Some(mut proposal) =
                        branch_proposal(storage, f, g, n.id, yes, no, follow)?
                    {
                        proposal.successor_boundary = Some(boundary);
                        push(&mut out, proposal)?;
                    }
                }
            }
            continue;
        }
        let empty = filled(f.nodes.len(), false)?;
        let mut parts = Vec::new();
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: copy(&[n.id])?,
                entry: Some(n.id),
                follow: None,
            },
        )?;
        let arms = copy(&g.outgoing[n.id.0 as usize])?;
        let mut starts = Vec::new();
        for &l in &arms {
            push(&mut starts, f.links[l.0 as usize].to)?;
        }
        let body = protection::reachable(storage, f, g, &starts, follow, &empty, false)?;
        if body[n.id.0 as usize] {
            continue;
        }
        let mut set = copy(&body)?;
        set[n.id.0 as usize] = true;
        let members = list(&set)?;
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: list(&body)?,
                entry: starts.iter().copied().find(|n| Some(*n) != follow),
                follow,
            },
        )?;
        let kind = RegionKind::Switch {
            arms,
            private_cases: false,
        };
        push(
            &mut out,
            Candidate {
                kind,
                entry: n.id,
                follow,
                members,
                parts,
                successor_boundary: None,
            },
        )?;
    }
    out.sort_unstable_by_key(|c| {
        (
            c.members.len(),
            match c.kind {
                RegionKind::Loop(_) => 0,
                RegionKind::Synchronized(_) => 1,
                RegionKind::Try(_) => 2,
                RegionKind::If { .. } => 3,
                _ => 4,
            },
            match c.kind {
                RegionKind::Try(id) => usize::MAX - f.tries[id.0 as usize].handlers.len(),
                _ => 0,
            },
            c.entry.0,
            match c.successor_boundary {
                None => (0, c.follow.map(|n| n.0), 0),
                Some(SuccessorBoundary::True(link)) => (1, c.follow.map(|n| n.0), link.0),
                Some(SuccessorBoundary::False(link)) => (2, c.follow.map(|n| n.0), link.0),
            },
            match c.kind {
                RegionKind::Loop(id) => id.0,
                RegionKind::Synchronized(id) => id.0,
                RegionKind::Try(id) => id.0,
                _ => 0,
            },
        )
    });
    // One original predicate and boundary determine the same reach sets/parts.
    out.dedup_by(|a, b| {
        a.successor_boundary.is_some()
            && b.successor_boundary.is_some()
            && a.entry == b.entry
            && a.follow == b.follow
    });
    Ok(out)
}
