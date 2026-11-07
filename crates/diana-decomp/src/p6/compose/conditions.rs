//! Recognizes one ordered predicate region whose tests select a single shared
//! result arm.
//!
//! Both original arms are compared inside the enclosing completion boundary.
//! One is a proper subset of the other, every later test selects the same shared
//! head, and intervening operations stay in their original block runs. Original
//! abrupt terminals need no concrete common postdominator. Entrance, exceptional
//! child closure and owning-part checks remain independent admission constraints.

use super::super::build::*;
use super::super::graph::Graph;
use super::super::protection;
use super::super::*;
use super::parts::*;
use crate::p5::StorageMethod;

fn subset(a: &[bool], b: &[bool]) -> bool {
    a.iter().zip(b).all(|(a, b)| !*a || *b)
}

/// The part run of `entry`'s original block, from `entry` to its terminal. Only
/// the last part owns the terminal; an edge into a block's middle is refused.
fn run(f: &RegionFacts, entry: NodeId) -> Result<Option<Vec<NodeId>>, RegionError> {
    let Some(block) = f.nodes.get(entry.0 as usize).map(|node| node.block()) else {
        return Ok(None);
    };
    let Some(parts) = f.blocks.get(block.0 as usize).map(|plan| plan.parts()) else {
        return Ok(None);
    };
    let Some(at) = parts.iter().position(|n| *n == entry) else {
        return Ok(None);
    };
    let Some((last, head)) = parts.get(at..).and_then(|run| run.split_last()) else {
        return Ok(None);
    };
    if !f
        .nodes
        .get(last.0 as usize)
        .is_some_and(|node| node.terminal)
    {
        return Ok(None);
    }
    if head
        .iter()
        .any(|n| f.nodes.get(n.0 as usize).is_none_or(|n| n.terminal))
    {
        return Ok(None);
    }
    let mut out = Vec::new();
    for &node in head.iter().chain([last]) {
        push(&mut out, node)?;
    }
    Ok(Some(out))
}

/// Every original link entering `set` from outside participates in the given
/// condition interface. A cross-arm entrance is not a member of that interface.
fn entered_by(f: &RegionFacts, set: &[bool], entries: &[LinkId]) -> bool {
    f.links.iter().all(|link| {
        !set[link.to().0 as usize] || set[link.from().0 as usize] || entries.contains(&link.id())
    })
}

/// Every original link entering `set` from outside targets its one entry.
fn enters_at(f: &RegionFacts, set: &[bool], entry: NodeId) -> bool {
    f.links
        .iter()
        .all(|link| !set[link.to().0 as usize] || set[link.from().0 as usize] || link.to() == entry)
}

/// Every original link leaving `set` reaches one of its declared successors.
fn exits_to(f: &RegionFacts, set: &[bool], exits: &[NodeId]) -> bool {
    f.links.iter().all(|link| {
        !set[link.from().0 as usize] || set[link.to().0 as usize] || exits.contains(&link.to())
    })
}

fn completion_exits(
    f: &RegionFacts,
    set: &[bool],
    follow: Option<NodeId>,
    boundary: &[bool],
) -> bool {
    f.links.iter().all(|link| {
        !set[link.from.0 as usize]
            || set[link.to.0 as usize]
            || Some(link.to) == follow
            || boundary[link.to.0 as usize]
    })
}

/// The two original conditional links of `node`.
fn conditional(f: &RegionFacts, g: &Graph, node: NodeId) -> Option<[LinkId; 2]> {
    let [a, b] = g.outgoing.get(node.0 as usize)?.as_slice() else {
        return None;
    };
    for link in [a, b] {
        if !matches!(f.links.get(link.0 as usize)?.origin(), LinkOrigin::Edge(_)) {
            return None;
        }
    }
    Some([*a, *b])
}

fn target(f: &RegionFacts, link: LinkId) -> NodeId {
    f.links[link.0 as usize].to()
}

/// The unique member of `set` that no other member links into.
fn head(f: &RegionFacts, set: &[bool]) -> Option<NodeId> {
    let mut entry = None;
    for (n, &inside) in set.iter().enumerate() {
        if !inside {
            continue;
        }
        let node = NodeId(id(n).ok()?);
        let mut owned = false;
        for link in &f.links {
            if link.from() != node && set[link.from().0 as usize] && link.to() == node {
                owned = true;
                break;
            }
        }
        if !owned {
            if entry.is_some() {
                return None;
            }
            entry = Some(node);
        }
    }
    entry
}

