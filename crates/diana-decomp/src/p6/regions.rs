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
struct Part {
    nodes: Vec<NodeId>,
    entry: Option<NodeId>,
    follow: Option<NodeId>,
}
struct Candidate {
    kind: RegionKind,
    entry: NodeId,
    follow: Option<NodeId>,
    members: Vec<NodeId>,
    parts: Vec<Part>,
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
fn candidates(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    s: &Structure,
) -> Result<Vec<Candidate>, RegionError> {
    let mut out = Vec::new();
    for l in &f.loops {
        let mut parts = Vec::new();
        let mut body = Vec::new();
        for &node in &l.members {
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
                nodes: body,
                entry,
                follow: Some(l.continue_target()),
            },
        )?;
        if let Some(condition) = l.condition() {
            push(
                &mut parts,
                Part {
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
                members: copy(&l.members)?,
                parts,
            },
        )?;
    }
    for p in &f.tries {
        let mut parts = Vec::new();
        push(
            &mut parts,
            Part {
                nodes: copy(&p.body)?,
                entry: Some(p.entry),
                follow: p.follow,
            },
        )?;
        for (&entry, body) in p.handlers.iter().zip(&p.handler_bodies) {
            push(
                &mut parts,
                Part {
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
            },
        )?;
    }
    for p in &f.synchronized {
        let entry = g.op_node[p.enter.0 as usize];
        let mut parts = Vec::new();
        push(
            &mut parts,
            Part {
                nodes: copy(&[entry])?,
                entry: Some(entry),
                follow: None,
            },
        )?;
        let body_entry = g.outgoing[entry.0 as usize]
            .first()
            .map(|l| f.links[l.0 as usize].to);
        let release = g.op_node[p.releases[0].0 as usize];
        push(
            &mut parts,
            Part {
                nodes: copy(&p.body)?,
                entry: body_entry.filter(|n| p.body.contains(n)),
                follow: Some(release),
            },
        )?;
        for &op in &p.releases {
            let n = g.op_node[op.0 as usize];
            push(
                &mut parts,
                Part {
                    nodes: copy(&[n])?,
                    entry: Some(n),
                    follow: p.follow,
                },
            )?;
        }
        for c in &p.cleanups {
            push(
                &mut parts,
                Part {
                    nodes: copy(&c.nodes)?,
                    entry: c.nodes.first().copied(),
                    follow: None,
                },
            )?;
        }
        push(
            &mut out,
            Candidate {
                kind: RegionKind::Synchronized(p.id),
                entry,
                follow: p.follow,
                members: copy(&p.members)?,
                parts,
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
        let empty = filled(f.nodes.len(), false)?;
        let mut parts = Vec::new();
        push(
            &mut parts,
            Part {
                nodes: copy(&[n.id])?,
                entry: Some(n.id),
                follow: None,
            },
        )?;
        let members;
        let kind = if matches!(terminal, Some(Terminal::Branch { .. })) {
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
            let t = f.links[yes.0 as usize].to;
            let e = f.links[no.0 as usize].to;
            let a = protection::reachable(storage, f, g, &[t], follow, &empty, false)?;
            let b = protection::reachable(storage, f, g, &[e], follow, &empty, false)?;
            if a[n.id.0 as usize] || b[n.id.0 as usize] || a.iter().zip(&b).any(|(a, b)| *a && *b) {
                continue;
            }
            let mut set = filled(f.nodes.len(), false)?;
            set[n.id.0 as usize] = true;
            for i in 0..set.len() {
                set[i] |= a[i] || b[i];
            }
            members = list(&set)?;
            push(
                &mut parts,
                Part {
                    nodes: list(&a)?,
                    entry: if Some(t) == follow { None } else { Some(t) },
                    follow,
                },
            )?;
            push(
                &mut parts,
                Part {
                    nodes: list(&b)?,
                    entry: if Some(e) == follow { None } else { Some(e) },
                    follow,
                },
            )?;
            RegionKind::If {
                true_link: yes,
                false_link: no,
            }
        } else {
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
            members = list(&set)?;
            push(
                &mut parts,
                Part {
                    nodes: list(&body)?,
                    entry: starts.iter().copied().find(|n| Some(*n) != follow),
                    follow,
                },
            )?;
            RegionKind::Switch { arms }
        };
        push(
            &mut out,
            Candidate {
                kind,
                entry: n.id,
                follow,
                members,
                parts,
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
        )
    });
    Ok(out)
}
fn accept(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    owners: &[RegionId],
    c: &Candidate,
) -> Result<Result<(), RecoveryReason>, RegionError> {
    let set = mask(f.nodes.len(), &c.members)?;
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
        match r.kind {
            RegionKind::Try(id) => {
                let p = &f.tries[id.0 as usize];
                for t in &f.throws {
                    if !p.body.contains(&t.node) {
                        continue;
                    }
                    let actual = protection::context_handlers(f, t.chain, p.outer_sync);
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
            RegionKind::Synchronized(id) => {
                let p = &f.synchronized[id.0 as usize];
                for t in &f.throws {
                    if !p.body.contains(&t.node) {
                        continue;
                    }
                    for (&edge, c) in t.edges.iter().zip(&f.chains[t.chain.0 as usize].clauses) {
                        if c.catch.is_none()
                            && matches!(c.target,CatchTarget::Handler(n)if p.cleanups.iter().any(|c|c.nodes.first()==Some(&n)))
                            && !matches!(
                                out[edge.0 as usize],
                                ExceptionDelivery::ImpossibleRelease(_)
                            )
                        {
                            out[edge.0 as usize] = ExceptionDelivery::Cleanup(id);
                        }
                    }
                }
            }
            _ => {}
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
                    ExceptionDelivery::Catch(_) | ExceptionDelivery::Cleanup(_) => natural = true,
                    _ => {}
                }
            }
            if !explicit || !natural {
                continue;
            }
            for &edge in &t.edges {
                for r in &f.regions {
                    if matches!((&r.kind,selected[edge.0 as usize]),(RegionKind::Try(a),ExceptionDelivery::Catch(b))if *a==b)
                        || matches!((&r.kind,selected[edge.0 as usize]),(RegionKind::Synchronized(a),ExceptionDelivery::Cleanup(b))if *a==b)
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
                t.protection = if t.edges.iter().any(|e| {
                    matches!(
                        selected[e.0 as usize],
                        ExceptionDelivery::Catch(_) | ExceptionDelivery::Cleanup(_)
                    )
                }) {
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
            OperationEmission::Construction { group, .. } => OperationOwner::Construction(group),
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
                for &op in &p.releases {
                    f.operation_owners[op.0 as usize] = OperationOwner::Synchronized {
                        scope: *id,
                        role: MonitorRole::Release,
                    };
                }
                for c in &p.cleanups {
                    for &node in &c.nodes {
                        for op in f.nodes[node.0 as usize].operations.clone() {
                            f.operation_owners[op as usize] = OperationOwner::Synchronized {
                                scope: *id,
                                role: MonitorRole::Cleanup,
                            };
                        }
                        if f.nodes[node.0 as usize].terminal {
                            f.terminal_owners[f.nodes[node.0 as usize].block.0 as usize] =
                                TerminalOwner::SynchronizedCleanup(*id);
                        }
                    }
                }
            }
            _ => {}
        }
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
                RegionKind::Switch { arms } if arms.contains(&l.id) => {
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
                    if p.release_link == l.id {
                        action = TransferAction::SynchronizedReleased(id);
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
                            && let Some(&op) = p
                                .releases
                                .iter()
                                .find(|op| g.op_node[op.0 as usize] == l.to)
                        {
                            action = TransferAction::LeaveSynchronized {
                                region: id,
                                release: op,
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
                    if f.synchronized[sync.0 as usize]
                        .releases
                        .iter()
                        .any(|op| g.op_node[op.0 as usize] == follow) =>
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
    for c in candidates(storage, f, g, s)? {
        if let Err(reason) = accept(storage, f, g, &owners, &c)? {
            push(
                &mut f.issues,
                PlanningIssue::Graph {
                    entry: c.entry,
                    reason,
                },
            )?;
            continue;
        }
        let mut children = Vec::new();
        for p in c.parts {
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
            nodes: all,
            entry: Some(NodeId(0)),
            follow: None,
        },
    )?);
    protect(storage, f, g)?;
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
            monitor.mode = MonitorMode::Explicit(LockSeparation::Region(reason));
        }
    }
    own(storage, f, g)?;
    transfers(f, g)
}
