//! Pre-check redundancy of an original `check-cast`.
//!
//! One original `check-cast` cannot add behavior at its own position when every
//! reaching transfer already establishes that the checked value is null or an
//! instance of that exact target type. Two original events establish it:
//!
//! 1. A same-target `check-cast` that completed normally on the same value. Its
//!    exceptional completion establishes nothing.
//! 2. An `instanceof` outcome on the branch where its result is nonzero. The
//!    zero outcome establishes nothing, and `instanceof` is false for null.
//!
//! The proof is a must-fact over P3 states/transfers and P4 alias relations, not
//! a union of alternative proofs: a join is established only when every reaching
//! transfer is, a root without reaching facts is never established, and a cycle
//! starts from "not established", so the first failing check of a loop body
//! cannot disappear. Each local proof is read at the transfer leaving its
//! establishing point, so a check's own success never justifies that same check.
//! Merged booleans, merged aliases and unavailable facts are not proofs.
//!
//! Unavailable/unsupported reaching facts are published as `Unknown`; nothing
//! here fails the method or suppresses a check.

use super::build::*;
use super::constraints::Work;
use super::references::*;
use super::*;
use crate::p2::{BranchOp, EdgeId, EdgeRole, NormalEdge, OpId, OpKind, Terminal};
use crate::p3::{
    PointId, PointKind, SsaMethod, StateId, Target, TransferId, TransferOrigin, ValueOrigin,
};
use std::collections::HashMap;

/// One original event that proves membership-or-null for one value and target.
#[derive(Clone, Copy)]
struct Establisher {
    /// The reference value the event proves.
    value: StateId,
    target: crate::TypeId,
    source: CheckSource,
    /// For an `instanceof` outcome, the normal edge that means "nonzero".
    edge: Option<EdgeId>,
}

/// One query: is this value established for this exact target at this point?
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct Query {
    state: StateId,
    point: PointId,
    target: crate::TypeId,
}

#[derive(Clone, Copy)]
struct Link {
    source: usize,
    /// The establishing event on the transfer into `source` and the alias
    /// relation between this value and that event's value.
    local: Option<(usize, Establisher)>,
}

struct Node {
    key: Query,
    established: bool,
    /// The original event at this very point that already proves membership, if
    /// the value is born here.
    grounded: Option<CheckSource>,
    /// The first unsupported dependency on this value at this point, if any.
    gap: Option<CheckGap>,
    links: Vec<Link>,
    users: Vec<usize>,
}

