use super::build::*;
use super::dominance::Structure;
use super::graph::Graph;
use super::*;
use crate::p2::{EdgeRole, EdgeTarget, OpKind};
use crate::p5::{CopyLocation, OperationEmission, StorageMethod, ThrowSite};

fn add(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    site: ThrowSite,
    node: NodeId,
    completion: crate::p2::ExceptionCompletion,
) -> Result<(), RegionError> {
    let mut clauses = Vec::new();
    let mut edges = Vec::new();
    for &id in &g.exceptions[node.0 as usize] {
        let edge = &storage.execution().edges()[id.0 as usize];
        if edge.source_op()
            != match site {
                ThrowSite::Operation(op) => Some(op),
                ThrowSite::Terminal(_) => None,
            }
        {
            continue;
        }
        let EdgeRole::Exception { order, catch, .. } = edge.role() else {
            return Err(bug("exception index contains normal edge"));
        };
        let target = match edge.target() {
            EdgeTarget::Block(b) => CatchTarget::Handler(Graph::entry(f, b)),
            EdgeTarget::Exit(_) => CatchTarget::Uncaught,
        };
        push(
            &mut clauses,
            HandlerClause {
                order,
                catch,
                target,
            },
        )?;
        push(&mut edges, id)?;
    }
    if edges.is_empty() {
        return Err(bug("throwing site lost its completion transfers"));
    }
    let chain = if let Some(c) = f.chains.iter().find(|c| c.clauses == clauses) {
        c.id
    } else {
        let id = ChainId(id(f.chains.len())?);
        push(&mut f.chains, HandlerChain { id, clauses })?;
        id
    };
    let index = ThrowSiteId(id(f.throws.len())?);
    for &e in &edges {
        let order = match storage.execution().edges()[e.0 as usize].role() {
            EdgeRole::Exception { order, .. } => order,
            _ => return Err(bug("throw edge changed role")),
        };
        f.edges[e.0 as usize].route = EdgeRoute::Exception {
            site: index,
            order,
            delivery: ExceptionDelivery::Explicit,
        };
    }
    let mut snapshots = Vec::new();
    for p in storage.facts().copy_plans() {
        if p.location() == CopyLocation::BeforeThrow(site) {
            push(&mut snapshots, p.id())?;
        }
    }
    let construction = match site {
        ThrowSite::Operation(op) => match storage.facts().operation_emission(op) {
            Some(OperationEmission::Construction { group, .. }) => Some(group),
            _ => None,
        },
        _ => None,
    };
    push(
        &mut f.throws,
        ThrowPlan {
            id: index,
            site,
            node,
            completion,
            chain,
            edges,
            snapshots,
            construction,
            protection: SiteProtection::Explicit,
        },
    )
}
pub(super) fn collect(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
) -> Result<(), RegionError> {
    for op in storage.execution().operations() {
        if let Some(c) = op.exceptional() {
            add(
                storage,
                f,
                g,
                ThrowSite::Operation(op.id()),
                g.op_node[op.id().0 as usize],
                c,
            )?;
        }
    }
    for b in storage.execution().blocks() {
        if let Some(c) = b.exceptional() {
            add(
                storage,
                f,
                g,
                ThrowSite::Terminal(b.id()),
                g.terminal_node[b.id().0 as usize],
                c,
            )?;
        }
    }
    for edge in storage.execution().exception_edges() {
        if let EdgeTarget::Block(b) = edge.target() {
            let node = Graph::entry(f, b);
            if f.handlers.iter().any(|h| h.node == node) {
                continue;
            }
            let capture = storage.execution().blocks()[b.0 as usize]
                .operations()
                .next()
                .map(|op| &storage.execution().operations()[op as usize])
                .filter(|op| matches!(op.kind(), OpKind::MoveException))
                .map(|op| op.id());
            let mut receives = Vec::new();
            for p in storage.facts().copy_plans() {
                if matches!(p.location(),CopyLocation::HandlerEntry{block,..}if block==b) {
                    push(&mut receives, p.id())?;
                }
            }
            f.labels[node.0 as usize].kind = LabelKind::Handler;
            push(
                &mut f.handlers,
                HandlerEntry {
                    node,
                    capture,
                    receives,
                },
            )?;
        }
    }
    Ok(())
}
pub(super) fn handlers(chain: &HandlerChain) -> &[HandlerClause] {
    let end = chain
        .clauses
        .iter()
        .position(|c| c.target == CatchTarget::Uncaught)
        .unwrap_or(chain.clauses.len());
    &chain.clauses[..end]
}
pub(super) fn suffix(actual: &[HandlerClause], wanted: &[HandlerClause]) -> bool {
    actual.len() >= wanted.len()
        && actual[actual.len() - wanted.len()..]
            .iter()
            .zip(wanted)
            .all(|(a, b)| a.catch == b.catch && a.target == b.target)
}
pub(super) fn reachable(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    seeds: &[NodeId],
    stop: Option<NodeId>,
    excluded: &[bool],
    exceptional: bool,
) -> Result<Vec<bool>, RegionError> {
    let mut result = filled(f.nodes.len(), false)?;
    let mut queue = Vec::new();
    for &s in seeds {
        if Some(s) != stop && !excluded[s.0 as usize] && !result[s.0 as usize] {
            result[s.0 as usize] = true;
            push(&mut queue, s)?;
        }
    }
    while let Some(p) = queue.pop() {
        for &l in &g.outgoing[p.0 as usize] {
            let q = f.links[l.0 as usize].to;
            if Some(q) != stop && !excluded[q.0 as usize] && !result[q.0 as usize] {
                result[q.0 as usize] = true;
                push(&mut queue, q)?;
            }
        }
        if exceptional {
            for &e in &g.exceptions[p.0 as usize] {
                if let Some(q) = Graph::exception_target(f, storage, e)
                    && Some(q) != stop
                    && !excluded[q.0 as usize]
                    && !result[q.0 as usize]
                {
                    result[q.0 as usize] = true;
                    push(&mut queue, q)?;
                }
            }
        }
    }
    Ok(result)
}
fn list(set: &[bool]) -> Result<Vec<NodeId>, RegionError> {
    let mut result = Vec::new();
    for (i, &inside) in set.iter().enumerate() {
        if inside {
            push(&mut result, NodeId(id(i)?))?;
        }
    }
    Ok(result)
}
pub(super) fn closed_entry(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    set: &[bool],
    entry: NodeId,
) -> bool {
    if g.external_entry(f, set, entry) {
        return false;
    }
    for (from, edges) in g.exceptions.iter().enumerate() {
        if set[from] {
            continue;
        }
        for &e in edges {
            if let Some(to) = Graph::exception_target(f, storage, e)
                && set[to.0 as usize]
                && to != entry
            {
                return false;
            }
        }
    }
    true
}
/// Only a proved enclosing lock cleanup is projected out of a catch frontier.
/// The original site chain and every edge/order remain unchanged.
pub(super) fn context_handlers(
    f: &RegionFacts,
    chain: ChainId,
    outer: Option<SyncId>,
) -> &[HandlerClause] {
    let raw = handlers(&f.chains[chain.0 as usize]);
    if let Some(id) = outer
        && let Some(last) = raw.last()
        && last.catch.is_none()
        && matches!(last.target,CatchTarget::Handler(n)if f.synchronized[id.0 as usize].cleanups.iter().any(|c|c.nodes.first()==Some(&n)))
    {
        &raw[..raw.len() - 1]
    } else {
        raw
    }
}
struct Pattern {
    chain: ChainId,
    start: usize,
    end: usize,
    outer: Option<SyncId>,
}
pub(super) fn plan(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
    s: &Structure,
) -> Result<(), RegionError> {
    let mut patterns: Vec<Pattern> = Vec::new();
    for context in 0..=f.synchronized.len() {
        let outer = if context == 0 {
            None
        } else {
            Some(SyncId(id(context - 1)?))
        };
        for t in &f.throws {
            if outer.is_some_and(|id| !f.synchronized[id.0 as usize].body.contains(&t.node)) {
                continue;
            }
            let clauses = context_handlers(f, t.chain, outer);
            for start in 0..clauses.len() {
                let wanted = &clauses[start..];
                if patterns.iter().any(|p| {
                    p.outer == outer
                        && p.end - p.start == wanted.len()
                        && suffix(
                            &f.chains[p.chain.0 as usize].clauses[p.start..p.end],
                            wanted,
                        )
                }) {
                    continue;
                }
                push(
                    &mut patterns,
                    Pattern {
                        chain: t.chain,
                        start,
                        end: clauses.len(),
                        outer,
                    },
                )?;
            }
        }
    }
    for pattern in patterns {
        let Pattern {
            chain,
            start,
            end,
            outer,
        } = pattern;
        let wanted = &f.chains[chain.0 as usize].clauses[start..end];
        let mut targets = Vec::new();
        let mut repeated = false;
        for clause in wanted {
            let CatchTarget::Handler(target) = clause.target else {
                continue;
            };
            if targets.contains(&target) {
                repeated = true;
                break;
            }
            push(&mut targets, target)?;
        }
        if repeated || targets.is_empty() {
            continue;
        }
        let mut seeds = Vec::new();
        for t in &f.throws {
            if outer.is_some_and(|id| !f.synchronized[id.0 as usize].body.contains(&t.node)) {
                continue;
            }
            if suffix(context_handlers(f, t.chain, outer), wanted) && !seeds.contains(&t.node) {
                push(&mut seeds, t.node)?;
            }
        }
        if seeds.is_empty() {
            continue;
        }
        if f.synchronized.iter().any(|sync| {
            targets.iter().any(|target| {
                sync.cleanups
                    .iter()
                    .any(|c| c.nodes.first() == Some(target))
            }) && seeds.iter().all(|n| sync.members.contains(n))
        }) {
            continue;
        }
        let mut follows = Vec::new();
        for &n in seeds.iter().chain(&targets) {
            push(&mut follows, n)?;
        }
        let follow = s.common_follow(&follows);
        let mut excluded = filled(f.nodes.len(), false)?;
        if let Some(scope) = outer {
            for (i, slot) in excluded.iter_mut().enumerate() {
                *slot = !f.synchronized[scope.0 as usize]
                    .body
                    .contains(&NodeId(id(i)?));
            }
        }
        for &target in &targets {
            excluded[target.0 as usize] = true;
        }
        let body = reachable(storage, f, g, &seeds, follow, &excluded, true)?;
        let mut entries = Vec::new();
        if body[0] {
            push(&mut entries, NodeId(0))?;
        }
        for l in &f.links {
            if body[l.to.0 as usize] && !body[l.from.0 as usize] && !entries.contains(&l.to) {
                push(&mut entries, l.to)?;
            }
        }
        for (from, edges) in g.exceptions.iter().enumerate() {
            if body[from] {
                continue;
            }
            for &e in edges {
                if let Some(to) = Graph::exception_target(f, storage, e)
                    && body[to.0 as usize]
                    && !entries.contains(&to)
                {
                    push(&mut entries, to)?;
                }
            }
        }
        if entries.len() != 1 {
            continue;
        }
        let entry = entries[0];
        let guarded =
            f.throws
                .iter()
                .filter(|t| body[t.node.0 as usize])
                .all(|t| {
                    t.edges.iter().all(|e| {
                        matches!(
                            f.edges[e.0 as usize].route,
                            EdgeRoute::Exception {
                                delivery: ExceptionDelivery::ImpossibleRelease(_),
                                ..
                            }
                        )
                    }) || suffix(context_handlers(f, t.chain, outer), wanted)
                        || f.chains[t.chain.0 as usize].clauses.iter().all(
                            |c| matches!(c.target,CatchTarget::Handler(n)if body[n.0 as usize]),
                        )
                });
        if !guarded {
            continue;
        }
        let mut members = body;
        let body_nodes = list(&members)?;
        let mut valid = true;
        let mut handler_bodies = Vec::new();
        for &target in &targets {
            let mut exclude = filled(f.nodes.len(), false)?;
            if let Some(scope) = outer {
                for (i, slot) in exclude.iter_mut().enumerate() {
                    *slot = !f.synchronized[scope.0 as usize]
                        .body
                        .contains(&NodeId(id(i)?));
                }
            }
            for (i, &inside) in members.iter().enumerate() {
                exclude[i] |= inside;
            }
            for &other in &targets {
                if other != target {
                    exclude[other.0 as usize] = true;
                }
            }
            let arm = reachable(storage, f, g, &[target], follow, &exclude, true)?;
            if !arm[target.0 as usize]
                || f.throws.iter().any(|t| {
                    arm[t.node.0 as usize] && suffix(context_handlers(f, t.chain, outer), wanted)
                })
            {
                valid = false;
                break;
            }
            push(&mut handler_bodies, list(&arm)?)?;
            for (i, &inside) in arm.iter().enumerate() {
                members[i] |= inside;
            }
        }
        if valid {
            for (&target, nodes) in targets.iter().zip(&handler_bodies) {
                let mut arm = filled(f.nodes.len(), false)?;
                for node in nodes {
                    arm[node.0 as usize] = true;
                }
                for l in &f.links {
                    if arm[l.to.0 as usize] && (l.to == target || !arm[l.from.0 as usize]) {
                        valid = false;
                        break;
                    }
                }
                for (from, edges) in g.exceptions.iter().enumerate() {
                    for &e in edges {
                        if let Some(to) = Graph::exception_target(f, storage, e)
                            && arm[to.0 as usize]
                            && (to == target || !arm[from])
                            && !(to == target && body_nodes.contains(&NodeId(id(from)?)))
                        {
                            valid = false;
                            break;
                        }
                    }
                    if !valid {
                        break;
                    }
                }
                if !valid {
                    break;
                }
            }
        }
        if !valid || !closed_entry(storage, f, g, &members, entry) {
            continue;
        }
        let plan = TryPlan {
            id: TryId(id(f.tries.len())?),
            chain,
            clause_range: id(start)?..id(end)?,
            outer_sync: outer,
            entry,
            body: body_nodes,
            handlers: targets,
            handler_bodies,
            members: list(&members)?,
            follow,
        };
        push(&mut f.tries, plan)?;
    }
    Ok(())
}
