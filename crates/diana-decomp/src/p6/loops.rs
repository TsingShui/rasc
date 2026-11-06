use super::build::*;
use super::dominance::Structure;
use super::graph::Graph;
use super::*;
use crate::p2::{EdgeRole, NormalEdge, Terminal};
use crate::p5::StorageMethod;

pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    s: &Structure,
) -> Result<(), RegionError> {
    let n = f.nodes.len();
    let mut heads = lists(n)?;
    for link in &f.links {
        if s.dom.dominates(link.to.0 as usize, link.from.0 as usize) {
            push(&mut heads[link.to.0 as usize], link.id)?;
        }
    }
    for (header, latches) in heads.into_iter().enumerate() {
        if latches.is_empty() {
            continue;
        }
        let header = NodeId(id(header)?);
        let mut member = filled(n, false)?;
        member[header.0 as usize] = true;
        let mut queue = Vec::new();
        for &l in &latches {
            let from = f.links[l.0 as usize].from;
            if !member[from.0 as usize] {
                member[from.0 as usize] = true;
                push(&mut queue, from)?;
            }
        }
        while let Some(p) = queue.pop() {
            for &l in &g.incoming[p.0 as usize] {
                let q = f.links[l.0 as usize].from;
                if !member[q.0 as usize] && s.dom.dominates(header.0 as usize, q.0 as usize) {
                    member[q.0 as usize] = true;
                    push(&mut queue, q)?;
                }
            }
        }
        if g.external_entry(f, &member, header) {
            push(
                &mut f.issues,
                PlanningIssue::Graph {
                    entry: header,
                    reason: RecoveryReason::MultipleEntry,
                },
            )?;
            continue;
        }
        let mut members = Vec::new();
        for (i, &inside) in member.iter().enumerate() {
            if inside {
                push(&mut members, NodeId(id(i)?))?;
            }
        }
        let mut exits = Vec::new();
        let mut targets = Vec::new();
        for l in &f.links {
            if member[l.from.0 as usize] && !member[l.to.0 as usize] {
                push(&mut exits, l.id)?;
                if !targets.contains(&l.to) {
                    push(&mut targets, l.to)?;
                }
            }
        }
        let mut follow = if targets.len() == 1 {
            Some(targets[0])
        } else {
            s.follow(header).filter(|p| !member[p.0 as usize])
        };
        let mut form = LoopForm::Infinite;
        let condition_plan = |node: NodeId| -> Result<LoopCondition, RegionError> {
            let repeat = g.outgoing[node.0 as usize]
                .iter()
                .copied()
                .find(|l| member[f.links[l.0 as usize].to.0 as usize])
                .ok_or(bug("loop condition has no repeat transfer"))?;
            let exit = g.outgoing[node.0 as usize]
                .iter()
                .copied()
                .find(|l| !member[f.links[l.0 as usize].to.0 as usize])
                .ok_or(bug("loop condition has no exit transfer"))?;
            Ok(LoopCondition { node, repeat, exit })
        };
        let conditional = |node: NodeId| {
            f.nodes[node.0 as usize].terminal
                && matches!(
                    storage.execution().blocks()[f.nodes[node.0 as usize].block.0 as usize]
                        .terminal(),
                    Some(Terminal::Branch { .. })
                )
        };
        if conditional(header)
            && f.nodes[header.0 as usize].operations.is_empty()
            && g.outgoing[header.0 as usize].len() == 2
        {
            let inside = g.outgoing[header.0 as usize]
                .iter()
                .filter(|l| member[f.links[l.0 as usize].to.0 as usize])
                .count();
            if inside == 1 {
                form = LoopForm::While(condition_plan(header)?);
            }
        }
        if matches!(form, LoopForm::Infinite) && latches.len() == 1 {
            let latch = f.links[latches[0].0 as usize].from;
            if conditional(latch)
                && g.outgoing[latch.0 as usize].len() == 2
                && g.outgoing[latch.0 as usize]
                    .iter()
                    .any(|l| !member[f.links[l.0 as usize].to.0 as usize])
            {
                form = LoopForm::DoWhile(condition_plan(latch)?);
            }
        }
        // A conditional loop's normal continuation is its actual false/release
        // destination, not a postdominator beyond other exit-side operations.
        if let LoopForm::While(c) | LoopForm::DoWhile(c) = form {
            follow = Some(f.links[c.exit.0 as usize].to);
        }
        let loop_id = LoopId(id(f.loops.len())?);
        push(
            &mut f.loops,
            LoopPlan {
                id: loop_id,
                header,
                members,
                latches,
                exits,
                follow,
                form,
            },
        )?;
    }
    for (component, &cyclic) in s.cyclic.iter().enumerate() {
        if !cyclic {
            continue;
        }
        let mut members = Vec::new();
        for i in 0..n {
            if s.component[i] == component {
                push(&mut members, i)?;
            }
        }
        if !f.loops.iter().any(|l| {
            members
                .iter()
                .all(|&i| s.dom.dominates(l.header.0 as usize, i))
        }) && let Some(&entry) = members.first()
        {
            push(
                &mut f.issues,
                PlanningIssue::Graph {
                    entry: NodeId(id(entry)?),
                    reason: RecoveryReason::Irreducible,
                },
            )?;
        }
    }
    // Edge roles are retained on original links, not inferred from target equality.
    for loop_plan in &f.loops {
        if matches!(loop_plan.form, LoopForm::While(_)) {
            for &l in &g.outgoing[loop_plan.header.0 as usize] {
                if let LinkOrigin::Edge(e) = f.links[l.0 as usize].origin
                    && !matches!(
                        storage.execution().edges()[e.0 as usize].role(),
                        EdgeRole::Normal(
                            NormalEdge::ConditionalTrue | NormalEdge::ConditionalFalse
                        )
                    )
                {
                    return Err(bug("while condition lost original branch roles"));
                }
            }
        }
    }
    Ok(())
}