/// Reachability restricted to `set`, stopping at `follow`. The produced set must
/// equal `set` for `root` to be that arm's sole entry and owner.
fn closed_arm(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    root: NodeId,
    set: &[bool],
    follow: Option<NodeId>,
    boundary: &[bool],
) -> Result<bool, RegionError> {
    let mut excluded = filled(set.len(), false)?;
    for (i, &inside) in set.iter().enumerate() {
        excluded[i] = !inside;
    }
    for (index, &stop) in boundary.iter().enumerate() {
        excluded[index] |= stop;
    }
    let reached = protection::reachable(storage, f, g, &[root], follow, &excluded, false)?;
    Ok(reached.iter().zip(set).all(|(a, b)| a == b))
}

/// Exceptional arrivals do not select a condition edge. Only an arrival at the
/// whole condition's entry from outside it, or one internal to the same child,
/// preserves the child's established ownership. Outward propagation is unchanged.
fn exceptional_interfaces(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    parts: &[Part],
    members: &[bool],
    entry: NodeId,
) -> Result<bool, RegionError> {
    let mut child = filled(f.nodes.len(), None)?;
    for (index, part) in parts.iter().enumerate() {
        for node in &part.nodes {
            child[node.0 as usize] = Some(index);
        }
    }
    for (from, edges) in g.exceptions.iter().enumerate() {
        for &edge in edges {
            if !Graph::exception_possible(f, edge) {
                continue;
            }
            let Some(to) = Graph::exception_target(f, storage, edge) else {
                continue;
            };
            if let Some(target_child) = child[to.0 as usize]
                && child[from] != Some(target_child)
                && (members[from] || target_child != 0 || to != entry)
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn candidate(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    follow: Option<NodeId>,
    boundary: &[bool],
    predicate: NodeId,
) -> Result<Option<Candidate>, RegionError> {
    if follow == Some(predicate) {
        return Ok(None);
    }
    let Some([yes, no]) = conditional(f, g, predicate) else {
        return Ok(None);
    };
    let first = super::completion::reachable(f, g, &[target(f, yes)], boundary, follow)?;
    let second = super::completion::reachable(f, g, &[target(f, no)], boundary, follow)?;
    // The shared arm is the smaller side; the other side carries the remaining
    // tests and the other result arm. A real if/else has two distinct arms and
    // overlapping but non-nested reach sets, so it never selects here.
    if !first.iter().any(|&x| x) {
        return Ok(None);
    }
    let (shared, rest, select, other) = if subset(&first, &second) && !subset(&second, &first) {
        (first, second, yes, no)
    } else if subset(&second, &first) && !subset(&first, &second) {
        (second, first, no, yes)
    } else {
        return Ok(None);
    };
    let Some(shared_entry) = head(f, &shared) else {
        return Ok(None);
    };
    if shared[predicate.0 as usize]
        || target(f, select) != shared_entry
        || !closed_arm(storage, f, g, shared_entry, &shared, follow, boundary)?
    {
        return Ok(None);
    }
    let first_part = f.blocks[f.nodes[predicate.0 as usize].block().0 as usize]
        .parts()
        .first()
        .copied()
        .ok_or(bug("condition predicate has no block run"))?;
    let Some(first_run) = run(f, first_part)? else {
        return Ok(None);
    };
    if first_run.last() != Some(&predicate) {
        return Ok(None);
    }
    let start = first_run[0];
    let mut members = filled(f.nodes.len(), false)?;
    let mut seen = filled(f.nodes.len(), false)?;
    let mut parts = Vec::new();
    for &n in &first_run {
        members[n.0 as usize] = true;
        seen[n.0 as usize] = true;
    }
    // A test run is a straight sequence on this schedule: no foreign entrance
    // reaches its middle and no link leaves it except the terminal's two
    // original tests. Otherwise the run is not the promised ordered portion.
    let run_set = mask(f.nodes.len(), &first_run)?;
    if !enters_at(f, &run_set, start) || !exits_to(f, &run_set, &[shared_entry, target(f, other)]) {
        return Ok(None);
    }
    push(
        &mut parts,
        Part {
            completion: PartCompletion::Ordinary,
            nodes: first_run,
            entry: Some(start),
            follow: None,
        },
    )?;
    let mut tests = Vec::new();
    push(
        &mut tests,
        ConditionTest {
            predicate,
            select,
            other,
        },
    )?;
    let mut cursor = target(f, other);
    loop {
        if Some(cursor) == follow || boundary[cursor.0 as usize] {
            break;
        }
        if shared[cursor.0 as usize] || seen[cursor.0 as usize] {
            return Ok(None);
        }
        // A run that does not end in another conditional starts the other
        // result arm; its nodes are collected below from the larger side.
        let Some(nodes) = run(f, cursor)? else {
            break;
        };
        let Some(&test) = nodes.last() else {
            return Ok(None);
        };
        let Some([a, b]) = conditional(f, g, test) else {
            break;
        };
        let (selected, continued) =
            match (target(f, a) == shared_entry, target(f, b) == shared_entry) {
                (true, false) => (a, b),
                (false, true) => (b, a),
                _ => return Ok(None),
            };
        if shared[target(f, continued).0 as usize] {
            return Ok(None);
        }
        let next = target(f, continued);
        let run_set = mask(f.nodes.len(), &nodes)?;
        if !enters_at(f, &run_set, cursor) || !exits_to(f, &run_set, &[shared_entry, next]) {
            return Ok(None);
        }
        push(
            &mut tests,
            ConditionTest {
                predicate: test,
                select: selected,
                other: continued,
            },
        )?;
        for &n in &nodes {
            if members[n.0 as usize] {
                return Ok(None);
            }
            members[n.0 as usize] = true;
            seen[n.0 as usize] = true;
        }
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes,
                entry: Some(cursor),
                follow: None,
            },
        )?;
        cursor = target(f, continued);
        if tests.len() > f.nodes.len() {
            return Err(bug("condition chain did not terminate"));
        }
    }
    // The shared arm is printed last but owned by the same proposal; every other
    // member of the larger side must be one test run or the remaining arm.
    let mut remaining = Vec::new();
    for (n, &inside) in rest.iter().enumerate() {
        if inside && !members[n] && !shared[n] {
            push(&mut remaining, NodeId(id(n)?))?;
        }
    }
    // The shared result arm is one closed interface: only the tests' selected
    // original links enter it, and it leaves only at the common follow. In
    // particular, the other result arm cannot jump into a natural shared body.
    let mut selects = Vec::new();
    for test in &tests {
        push(&mut selects, test.select)?;
    }
    if !entered_by(f, &shared, &selects) || !completion_exits(f, &shared, follow, boundary) {
        return Ok(None);
    }
    let shared_nodes = list(&shared)?;
    for &n in &shared_nodes {
        members[n.0 as usize] = true;
    }
    push(
        &mut parts,
        Part {
            completion: PartCompletion::Ordinary,
            nodes: shared_nodes,
            entry: Some(shared_entry),
            follow,
        },
    )?;
    if Some(cursor) == follow || boundary[cursor.0 as usize] {
        if !remaining.is_empty() {
            return Ok(None);
        }
    } else {
        let mut set = filled(f.nodes.len(), false)?;
        for &n in &remaining {
            set[n.0 as usize] = true;
        }
        if remaining.is_empty()
            || !set[cursor.0 as usize]
            || !entered_by(
                f,
                &set,
                &[tests.last().ok_or(bug("condition has no test"))?.other],
            )
            || !completion_exits(f, &set, follow, boundary)
            || !closed_arm(storage, f, g, cursor, &set, follow, boundary)?
        {
            return Ok(None);
        }
        for &n in &remaining {
            members[n.0 as usize] = true;
        }
        push(
            &mut parts,
            Part {
                completion: PartCompletion::Ordinary,
                nodes: remaining,
                entry: Some(cursor),
                follow,
            },
        )?;
    }
    if !exceptional_interfaces(storage, f, g, &parts, &members, start)? {
        return Ok(None);
    }
    let id = ConditionId(id(f.conditions.len())?);
    push(
        &mut f.conditions,
        ConditionPlan {
            id,
            tests,
            shared: shared_entry,
            else_arm: Some(cursor) != follow && !boundary[cursor.0 as usize],
            explicit: None,
        },
    )?;
    Ok(Some(Candidate {
        kind: RegionKind::Condition(id),
        entry: start,
        follow,
        members: list(&members)?,
        parts,
        successor_boundary: None,
    }))
}