struct Solver<'s, 'a> {
    refs: References<'s, 'a>,
    incoming: Vec<Vec<TransferId>>,
    events: Vec<Option<Establisher>>,
    aliases: Aliases,
    index: HashMap<Query, usize>,
    nodes: Vec<Node>,
}
impl Solver<'_, '_> {
    fn query(
        &mut self,
        state: StateId,
        point: PointId,
        target: crate::TypeId,
    ) -> Result<usize, TypeError> {
        let point = self
            .refs
            .canonical(state, None, point, &self.incoming, Some(&self.events));
        let key = Query {
            state,
            point,
            target,
        };
        if let Some(&index) = self.index.get(&key) {
            return Ok(index);
        }
        self.index
            .try_reserve(1)
            .map_err(crate::RequestError::Allocation)?;
        let index = self.nodes.len();
        push(
            &mut self.nodes,
            Node {
                key,
                established: false,
                grounded: None,
                gap: None,
                links: Vec::new(),
                users: Vec::new(),
            },
        )?;
        self.index.insert(key, index);
        Ok(index)
    }
    fn link(
        &mut self,
        target: usize,
        source: usize,
        local: Option<(usize, Establisher)>,
    ) -> Result<(), TypeError> {
        push(&mut self.nodes[target].links, Link { source, local })?;
        push(&mut self.nodes[source].users, target)
    }
    /// Expand every query to its reaching transfers. A value created at this
    /// point (entry, allocation, call, field or array read) has no earlier event
    /// that proves its dynamic membership, so it stays unestablished.
    fn build(&mut self) -> Result<(), TypeError> {
        let mut index = 0;
        while index < self.nodes.len() {
            let Query { state, point, .. } = self.nodes[index].key;
            if !self.refs.available(state) {
                self.nodes[index].gap = Some(CheckGap::SourceUnavailable(state));
                index += 1;
                continue;
            }
            if self.refs.value(state).is_none() {
                self.nodes[index].gap = Some(CheckGap::UndefinedSource(state));
                index += 1;
                continue;
            }
            let source = self.refs.source(state)?;
            if self.refs.definition(state) == Some(point) && matches!(source, Source::Leaf) {
                // A value created by an original `new-instance` of exactly the
                // checked type is an instance of that type on every execution of
                // the site, so this check adds no behavior here. The allocation
                // itself is not a claim that two executions share an object.
                let target = self.nodes[index].key.target;
                self.nodes[index].grounded = allocation(&self.refs, state, target)?;
                index += 1;
                continue;
            }
            for i in 0..self.incoming[point.0 as usize].len() {
                let transfer = self.incoming[point.0 as usize][i];
                let from = self.refs.ssa.facts().transfers()[transfer.0 as usize].from();
                let state = self.refs.rewrite(state, point, transfer)?;
                let local = establishment(
                    &self.refs,
                    &self.incoming,
                    &mut self.aliases,
                    self.events[from.0 as usize],
                    transfer,
                    state,
                    from,
                )?;
                let source = self.query(state, from, self.nodes[index].key.target)?;
                self.link(index, source, local)?;
            }
            index += 1;
        }
        aliases(&self.refs, &self.incoming, &mut self.aliases)
    }
    /// The local proof this link contributes to its own node, if it holds.
    fn local(&self, node: usize, link: Link) -> Option<Establisher> {
        let (alias, event) = link.local?;
        (self.aliases.nodes[alias].bits == SAME && event.target == self.nodes[node].key.target)
            .then_some(event)
    }
    /// Least fixpoint of "established here and on every reaching transfer". A
    /// node without reaching transfers is never established, so a root or a
    /// cycle cannot prove itself.
    fn solve(&mut self) -> Result<(), TypeError> {
        let mut queue = Vec::new();
        let mut queued = filled(self.nodes.len(), false)?;
        for (index, pending) in queued.iter_mut().enumerate() {
            *pending = true;
            push(&mut queue, index)?;
        }
        while let Some(index) = queue.pop() {
            queued[index] = false;
            let mut established =
                self.nodes[index].grounded.is_some() || !self.nodes[index].links.is_empty();
            if self.nodes[index].grounded.is_none() && established {
                for i in 0..self.nodes[index].links.len() {
                    let link = self.nodes[index].links[i];
                    if self.local(index, link).is_none() && !self.nodes[link.source].established {
                        established = false;
                        break;
                    }
                }
            }
            if !established || self.nodes[index].established {
                continue;
            }
            self.nodes[index].established = true;
            for i in 0..self.nodes[index].users.len() {
                let user = self.nodes[index].users[i];
                if !queued[user] {
                    queued[user] = true;
                    push(&mut queue, user)?;
                }
            }
        }
        Ok(())
    }
    /// The original events the all-path proof of this node relied on.
    fn witnesses(&self, root: usize) -> Result<Vec<CheckSource>, TypeError> {
        let mut sources = Vec::new();
        if !self.nodes[root].established {
            return Ok(sources);
        }
        if let Some(source) = self.nodes[root].grounded {
            push(&mut sources, source)?;
        }
        let mut seen = filled(self.nodes.len(), false)?;
        let mut work = Vec::new();
        seen[root] = true;
        push(&mut work, root)?;
        while let Some(index) = work.pop() {
            for i in 0..self.nodes[index].links.len() {
                let link = self.nodes[index].links[i];
                if let Some(event) = self.local(index, link) {
                    if !sources.contains(&event.source) {
                        push(&mut sources, event.source)?;
                    }
                } else if self.nodes[link.source].established && !seen[link.source] {
                    seen[link.source] = true;
                    push(&mut work, link.source)?;
                }
            }
        }
        // Witnesses are unique and this ordering must not allocate scratch on
        // the input-derived analysis path, so no stable-sort buffer is used.
        sources.sort_unstable();
        Ok(sources)
    }
    /// The first unsupported dependency on some reaching path of this node.
    fn gap(&self, root: usize) -> Result<Option<CheckGap>, TypeError> {
        let mut seen = filled(self.nodes.len(), false)?;
        let mut work = Vec::new();
        let mut gap = None;
        seen[root] = true;
        push(&mut work, root)?;
        while let Some(index) = work.pop() {
            if gap.is_none() {
                gap = self.nodes[index].gap;
            }
            for i in 0..self.nodes[index].links.len() {
                let link = self.nodes[index].links[i];
                if self.local(index, link).is_some() || seen[link.source] {
                    continue;
                }
                seen[link.source] = true;
                push(&mut work, link.source)?;
            }
        }
        Ok(gap)
    }
}

