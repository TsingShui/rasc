//! Enumerates region proposals from established facts: established loops, try
//! plans, lock scopes, and one proposal per conditional or switch terminal. A
//! proposal states its parts and a convenient concrete follow inside the
//! enclosing completion boundary. `hierarchy` selects interfaces before children;
//! `accept` retains the original entrance, construction and scope constraints.

use super::super::build::*;
use super::super::dominance::Structure;
use super::super::graph::Graph;
use super::super::protection;
use super::super::*;
use super::conditions;
use super::parts::*;
use crate::p2::{EdgeRole, NormalEdge, Terminal};
use crate::p5::StorageMethod;

pub(super) fn branch_proposal(
    f: &RegionFacts,
    g: &Graph,
    entry: NodeId,
    true_link: LinkId,
    false_link: LinkId,
    follow: Option<NodeId>,
    boundary: &[bool],
) -> Result<Option<Candidate>, RegionError> {
    let t = f.links[true_link.0 as usize].to;
    let e = f.links[false_link.0 as usize].to;
    let a = super::completion::reachable(f, g, &[t], boundary, follow)?;
    let b = super::completion::reachable(f, g, &[e], boundary, follow)?;
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
                entry: if Some(target) == follow || boundary[target.0 as usize] {
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
    scope: &Part,
    boundary: &[bool],
) -> Result<Vec<Candidate>, RegionError> {
    let mut out = Vec::new();
    for l in &f.loops {
        if !l.lexical_members.iter().all(|n| scope.nodes.contains(n)) {
            continue;
        }
        let members = copy(&l.lexical_members)?;
        let mut parts = Vec::new();
        let mut body = Vec::new();
        for &node in &members {
            if !l.evaluation.contains(&node) {
                push(&mut body, node)?;
            }
        }
        let entry = if matches!(l.form, LoopForm::While(_)) {
            match l.form {
                LoopForm::While(condition) => {
                    let target = f.links[condition.repeat.0 as usize].to;
                    body.contains(&target).then_some(target)
                }
                _ => None,
            }
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
        if l.condition().is_some() {
            push(
                &mut parts,
                Part {
                    completion: PartCompletion::Ordinary,
                    nodes: copy(&l.evaluation)?,
                    entry: l.evaluation.first().copied(),
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
        if !p.members.iter().all(|n| scope.nodes.contains(n)) {
            continue;
        }
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
        if !p.members.iter().all(|n| scope.nodes.contains(n)) {
            continue;
        }
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
    for index in 0..f.nodes.len() {
        if !scope.nodes.contains(&NodeId(id(index)?)) || !f.nodes[index].terminal {
            continue;
        }
        let n = f.nodes[index].id();
        let block = f.nodes[index].block();
        let terminal = storage.execution().blocks()[block.0 as usize].terminal();
        if !matches!(
            terminal,
            Some(Terminal::Branch { .. } | Terminal::Switch { .. })
        ) {
            continue;
        }
        if storage
            .facts()
            .terminal_uses(block)
            .is_none_or(|uses| uses.iter().any(|u| u.read().is_err()))
        {
            continue;
        }
        if f.loops.iter().any(|l| l.condition() == Some(n)) {
            continue;
        }
        let follow = s
            .follow(n)
            .filter(|node| scope.nodes.contains(node))
            .or(scope.follow);
        if matches!(terminal, Some(Terminal::Branch { .. })) {
            if let Some(proposal) = conditions::candidate(storage, f, g, follow, boundary, n)? {
                push(&mut out, proposal)?;
                continue;
            }
            let mut yes = None;
            let mut no = None;
            for &l in &g.outgoing[n.0 as usize] {
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
            if let Some(proposal) = branch_proposal(f, g, n, yes, no, follow, boundary)? {
                let set = mask(f.nodes.len(), &proposal.members)?;
                if protection::closed_entry(storage, f, g, &set, n) {
                    push(&mut out, proposal)?;
                    continue;
                }
            }
            let t = f.links[yes.0 as usize].to;
            let e = f.links[no.0 as usize].to;
            if t != e {
                for (successor, link) in [
                    (SuccessorBoundary::True(yes), yes),
                    (SuccessorBoundary::False(no), no),
                ] {
                    let follow = Some(f.links[link.0 as usize].to);
                    if let Some(mut proposal) = branch_proposal(f, g, n, yes, no, follow, boundary)?
                    {
                        proposal.successor_boundary = Some(successor);
                        push(&mut out, proposal)?;
                    }
                }
            }
            continue;
        }
        let mut parts = Vec::new();
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: copy(&[n])?,
                entry: Some(n),
                follow: None,
            },
        )?;
        let arms = copy(&g.outgoing[n.0 as usize])?;
        let mut starts = Vec::new();
        for &l in &arms {
            push(&mut starts, f.links[l.0 as usize].to)?;
        }
        let follow = match s.follow(n).filter(|node| scope.nodes.contains(node)) {
            Some(follow) => Some(follow),
            None => super::completion::shared_target(f, g, &starts, boundary)?.or(scope.follow),
        };
        let body = super::completion::reachable(f, g, &starts, boundary, follow)?;
        if body[n.0 as usize] {
            continue;
        }
        let mut set = copy(&body)?;
        set[n.0 as usize] = true;
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
                entry: n,
                follow,
                members,
                parts,
                successor_boundary: None,
            },
        )?;
    }
    Ok(out)
}
