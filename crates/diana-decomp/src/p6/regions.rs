use super::build::*;
use super::dominance::Structure;
use super::graph::Graph;
use super::protection;
use super::*;
use crate::p2::{EdgeRole, NormalEdge, Terminal};
use crate::p5::{OperationEmission, StorageMethod};

fn copy<T: Copy>(source: &[T]) -> Result<Vec<T>, RegionError> {
    let mut out = Vec::new();
    crate::substrate::owned::reserve(&mut out, source.len())?;
    out.extend_from_slice(source);
    Ok(out)
}
fn mask(n: usize, nodes: &[NodeId]) -> Result<Vec<bool>, RegionError> {
    let mut out = filled(n, false)?;
    for p in nodes {
        out[p.0 as usize] = true;
    }
    Ok(out)
}
fn list(set: &[bool]) -> Result<Vec<NodeId>, RegionError> {
    let mut out = Vec::new();
    for (i, &inside) in set.iter().enumerate() {
        if inside {
            push(&mut out, NodeId(id(i)?))?;
        }
    }
    Ok(out)
}
#[derive(Clone, Copy)]
enum PartCompletion {
    Ordinary,
    LoopBody(LoopId),
}
struct Part {
    nodes: Vec<NodeId>,
    entry: Option<NodeId>,
    follow: Option<NodeId>,
    completion: PartCompletion,
}
#[derive(Clone, Copy)]
enum SuccessorBoundary {
    True(LinkId),
    False(LinkId),
}
struct Candidate {
    kind: RegionKind,
    entry: NodeId,
    follow: Option<NodeId>,
    members: Vec<NodeId>,
    parts: Vec<Part>,
    successor_boundary: Option<SuccessorBoundary>,
}
fn add(
    f: &mut RegionFacts,
    kind: RegionKind,
    entry: Option<NodeId>,
    follow: Option<NodeId>,
    children: Vec<RegionId>,
    nodes: Vec<NodeId>,
) -> Result<RegionId, RegionError> {
    let region = RegionId(id(f.regions.len())?);
    for &child in &children {
        if f.parents[child.0 as usize].is_some() {
            return Err(bug("region child has two execution owners"));
        }
    }
    push(&mut f.parents, None)?;
    for &child in &children {
        f.parents[child.0 as usize] = Some(region);
    }
    push(
        &mut f.regions,
        Region {
            id: region,
            kind,
            entry,
            follow,
            children,
            nodes,
            continuation: Continuation::None,
        },
    )?;
    Ok(region)
}
fn entries(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    set: &[bool],
    entry: Option<NodeId>,
) -> Result<Vec<NodeId>, RegionError> {
    let mut out = Vec::new();
    if let Some(p) = entry {
        push(&mut out, p)?;
    }
    if set[0] && !out.contains(&NodeId(0)) {
        push(&mut out, NodeId(0))?;
    }
    for l in &f.links {
        if set[l.to.0 as usize] && !set[l.from.0 as usize] && !out.contains(&l.to) {
            push(&mut out, l.to)?;
        }
    }
    for (from, row) in g.exceptions.iter().enumerate() {
        if set[from] {
            continue;
        }
        for &e in row {
            if let Some(to) = Graph::exception_target(f, storage, e)
                && set[to.0 as usize]
                && !out.contains(&to)
            {
                push(&mut out, to)?;
            }
        }
    }
    Ok(out)
}
fn graph_kind(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    members: &[NodeId],
    entry: Option<NodeId>,
    reason: RecoveryReason,
) -> Result<RegionKind, RegionError> {
    let set = mask(f.nodes.len(), members)?;
    let entries = entries(storage, f, g, &set, entry)?;
    let mut exits = Vec::new();
    for l in &f.links {
        if set[l.from.0 as usize] && !set[l.to.0 as usize] {
            push(&mut exits, l.id)?;
        }
    }
    Ok(RegionKind::Graph {
        reason,
        entries,
        exits,
    })
}
fn compose(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    owners: &mut [RegionId],
    part: Part,
) -> Result<RegionId, RegionError> {
    let loop_continue = match part.completion {
        PartCompletion::Ordinary => None,
        PartCompletion::LoopBody(id) => {
            let target = f.loops[id.0 as usize].continue_target();
            if part.follow != Some(target) {
                return Err(bug("loop body lost its continue boundary"));
            }
            Some(target)
        }
    };
    let set = mask(f.nodes.len(), &part.nodes)?;
    let mut roots = Vec::new();
    for &p in &part.nodes {
        let r = owners[p.0 as usize];
        if !roots.contains(&r) {
            push(&mut roots, r)?;
        }
    }
    if roots.len() == 1 {
        return Ok(roots[0]);
    }
    let exported = entries(storage, f, g, &set, part.entry)?;
    let mut order = Vec::new();
    let mut at = part.entry.map(|p| owners[p.0 as usize]);
    let mut linear = exported.len() <= 1;
    while let Some(r) = at {
        if order.contains(&r) {
            linear = false;
            break;
        }
        push(&mut order, r)?;
        let mut next = None;
        for &p in &f.regions[r.0 as usize].nodes {
            for &l in &g.outgoing[p.0 as usize] {
                let q = f.links[l.0 as usize].to;
                // The accepted loop owns this body-completion link, not a
                // successor sibling. Its original transfer and copies survive.
                if Some(q) == loop_continue {
                    continue;
                }
                if set[q.0 as usize] && owners[q.0 as usize] != r {
                    let q = owners[q.0 as usize];
                    if next.is_some_and(|old| old != q) {
                        linear = false;
                    } else {
                        next = Some(q);
                    }
                }
            }
        }
        at = next;
    }
    linear &= order.len() == roots.len();
    let kind = if linear {
        RegionKind::Sequence
    } else {
        order = roots;
        let reason=if f.issues.iter().any(|issue|matches!(issue,PlanningIssue::Graph{entry,reason:RecoveryReason::Irreducible}if set[entry.0 as usize])){RecoveryReason::Irreducible}else if exported.len()>1{RecoveryReason::MultipleEntry}else{RecoveryReason::Unstructured};
        if let Some(entry) = part.entry {
            push(&mut f.issues, PlanningIssue::Graph { entry, reason })?;
        }
        graph_kind(storage, f, g, &part.nodes, part.entry, reason)?
    };
    let is_graph = matches!(kind, RegionKind::Graph { .. });
    let mut region = add(f, kind, part.entry, part.follow, order, copy(&part.nodes)?)?;
    if is_graph && let Some(target) = part.follow.filter(|n| !set[n.0 as usize]) {
        let mut children = Vec::new();
        push(&mut children, region)?;
        region = add(
            f,
            RegionKind::LabelledBlock { target },
            part.entry,
            part.follow,
            children,
            copy(&part.nodes)?,
        )?;
    }
    for p in part.nodes {
        owners[p.0 as usize] = region;
    }
    Ok(region)
}
fn branch_proposal(
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
fn candidates(
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
/// Split only private, linearly composable arm owners. This happens before
/// transfer planning: a common-follow exit then leaves the switch, not an
/// artificial labelled wrapper around a multi-entry graph. Selected nested
/// owners remain whole, so their scopes and exception ownership are unchanged.
fn private_switch_parts(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    owners: &[RegionId],
    c: &Candidate,
) -> Result<Option<Vec<Part>>, RegionError> {
    let RegionKind::Switch { arms, .. } = &c.kind else {
        return Ok(None);
    };
    if arms.len() < 2 {
        return Ok(None);
    }
    let body = &c.parts[1];
    let set = mask(f.nodes.len(), &body.nodes)?;
    let exported = entries(storage, f, g, &set, body.entry)?;
    let mut starts = Vec::new();
    for &arm in arms {
        let target = f.links[arm.0 as usize].to;
        if !set[target.0 as usize] || starts.contains(&target) {
            return Ok(None);
        }
        push(&mut starts, target)?;
    }
    if exported.len() != starts.len() || exported.iter().any(|n| !starts.contains(n)) {
        return Ok(None);
    }
    let mut partition = filled(f.nodes.len(), None)?;
    let empty = filled(f.nodes.len(), false)?;
    let mut parts = Vec::new();
    push(
        &mut parts,
        Part {
            nodes: copy(&[c.entry])?,
            entry: Some(c.entry),
            follow: None,
            completion: PartCompletion::Ordinary,
        },
    )?;
    for (index, &entry) in starts.iter().enumerate() {
        let arm = protection::reachable(storage, f, g, &[entry], c.follow, &empty, false)?;
        let nodes = list(&arm)?;
        let mut roots = Vec::new();
        for &node in &nodes {
            if !set[node.0 as usize] || partition[node.0 as usize].replace(index).is_some() {
                return Ok(None);
            }
            // Every normal entrance is either private or this selector's
            // original arm. Even an outside arrival at the case label rejects.
            for &link in &g.incoming[node.0 as usize] {
                let link = f.links[link.0 as usize];
                if !arm[link.from.0 as usize] && !(link.from == c.entry && node == entry) {
                    return Ok(None);
                }
            }
            let root = owners[node.0 as usize];
            if !roots.contains(&root) {
                push(&mut roots, root)?;
            }
            if f.regions[root.0 as usize]
                .nodes
                .iter()
                .any(|n| !arm[n.0 as usize])
            {
                return Ok(None);
            }
            for &link in &g.outgoing[node.0 as usize] {
                let target = f.links[link.0 as usize].to;
                if !arm[target.0 as usize] && Some(target) != c.follow {
                    return Ok(None);
                }
            }
            if g.outgoing[node.0 as usize].is_empty()
                && (!f.nodes[node.0 as usize].terminal
                    || !matches!(
                        storage.execution().blocks()[f.nodes[node.0 as usize].block.0 as usize]
                            .terminal(),
                        Some(Terminal::Return { .. } | Terminal::Throw { .. })
                    ))
            {
                return Ok(None);
            }
        }
        for (from, edges) in g.exceptions.iter().enumerate() {
            if arm[from] {
                continue;
            }
            for &edge in edges {
                if Graph::exception_possible(f, edge)
                    && Graph::exception_target(f, storage, edge).is_some_and(|n| arm[n.0 as usize])
                {
                    return Ok(None);
                }
            }
        }
        // Do not create a new graph/labelled scope within an arm. Only a
        // single chain of existing owners is admitted; no shared tail or
        // cross-arm fallthrough is moved or duplicated.
        let mut order = Vec::new();
        let mut at = Some(owners[entry.0 as usize]);
        while let Some(root) = at {
            if order.contains(&root) {
                return Ok(None);
            }
            push(&mut order, root)?;
            let mut next = None;
            for &node in &f.regions[root.0 as usize].nodes {
                for &link in &g.outgoing[node.0 as usize] {
                    let target = f.links[link.0 as usize].to;
                    if arm[target.0 as usize] && owners[target.0 as usize] != root {
                        let target = owners[target.0 as usize];
                        if next.is_some_and(|old| old != target) {
                            return Ok(None);
                        }
                        next = Some(target);
                    }
                }
            }
            at = next;
        }
        if order.len() != roots.len() {
            return Ok(None);
        }
        // A private continuation must terminate, not merely have a finite
        // postdominator candidate on one path. Reject cycles conservatively.
        if !normal_arm_acyclic(f, g, &arm)? {
            return Ok(None);
        }
        push(
            &mut parts,
            Part {
                nodes,
                entry: Some(entry),
                follow: c.follow,
                completion: PartCompletion::Ordinary,
            },
        )?;
    }
    if body.nodes.iter().any(|n| partition[n.0 as usize].is_none()) {
        return Ok(None);
    }
    Ok(Some(parts))
}

fn normal_arm_acyclic(f: &RegionFacts, g: &Graph, arm: &[bool]) -> Result<bool, RegionError> {
    let mut incoming = filled(f.nodes.len(), 0usize)?;
    let mut queue = Vec::new();
    let mut remaining = 0usize;
    for (node, &inside) in arm.iter().enumerate() {
        if inside {
            remaining += 1;
            incoming[node] = g.incoming[node]
                .iter()
                .filter(|l| arm[f.links[l.0 as usize].from.0 as usize])
                .count();
            if incoming[node] == 0 {
                push(&mut queue, NodeId(id(node)?))?;
            }
        }
    }
    while let Some(node) = queue.pop() {
        remaining -= 1;
        for &link in &g.outgoing[node.0 as usize] {
            let target = f.links[link.0 as usize].to;
            if arm[target.0 as usize] {
                incoming[target.0 as usize] -= 1;
                if incoming[target.0 as usize] == 0 {
                    push(&mut queue, target)?;
                }
            }
        }
    }
    Ok(remaining == 0)
}

fn accept(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    owners: &[RegionId],
    c: &Candidate,
) -> Result<Result<(), RecoveryReason>, RegionError> {
    let set = mask(f.nodes.len(), &c.members)?;
    if f.synchronized.iter().any(|scope| {
        scope.separation.is_none()
            && scope.body.contains(&c.entry)
            && c.members.iter().any(|n| !scope.body.contains(n))
    }) {
        return Ok(Err(RecoveryReason::LockBoundary));
    }
    if !protection::closed_entry(storage, f, g, &set, c.entry) {
        return Ok(Err(RecoveryReason::MultipleEntry));
    }
    for group in storage.facts().construction_groups() {
        let inside = group
            .members()
            .iter()
            .filter(|op| set[g.op_node[op.0 as usize].0 as usize])
            .count();
        if inside != 0 && inside != group.members().len() {
            return Ok(Err(RecoveryReason::ConstructionSplit));
        }
    }
    let mut partition = filled(f.nodes.len(), usize::MAX)?;
    for (i, p) in c.parts.iter().enumerate() {
        for &n in &p.nodes {
            if !set[n.0 as usize] || partition[n.0 as usize] != usize::MAX {
                return Err(bug("region candidate has duplicate or foreign content"));
            }
            partition[n.0 as usize] = i;
        }
    }
    for &n in &c.members {
        let part = partition[n.0 as usize];
        if part == usize::MAX {
            return Err(bug("region candidate omitted execution content"));
        }
        let root = owners[n.0 as usize];
        if f.regions[root.0 as usize]
            .nodes
            .iter()
            .any(|n| partition[n.0 as usize] != part)
        {
            return Ok(Err(RecoveryReason::Overlap));
        }
    }
    Ok(Ok(()))
}
fn match_branch_loops(
    f: &RegionFacts,
    c: &Candidate,
) -> Result<Result<(), RecoveryReason>, RegionError> {
    if !matches!(c.kind, RegionKind::If { .. }) {
        return Ok(Ok(()));
    }
    let set = mask(f.nodes.len(), &c.members)?;
    for plan in &f.loops {
        let inside = plan.members.iter().filter(|n| set[n.0 as usize]).count();
        let owns_loop = inside == plan.members.len();
        // All established plans constrain branches, before any loop is selected.
        if inside != 0 && !owns_loop && inside != c.members.len() {
            return Ok(Err(RecoveryReason::Overlap));
        }
        if !owns_loop
            && (set[plan.continue_target().0 as usize]
                || plan
                    .latches
                    .iter()
                    .any(|l| set[f.links[l.0 as usize].from.0 as usize]))
        {
            return Ok(Err(RecoveryReason::Overlap));
        }
    }
    Ok(Ok(()))
}
fn successor_arm_acyclic(
    f: &RegionFacts,
    owners: &[RegionId],
    arm: &[bool],
) -> Result<bool, RegionError> {
    let mut representative = Vec::new();
    for n in 0..f.nodes.len() {
        push(&mut representative, NodeId(id(n)?))?;
    }
    let mut contracted = filled(f.nodes.len(), false)?;
    let mut seen = filled(f.regions.len(), false)?;
    let mut stack = Vec::new();
    for (n, &inside) in arm.iter().enumerate() {
        if inside {
            let owner = owners[n];
            if !seen[owner.0 as usize] {
                seen[owner.0 as usize] = true;
                push(&mut stack, owner)?;
            }
        }
    }
    while let Some(id) = stack.pop() {
        let region = &f.regions[id.0 as usize];
        if matches!(region.kind, RegionKind::Loop(_))
            && region.nodes.iter().all(|n| arm[n.0 as usize])
        {
            let entry = region.entry.ok_or(bug("selected loop has no entry"))?;
            for &n in &region.nodes {
                representative[n.0 as usize] = entry;
                contracted[n.0 as usize] = true;
            }
        } else {
            for &child in &region.children {
                push(&mut stack, child)?;
            }
        }
    }
    let mut outgoing = lists(f.nodes.len())?;
    let mut incoming = filled(f.nodes.len(), 0usize)?;
    for link in &f.links {
        if !arm[link.from.0 as usize] || !arm[link.to.0 as usize] {
            continue;
        }
        let from = representative[link.from.0 as usize];
        let to = representative[link.to.0 as usize];
        if from == to && contracted[link.from.0 as usize] {
            continue;
        }
        push(&mut outgoing[from.0 as usize], to)?;
        incoming[to.0 as usize] = incoming[to.0 as usize]
            .checked_add(1)
            .ok_or(crate::RequestError::PlatformCapacity)?;
    }
    let mut queue = Vec::new();
    let mut remaining = 0usize;
    for (n, &inside) in arm.iter().enumerate() {
        if inside && representative[n].0 as usize == n {
            remaining += 1;
            if incoming[n] == 0 {
                push(&mut queue, NodeId(id(n)?))?;
            }
        }
    }
    while let Some(n) = queue.pop() {
        remaining -= 1;
        for &to in &outgoing[n.0 as usize] {
            incoming[to.0 as usize] -= 1;
            if incoming[to.0 as usize] == 0 {
                push(&mut queue, to)?;
            }
        }
    }
    Ok(remaining == 0)
}
fn match_successor_boundary(
    f: &RegionFacts,
    owners: &[RegionId],
    c: &Candidate,
) -> Result<Result<(), RecoveryReason>, RegionError> {
    if c.successor_boundary.is_none() {
        return Ok(Ok(()));
    }
    let follow = c.follow.ok_or(bug("successor boundary has no follow"))?;
    let mut arm = mask(f.nodes.len(), &c.members)?;
    if arm[follow.0 as usize] || c.parts[1..].iter().any(|p| p.nodes.contains(&c.entry)) {
        return Ok(Err(RecoveryReason::Unstructured));
    }
    if arm[0] && c.entry != NodeId(0) {
        return Ok(Err(RecoveryReason::MultipleEntry));
    }
    for link in &f.links {
        if arm[link.from.0 as usize] && !arm[link.to.0 as usize] && link.to != follow {
            return Ok(Err(RecoveryReason::Unstructured));
        }
    }
    arm[c.entry.0 as usize] = false;
    if !successor_arm_acyclic(f, owners, &arm)? {
        return Ok(Err(RecoveryReason::Unstructured));
    }
    Ok(Ok(()))
}
fn deliveries(f: &RegionFacts) -> Result<Vec<ExceptionDelivery>, RegionError> {
    let mut out = filled(f.edges.len(), ExceptionDelivery::Explicit)?;
    for e in &f.edges {
        if let EdgeRoute::Exception {
            delivery: ExceptionDelivery::ImpossibleRelease(op),
            ..
        } = e.route
        {
            out[e.edge.0 as usize] = ExceptionDelivery::ImpossibleRelease(op);
        }
    }
    for r in &f.regions {
        if let RegionKind::Try(id) = r.kind {
            let p = &f.tries[id.0 as usize];
            for t in &f.throws {
                if !p.body.contains(&t.node) {
                    continue;
                }
                let actual = protection::handlers(&f.chains[t.chain.0 as usize]);
                let wanted = &f.chains[p.chain.0 as usize].clauses
                    [p.clause_range.start as usize..p.clause_range.end as usize];
                if !protection::suffix(actual, wanted) {
                    continue;
                }
                let start = actual.len() - wanted.len();
                for &edge in &t.edges[start..actual.len()] {
                    if !matches!(
                        out[edge.0 as usize],
                        ExceptionDelivery::ImpossibleRelease(_)
                    ) {
                        out[edge.0 as usize] = ExceptionDelivery::Catch(id);
                    }
                }
            }
        }
    }
    Ok(out)
}
fn protect(storage: &StorageMethod<'_>, f: &mut RegionFacts, g: &Graph) -> Result<(), RegionError> {
    loop {
        let selected = deliveries(f)?;
        let mut withdraw = filled(f.regions.len(), false)?;
        for t in &f.throws {
            let mut explicit = false;
            let mut natural = false;
            for (&edge, c) in t.edges.iter().zip(&f.chains[t.chain.0 as usize].clauses) {
                if c.target == CatchTarget::Uncaught {
                    continue;
                }
                match selected[edge.0 as usize] {
                    ExceptionDelivery::Explicit => explicit = true,
                    ExceptionDelivery::Catch(_) => natural = true,
                    _ => {}
                }
            }
            if !explicit || !natural {
                continue;
            }
            for &edge in &t.edges {
                for r in &f.regions {
                    if matches!((&r.kind,selected[edge.0 as usize]),(RegionKind::Try(a),ExceptionDelivery::Catch(b))if *a==b)
                    {
                        withdraw[r.id.0 as usize] = true;
                    }
                }
            }
        }
        if !withdraw.iter().any(|x| *x) {
            for e in &mut f.edges {
                if let EdgeRoute::Exception { delivery, .. } = &mut e.route {
                    *delivery = selected[e.edge.0 as usize];
                }
            }
            for t in &mut f.throws {
                t.protection = if t
                    .edges
                    .iter()
                    .any(|e| matches!(selected[e.0 as usize], ExceptionDelivery::Catch(_)))
                {
                    SiteProtection::Natural
                } else {
                    SiteProtection::Explicit
                };
            }
            return Ok(());
        }
        for (i, &drop) in withdraw.iter().enumerate() {
            if !drop {
                continue;
            }
            let r = &f.regions[i];
            let kind = graph_kind(
                storage,
                f,
                g,
                &r.nodes,
                r.entry,
                RecoveryReason::ProtectionBoundary,
            )?;
            let entry = r.entry;
            f.regions[i].kind = kind;
            if let Some(entry) = entry {
                push(
                    &mut f.issues,
                    PlanningIssue::Graph {
                        entry,
                        reason: RecoveryReason::ProtectionBoundary,
                    },
                )?;
            }
        }
    }
}
fn scope_contexts(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
) -> Result<(), RegionError> {
    for index in 0..f.regions.len() {
        let RegionKind::Synchronized(scope) = f.regions[index].kind else {
            continue;
        };
        let mut failure = None;
        for exit in &f.synchronized[scope.0 as usize].exits {
            let SyncExitKind::Loop { loop_id, .. } = exit.kind else {
                continue;
            };
            let mut at = f.parents[index];
            let mut found = false;
            while let Some(parent) = at {
                if matches!(f.regions[parent.0 as usize].kind,RegionKind::Loop(id)if id==loop_id) {
                    found = true;
                    break;
                }
                at = f.parents[parent.0 as usize];
            }
            if !found {
                failure = Some(LockSeparation::LoopContext(loop_id));
                break;
            }
        }
        if let Some(reason) = failure {
            let r = &f.regions[index];
            let entry = r.entry;
            let kind = graph_kind(storage, f, g, &r.nodes, entry, RecoveryReason::LockBoundary)?;
            f.regions[index].kind = kind;
            f.synchronized[scope.0 as usize].separation = Some(reason);
            if let Some(entry) = entry {
                push(
                    &mut f.issues,
                    PlanningIssue::Graph {
                        entry,
                        reason: RecoveryReason::LockBoundary,
                    },
                )?;
            }
        }
    }
    Ok(())
}
fn own(storage: &StorageMethod<'_>, f: &mut RegionFacts, g: &Graph) -> Result<(), RegionError> {
    for op in storage.execution().operations() {
        let node = g.op_node[op.id().0 as usize];
        let owner = match storage
            .facts()
            .operation_emission(op.id())
            .ok_or(bug("P6 operation has no P5 emission owner"))?
        {
            OperationEmission::Ordinary => OperationOwner::Ordinary {
                node,
                region: f.labels[node.0 as usize].owner,
            },
            OperationEmission::Construction { group, role } => {
                OperationOwner::Construction { group, role }
            }
        };
        push(&mut f.operation_owners, owner)?;
    }
    for b in storage.execution().blocks() {
        push(
            &mut f.terminal_owners,
            TerminalOwner::Ordinary(f.labels[g.terminal_node[b.id().0 as usize].0 as usize].owner),
        )?;
    }
    for r in &f.regions {
        match &r.kind {
            RegionKind::If { .. } => {
                let node = r.entry.ok_or(bug("if has no condition entry"))?;
                f.terminal_owners[f.nodes[node.0 as usize].block.0 as usize] =
                    TerminalOwner::IfCondition(r.id);
            }
            RegionKind::Switch { .. } => {
                let node = r.entry.ok_or(bug("switch has no selector entry"))?;
                f.terminal_owners[f.nodes[node.0 as usize].block.0 as usize] =
                    TerminalOwner::SwitchSelector(r.id);
            }
            RegionKind::Loop(id) => {
                if let Some(node) = f.loops[id.0 as usize].condition() {
                    f.terminal_owners[f.nodes[node.0 as usize].block.0 as usize] =
                        TerminalOwner::LoopCondition(*id);
                }
            }
            RegionKind::Try(id) => {
                let p = &f.tries[id.0 as usize];
                for &handler in &p.handlers {
                    if let Some(op) = f
                        .handlers
                        .iter()
                        .find(|h| h.node == handler)
                        .and_then(|h| h.capture)
                    {
                        f.operation_owners[op.0 as usize] = OperationOwner::CatchCapture(*id);
                    }
                }
            }
            RegionKind::Synchronized(id) => {
                let p = &f.synchronized[id.0 as usize];
                f.operation_owners[p.enter.0 as usize] = OperationOwner::Synchronized {
                    scope: *id,
                    role: MonitorRole::Acquire,
                };
                for exit in &p.exits {
                    f.operation_owners[exit.release.0 as usize] = OperationOwner::Synchronized {
                        scope: *id,
                        role: MonitorRole::Release,
                    };
                    for &node in &exit.routing {
                        let part = &f.nodes[node.0 as usize];
                        if part.terminal {
                            f.terminal_owners[part.block.0 as usize] =
                                TerminalOwner::SynchronizedExit(*id);
                        }
                    }
                    for &plan in exit.before.iter().chain(&exit.after) {
                        let owner = &mut f.copies[plan.0 as usize].owner;
                        match *owner {
                            CopyOwner::Original => *owner = CopyOwner::SynchronizedExit(*id),
                            CopyOwner::SynchronizedExit(at) if at == *id => {}
                            _ => return Err(bug("copy has two selected emission owners")),
                        }
                    }
                }
            }
            _ => {}
        }
    }
    for plan in &mut f.link_plans {
        plan.copies
            .retain(|id| f.copies[id.0 as usize].owner == CopyOwner::Original);
    }
    Ok(())
}
fn transfers(f: &mut RegionFacts, g: &Graph) -> Result<(), RegionError> {
    for index in 0..f.links.len() {
        let l = f.links[index];
        let mut action = TransferAction::Goto(LabelId(l.to.0));
        let mut at = Some(f.labels[l.from.0 as usize].owner);
        let mut handled = false;
        while let Some(id) = at {
            let r = &f.regions[id.0 as usize];
            match &r.kind {
                RegionKind::If {
                    true_link,
                    false_link,
                } if r.entry == Some(l.from) => {
                    if l.id == *true_link || l.id == *false_link {
                        action = TransferAction::IfArm {
                            region: id,
                            taken: l.id == *true_link,
                        };
                        handled = true;
                        break;
                    }
                }
                RegionKind::Switch { arms, .. } if arms.contains(&l.id) => {
                    action = TransferAction::SwitchArm { region: id };
                    handled = true;
                    break;
                }
                RegionKind::Loop(loop_id) => {
                    let p = &f.loops[loop_id.0 as usize];
                    if p.condition() == Some(l.from) {
                        action = TransferAction::LoopCondition {
                            region: id,
                            repeat: p.members.contains(&l.to),
                        };
                        handled = true;
                        break;
                    }
                }
                RegionKind::Synchronized(scope) => {
                    let p = &f.synchronized[scope.0 as usize];
                    if p.enter_link == l.id {
                        action = TransferAction::EnterSynchronized(id);
                        handled = true;
                        break;
                    }
                    if let Some(exit) = p.exits.iter().find(|exit| exit.link == l.id) {
                        action = TransferAction::SynchronizedReleased {
                            region: id,
                            release: exit.release,
                        };
                        handled = true;
                        break;
                    }
                }
                _ => {}
            }
            at = f.parents[id.0 as usize];
        }
        if !handled {
            let mut at = Some(f.labels[l.from.0 as usize].owner);
            while let Some(id) = at {
                let r = &f.regions[id.0 as usize];
                match &r.kind {
                    RegionKind::Loop(loop_id) => {
                        let p = &f.loops[loop_id.0 as usize];
                        if l.to == p.continue_target() {
                            action = TransferAction::Continue(*loop_id);
                            break;
                        }
                        if p.follow == Some(l.to) && !p.members.contains(&l.to) {
                            action = TransferAction::Break(id);
                            break;
                        }
                    }
                    RegionKind::Synchronized(sync) => {
                        let p = &f.synchronized[sync.0 as usize];
                        if p.body.contains(&l.from)
                            && let Some(exit) = p
                                .exits
                                .iter()
                                .find(|exit| g.op_node[exit.release.0 as usize] == l.to)
                        {
                            action = TransferAction::LeaveSynchronized {
                                region: id,
                                release: exit.release,
                            };
                            break;
                        }
                    }
                    RegionKind::Switch { .. } | RegionKind::LabelledBlock { .. }
                        if r.follow == Some(l.to) && !r.nodes.contains(&l.to) =>
                    {
                        action = TransferAction::Break(id);
                        break;
                    }
                    RegionKind::Sequence => {
                        for pair in r.children.windows(2) {
                            let a = &f.regions[pair[0].0 as usize];
                            let b = &f.regions[pair[1].0 as usize];
                            if a.nodes.contains(&l.from) && b.entry == Some(l.to) {
                                action = TransferAction::Next(l.to);
                                handled = true;
                                break;
                            }
                        }
                        if handled {
                            break;
                        }
                    }
                    RegionKind::If { .. } | RegionKind::Try(_) if r.follow == Some(l.to) => {
                        action = TransferAction::Next(l.to);
                        break;
                    }
                    _ => {}
                }
                at = f.parents[id.0 as usize];
            }
        }
        f.link_plans[index].action = action;
    }
    for i in 0..f.regions.len() {
        let r = &f.regions[i];
        let mut continuation = r
            .follow
            .map_or(Continuation::None, |n| Continuation::Goto(LabelId(n.0)));
        if let (Some(parent), Some(follow)) = (f.parents[i], r.follow) {
            let p = &f.regions[parent.0 as usize];
            match &p.kind {
                RegionKind::Sequence => {
                    if let Some(position) = p.children.iter().position(|r| r.0 as usize == i) {
                        if let Some(next) = p.children.get(position + 1) {
                            if f.regions[next.0 as usize].entry == Some(follow) {
                                continuation = Continuation::Next(follow);
                            }
                        } else if p.follow == Some(follow) {
                            continuation = Continuation::Parent;
                        }
                    }
                }
                RegionKind::If { .. }
                | RegionKind::Switch { .. }
                | RegionKind::Try(_)
                | RegionKind::LabelledBlock { .. }
                    if p.follow == Some(follow) =>
                {
                    continuation = Continuation::Parent
                }
                RegionKind::Loop(loop_id)
                    if f.loops[loop_id.0 as usize].continue_target() == follow =>
                {
                    continuation = Continuation::Parent
                }
                RegionKind::Synchronized(sync)
                    if f.synchronized[sync.0 as usize].follow == Some(follow)
                        || f.synchronized[sync.0 as usize]
                            .exits
                            .iter()
                            .any(|exit| g.op_node[exit.release.0 as usize] == follow) =>
                {
                    continuation = Continuation::Parent
                }
                _ => {}
            }
        }
        f.regions[i].continuation = continuation;
    }
    Ok(())
}
pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    s: &Structure,
) -> Result<(), RegionError> {
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
    for mut c in candidates(storage, f, g, s)? {
        let acceptance = match accept(storage, f, g, &owners, &c)? {
            Ok(()) => match match_branch_loops(f, &c)? {
                Ok(()) => match_successor_boundary(f, &owners, &c)?,
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
        if let Some(parts) = private_switch_parts(storage, f, g, &owners, &c)? {
            let old = std::mem::replace(&mut c.parts, parts);
            if accept(storage, f, g, &owners, &c)?.is_ok() {
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
    transfers(f, g)
}