/// The original `new-instance` that produced `state` at its own definition point
/// and allocates exactly `target`. The value is that allocation's result, so the
/// site's type is the value's type on every execution; no initialization state,
/// reference comparison or declaration is consulted.
fn allocation(
    refs: &References<'_, '_>,
    state: StateId,
    target: crate::TypeId,
) -> Result<Option<CheckSource>, TypeError> {
    let Some(value) = refs.value(state) else {
        return Ok(None);
    };
    let facts = refs.ssa.facts();
    let ValueOrigin::Operation { operation, output } = facts.values()[value.0 as usize].origin()
    else {
        return Ok(None);
    };
    if output != 0
        || !matches!(
            refs.ssa.execution().operations()[operation.0 as usize].kind(),
            OpKind::NewInstance { type_id } if *type_id == target
        )
    {
        return Ok(None);
    }
    Ok(Some(CheckSource::Allocation { operation }))
}

/// Whether a transfer is the normal completion of its source point, not an
/// exceptional one.
fn normal(refs: &References<'_, '_>, transfer: TransferId) -> bool {
    match refs.ssa.facts().transfers()[transfer.0 as usize].origin() {
        TransferOrigin::Edge(edge) => matches!(
            refs.ssa.execution().edges()[edge.0 as usize].role(),
            EdgeRole::Normal(_)
        ),
        TransferOrigin::Continue | TransferOrigin::NormalCommit(_) => true,
        TransferOrigin::Entry | TransferOrigin::Return(_) => false,
    }
}

/// The original event at `from` and the alias relation between this value and the
/// event's value. Only a normal completion proves a successful check, and an
/// `instanceof` proof only holds on the edge whose outcome is nonzero.
fn establishment(
    refs: &References<'_, '_>,
    incoming: &[Vec<TransferId>],
    aliases: &mut Aliases,
    event: Option<Establisher>,
    transfer: TransferId,
    state: StateId,
    from: PointId,
) -> Result<Option<(usize, Establisher)>, TypeError> {
    let Some(event) = event else {
        return Ok(None);
    };
    if !normal(refs, transfer) {
        return Ok(None);
    }
    let origin = refs.ssa.facts().transfers()[transfer.0 as usize].origin();
    if let Some(edge) = event.edge
        && !matches!(origin, TransferOrigin::Edge(source) if source == edge)
    {
        return Ok(None);
    }
    let alias = alias_query(refs, incoming, aliases, state, event.value, from)?;
    Ok(Some((alias, event)))
}

/// The original `instanceof` result whose nonzero outcome this branch edge is.
/// Only a result compared with zero qualifies: the result is 0/1, so any other
/// comparison states nothing about membership.
fn truthy_instance(
    refs: &References<'_, '_>,
    from: PointId,
    edge: EdgeId,
) -> Result<Option<OpId>, TypeError> {
    let facts = refs.ssa.facts();
    let flow = refs.ssa.execution();
    let Some(point) = facts.points().get(from.0 as usize) else {
        return Ok(None);
    };
    let PointKind::Terminal(block) = point.kind() else {
        return Ok(None);
    };
    let Some(Terminal::Branch { op, .. }) = flow.block(block).and_then(|b| b.terminal()) else {
        return Ok(None);
    };
    let Some(EdgeRole::Normal(role)) = flow.edges().get(edge.0 as usize).map(|e| e.role()) else {
        return Ok(None);
    };
    let taken = match role {
        NormalEdge::ConditionalTrue => true,
        NormalEdge::ConditionalFalse => false,
        _ => return Ok(None),
    };
    let uses = facts.terminal_uses(block).unwrap_or(&[]);
    let tested = match op {
        // The zero comparison carries no second operand.
        BranchOp::Nez | BranchOp::Eqz => {
            if (*op == BranchOp::Nez) != taken {
                return Ok(None);
            }
            uses.first().map(|use_binding| use_binding.state())
        }
        // Exactly one operand is the original result and the other is an
        // original zero literal, so the nonzero outcome is that result's true.
        BranchOp::Eq | BranchOp::Ne => {
            if (*op == BranchOp::Ne) != taken {
                return Ok(None);
            }
            match uses {
                [a, b] if refs.is_null(b.state()) => Some(a.state()),
                [a, b] if refs.is_null(a.state()) => Some(b.state()),
                _ => None,
            }
        }
        _ => None,
    };
    match tested {
        Some(state) => instance_of_result(refs, state),
        None => Ok(None),
    }
}

/// The original `instanceof` operation a boolean state carries. A copy keeps the
/// same result; a Phi merges alternative results, so it proves none of them.
fn instance_of_result(
    refs: &References<'_, '_>,
    state: StateId,
) -> Result<Option<OpId>, TypeError> {
    let facts = refs.ssa.facts();
    let mut state = state;
    for _ in 0..facts.values().len() {
        let Some(value) = refs.value(state) else {
            return Ok(None);
        };
        if let ValueOrigin::Operation { operation, output } =
            facts.values()[value.0 as usize].origin()
            && matches!(
                refs.ssa.execution().operations()[operation.0 as usize].kind(),
                OpKind::InstanceOf { .. }
            )
        {
            return Ok((output == 0).then_some(operation));
        }
        match refs.source(state)? {
            Source::Alias { input } => state = input,
            Source::Phi(_) | Source::Leaf => return Ok(None),
        }
    }
    Ok(None)
}

