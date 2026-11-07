//! Establish lexical loop content without changing natural backedge membership.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::*;
use super::parts::{copy, mask};
use crate::p2::{OpKind, Terminal};
use crate::p5::StorageMethod;

pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
) -> Result<(), RegionError> {
    for index in 0..f.loops.len() {
        let plan = &f.loops[index];
        let mut members = copy(&plan.members)?;
        let mut private_exits = Vec::new();
        for scope in &f.synchronized {
            if scope.separation.is_none()
                && plan.members.contains(&g.op_node[scope.enter.0 as usize])
                && !scope.members.contains(&plan.header)
                && plan.follow.is_none_or(|n| !scope.members.contains(&n))
            {
                for &node in &scope.members {
                    if !members.contains(&node) {
                        push(&mut members, node)?;
                    }
                }
            }
        }
        // The predicate's own exit remains the real normal continuation. Only
        // other exits can contribute private abrupt corridors to the body.
        if let LoopForm::While(condition) | LoopForm::DoWhile(condition) = plan.form {
            for &exit in &plan.exits {
                if exit == condition.exit {
                    continue;
                }
                if let Some(corridor) = corridor(storage, f, g, index, exit)? {
                    push(&mut private_exits, exit)?;
                    for node in corridor {
                        if !members.contains(&node) {
                            push(&mut members, node)?;
                        }
                    }
                }
            }
        }
        members.sort_unstable();
        f.loops[index].lexical_members = members;
        f.loops[index].private_exits = private_exits;
    }
    Ok(())
}

fn corridor(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    index: usize,
    exit: LinkId,
) -> Result<Option<Vec<NodeId>>, RegionError> {
    let plan = &f.loops[index];
    let mut nodes = Vec::new();
    let mut at = f.links[exit.0 as usize].to;
    loop {
        if plan.members.contains(&at)
            || plan.follow == Some(at)
            || nodes.contains(&at)
            || f.loops.iter().any(|other| other.members.contains(&at))
            || f.tries.iter().any(|scope| scope.members.contains(&at))
            || f.synchronized
                .iter()
                .any(|scope| scope.members.contains(&at))
        {
            return Ok(None);
        }
        let node = &f.nodes[at.0 as usize];
        if node.operations.clone().any(|op| {
            matches!(
                storage.execution().operations()[op as usize].kind(),
                OpKind::Monitor { .. }
            )
        }) || f.throws.iter().any(|site| {
            site.node == at
                && f.chains[site.chain.0 as usize]
                    .clauses
                    .iter()
                    .any(|clause| matches!(clause.target, CatchTarget::Handler(_)))
        }) {
            return Ok(None);
        }
        push(&mut nodes, at)?;
        match g.outgoing[at.0 as usize].as_slice() {
            [] if node.terminal
                && matches!(
                    storage.execution().blocks()[node.block.0 as usize].terminal(),
                    Some(Terminal::Return { .. } | Terminal::Throw { .. })
                ) =>
            {
                break;
            }
            [link] => at = f.links[link.0 as usize].to,
            _ => return Ok(None),
        }
    }
    let set = mask(f.nodes.len(), &nodes)?;
    for &node in &nodes {
        if node == NodeId(0)
            || g.incoming[node.0 as usize].iter().any(|link| {
                let link = f.links[link.0 as usize];
                !set[link.from.0 as usize] && link.id != exit
            })
        {
            return Ok(None);
        }
    }
    // A normal body arm cannot own an exception-arrival path. Keep handler
    // sharing explicit even when its destination happens to be an abrupt node.
    for &edge in g.exceptions.iter().flatten() {
        if Graph::exception_possible(f, edge)
            && Graph::exception_target(f, storage, edge).is_some_and(|n| set[n.0 as usize])
        {
            return Ok(None);
        }
    }
    for group in storage.facts().construction_groups() {
        if group
            .members()
            .iter()
            .any(|op| set[g.op_node[op.0 as usize].0 as usize])
            && group.members().iter().any(|op| {
                let node = g.op_node[op.0 as usize];
                !set[node.0 as usize] && !plan.members.contains(&node)
            })
        {
            return Ok(None);
        }
    }
    Ok(Some(nodes))
}