pub(super) fn construct(
    ssa: &SsaMethod<'_>,
    f: &mut TypeFacts,
    work: &Work,
) -> Result<(), TypeError> {
    let flow = ssa.execution();
    if !flow
        .operations()
        .iter()
        .any(|op| matches!(op.kind(), OpKind::Cast { .. }))
    {
        return Ok(());
    }
    let mut decisions: Vec<CheckDisposition> = Vec::new();
    if !work.casts.is_empty() {
        let refs = References::new(ssa, &f.values, &f.names, work)?;
        let mut incoming = lists(ssa.facts().points().len())?;
        for transfer in ssa.facts().transfers() {
            if let Target::Point(target) = transfer.target() {
                push(&mut incoming[target.0 as usize], transfer.id())?;
            }
        }
        let mut instance_at = filled(flow.operations().len(), None)?;
        for (index, instance) in work.instances.iter().enumerate() {
            instance_at[instance.operation.0 as usize] = Some(id(index)?);
        }
        let mut events = filled(ssa.facts().points().len(), None)?;
        for cast in &work.casts {
            let execute = ssa
                .facts()
                .operation_site(cast.operation)
                .ok_or(TypeError::InternalInvariant(
                    "check-cast bound lost its execution point",
                ))?
                .execute();
            events[execute.0 as usize] = Some(Establisher {
                value: cast.receiver,
                target: cast.target_id,
                source: CheckSource::SuccessfulCast {
                    operation: cast.operation,
                },
                edge: None,
            });
        }
        for transfer in ssa.facts().transfers() {
            let TransferOrigin::Edge(edge) = transfer.origin() else {
                continue;
            };
            let from = transfer.from();
            let Some(operation) = truthy_instance(&refs, from, edge)? else {
                continue;
            };
            let Some(index) = instance_at[operation.0 as usize] else {
                continue;
            };
            let instance = &work.instances[index as usize];
            if events[from.0 as usize].is_none() {
                events[from.0 as usize] = Some(Establisher {
                    value: instance.receiver,
                    target: instance.target,
                    source: CheckSource::InstanceOfTrue { operation },
                    edge: Some(edge),
                });
            }
        }
        let mut solver = Solver {
            refs,
            incoming,
            events,
            aliases: Aliases::new(),
            index: HashMap::new(),
            nodes: Vec::new(),
        };
        let mut roots = Vec::new();
        for cast in &work.casts {
            let execute = ssa
                .facts()
                .operation_site(cast.operation)
                .ok_or(TypeError::InternalInvariant(
                    "check-cast bound lost its execution point",
                ))?
                .execute();
            push(
                &mut roots,
                solver.query(cast.receiver, execute, cast.target_id)?,
            )?;
        }
        solver.build()?;
        solver.solve()?;
        for (index, &root) in roots.iter().enumerate() {
            let cast = &work.casts[index];
            let disposition = if solver.nodes[root].established {
                let value =
                    solver
                        .refs
                        .value(cast.receiver)
                        .ok_or(TypeError::InternalInvariant(
                            "redundant check-cast receiver lost its value",
                        ))?;
                CheckDisposition::Redundant(CheckEvidence {
                    receiver: cast.receiver,
                    value,
                    target: cast.target,
                    sources: solver.witnesses(root)?,
                })
            } else if let Some(gap) = solver.gap(root)? {
                CheckDisposition::Unknown(gap)
            } else {
                CheckDisposition::Retained
            };
            push(&mut decisions, disposition)?;
        }
        drop(solver);
    }
    // One entry per original check-cast, in operation order. A cast whose type
    // reference or operand layout never resolved publishes no target name and
    // no omission evidence.
    let mut decisions = decisions.into_iter();
    let mut cursor = 0usize;
    for op in flow.operations() {
        if !matches!(op.kind(), OpKind::Cast { .. }) {
            continue;
        }
        let mut view = CheckView {
            operation: op.id(),
            target: None,
            disposition: CheckDisposition::Unknown(CheckGap::UnresolvedCheck),
        };
        if work
            .casts
            .get(cursor)
            .is_some_and(|cast| cast.operation == op.id())
        {
            view.target = Some(work.casts[cursor].target);
            view.disposition = decisions.next().ok_or(TypeError::InternalInvariant(
                "check-cast disposition lost its operation order",
            ))?;
            cursor += 1;
        }
        push(&mut f.checks, view)?;
    }
    Ok(())
}
