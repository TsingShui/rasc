use super::build::*;
use super::graph::Graph;
use super::protection;
use super::*;
use crate::p2::OpKind;
use crate::p3::ValueId;
use crate::p4::{AllocationRelation, Identity};
use crate::p5::{
    CopyEmission, CopyLocation, Materialization, StorageBinding, StorageGap, StorageMethod,
    StorageRead, TransferSite,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Bottom,
    Known(Option<usize>),
    Unknown(LockGap),
}
struct Stack {
    enter: crate::p2::OpId,
    parent: Option<usize>,
}
struct LockFlow {
    captures: CaptureStability,
    arena: Vec<Stack>,
    intern: HashMap<(crate::p2::OpId, Option<usize>), usize>,
}
fn join(old: State, new: State, node: NodeId) -> State {
    match (old, new) {
        (x, State::Bottom) => x,
        (State::Bottom, x) => x,
        (State::Unknown(g), _) => State::Unknown(g),
        (_, State::Unknown(g)) => State::Unknown(g),
        (State::Known(a), State::Known(b)) if a == b => old,
        _ => State::Unknown(LockGap::Merge(node)),
    }
}
fn expose(state: State, stacks: &[Stack]) -> Result<LockState, RegionError> {
    match state {
        State::Bottom => Ok(LockState::Unreachable),
        State::Unknown(g) => Ok(LockState::Unknown(g)),
        State::Known(mut at) => {
            let mut out = Vec::new();
            while let Some(i) = at {
                push(&mut out, stacks[i].enter)?;
                at = stacks[i].parent;
            }
            out.reverse();
            Ok(LockState::Known(out))
        }
    }
}
fn receiver(storage: &StorageMethod<'_>, op: crate::p2::OpId) -> Result<StorageRead, StorageGap> {
    match storage.facts().operation_uses(op) {
        Some(Ok(uses)) => uses
            .first()
            .map_or(Err(StorageGap::InputLayout(op)), |u| u.read()),
        Some(Err(g)) => Err(g),
        None => Err(StorageGap::InputLayout(op)),
    }
}
fn value(read: StorageRead) -> Option<ValueId> {
    match read {
        StorageRead::Value { value, .. } | StorageRead::Literal { value, .. } => Some(value),
        _ => None,
    }
}
pub(super) fn root(storage: &StorageMethod<'_>, read: StorageRead) -> Option<ValueId> {
    match storage.type_facts().value(value(read)?)?.identity() {
        Identity::Root(v) => Some(v),
        _ => None,
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Write {
    node: NodeId,
    before: bool,
}
struct CaptureStability {
    writers: Vec<Vec<Write>>,
    pairs: HashMap<(crate::p2::OpId, crate::p2::OpId), bool>,
}
impl CaptureStability {
    fn new(storage: &StorageMethod<'_>, f: &RegionFacts, g: &Graph) -> Result<Self, RegionError> {
        let mut writers = lists(storage.facts().locals().len())?;
        for d in storage.facts().definitions() {
            match d.materialization() {
                Materialization::Store(l)
                | Materialization::AllocationCarrier(l)
                | Materialization::ExceptionCapture(l) => push(
                    &mut writers[l.0 as usize],
                    Write {
                        node: g.op_node[d.operation().0 as usize],
                        before: false,
                    },
                )?,
                _ => {}
            }
        }
        for group in storage.facts().construction_groups() {
            for p in group.publications() {
                push(
                    &mut writers[p.local.0 as usize],
                    Write {
                        node: g.op_node[group.initialization().0 as usize],
                        before: false,
                    },
                )?;
            }
        }
        for p in storage.facts().copy_plans() {
            let node = match storage.facts().copy_emission(p.id()) {
                Some(CopyEmission::Construction { group }) => {
                    g.op_node[storage.facts().construction_groups()[group.0 as usize]
                        .allocation()
                        .0 as usize]
                }
                _ => match p.location() {
                    CopyLocation::BeforeThrow(crate::p5::ThrowSite::Operation(op)) => {
                        g.op_node[op.0 as usize]
                    }
                    CopyLocation::BeforeThrow(crate::p5::ThrowSite::Terminal(b)) => {
                        g.terminal_node[b.0 as usize]
                    }
                    CopyLocation::HandlerEntry { block, .. } => Graph::entry(f, block),
                    CopyLocation::Transfer { site, .. } => match site {
                        TransferSite::InitialEntry => NodeId(0),
                        TransferSite::NormalEdge(e) => match f.edges[e.0 as usize].route {
                            EdgeRoute::Normal(l) => f.links[l.0 as usize].from,
                            _ => return Err(bug("normal lock copy lost link")),
                        },
                        TransferSite::AfterCommit(op) | TransferSite::BeforeOperation(op) => {
                            g.op_node[op.0 as usize]
                        }
                        TransferSite::BeforeTerminal(b) => g.terminal_node[b.0 as usize],
                    },
                },
            };
            let before = matches!(
                p.location(),
                CopyLocation::BeforeThrow(_)
                    | CopyLocation::HandlerEntry { .. }
                    | CopyLocation::Transfer {
                        site: TransferSite::InitialEntry
                            | TransferSite::BeforeOperation(_)
                            | TransferSite::BeforeTerminal(_),
                        ..
                    }
            );
            let write = Write { node, before };
            for a in p.assignments() {
                if a.issue().is_none()
                    && let Ok(local) = a.destination()
                    && !writers[local.0 as usize].contains(&write)
                {
                    push(&mut writers[local.0 as usize], write)?;
                }
            }
        }
        Ok(Self {
            writers,
            pairs: HashMap::new(),
        })
    }
    fn same(
        &mut self,
        storage: &StorageMethod<'_>,
        f: &RegionFacts,
        g: &Graph,
        enter: crate::p2::OpId,
        exit: crate::p2::OpId,
    ) -> Result<bool, RegionError> {
        if let Some(&answer) = self.pairs.get(&(enter, exit)) {
            return Ok(answer);
        }
        let answer = self.compare(storage, f, g, enter, exit)?;
        self.pairs
            .try_reserve(1)
            .map_err(crate::RequestError::Allocation)?;
        self.pairs.insert((enter, exit), answer);
        Ok(answer)
    }
    fn compare(
        &self,
        storage: &StorageMethod<'_>,
        f: &RegionFacts,
        g: &Graph,
        enter: crate::p2::OpId,
        exit: crate::p2::OpId,
    ) -> Result<bool, RegionError> {
        let (Ok(a), Ok(b)) = (receiver(storage, enter), receiver(storage, exit)) else {
            return Ok(false);
        };
        let local = if a.local().is_some() && a.local() == b.local() {
            a.local()
        } else {
            match (root(storage, a), root(storage, b)) {
                (Some(a), Some(b)) if a == b => {
                    if !storage
                        .type_facts()
                        .value(a)
                        .ok_or(bug("lock root lost type facts"))?
                        .allocations()
                        .is_empty()
                    {
                        for op in [enter, exit] {
                            if !storage.type_facts().initialization().iter().find(|u|matches!(u.site,crate::p3::UseSite::Operation{operation,argument:0}if operation==op)).and_then(|u|u.allocation).is_some_and(|a|a.relation==AllocationRelation::Same){return Ok(false);}
                        }
                    }
                    storage.facts().binding(a).and_then(StorageBinding::local)
                }
                _ => None,
            }
        };
        let Some(local) = local else {
            return Ok(false);
        };
        if self.writers[local.0 as usize].is_empty() {
            return Ok(true);
        }
        let begin = g.op_node[enter.0 as usize];
        let end = g.op_node[exit.0 as usize];
        let mut excluded = filled(f.nodes.len(), false)?;
        excluded[begin.0 as usize] = true;
        let mut starts = Vec::new();
        for &link in &g.outgoing[begin.0 as usize] {
            push(&mut starts, f.links[link.0 as usize].to)?;
        }
        let between = protection::reachable(storage, f, g, &starts, Some(end), &excluded, true)?;
        Ok(!self.writers[local.0 as usize].iter().any(|write| {
            if write.node == begin {
                !write.before
            } else if write.node == end {
                write.before
            } else {
                between[write.node.0 as usize]
            }
        }))
    }
}
fn transfer(
    storage: &StorageMethod<'_>,
    f: &RegionFacts,
    g: &Graph,
    flow: &mut LockFlow,
    node: NodeId,
    before: State,
) -> Result<(State, State, Option<crate::p2::OpId>, bool), RegionError> {
    let LockFlow {
        captures: refs,
        arena,
        intern,
    } = flow;
    let part = &f.nodes[node.0 as usize];
    let monitor = part.operations.clone().find(|&i| {
        matches!(
            storage.execution().operations()[i as usize].kind(),
            OpKind::Monitor { .. }
        )
    });
    let Some(index) = monitor else {
        return Ok((before, before, None, false));
    };
    let op = &storage.execution().operations()[index as usize];
    if before == State::Bottom {
        return Ok((before, before, None, false));
    }
    if let Err(gap) = receiver(storage, op.id()) {
        return Ok((State::Unknown(LockGap::Storage(gap)), before, None, false));
    }
    let OpKind::Monitor { enter } = op.kind() else {
        return Err(bug("monitor range changed kind"));
    };
    let State::Known(top) = before else {
        return Ok((before, before, None, false));
    };
    if *enter {
        let mut at = top;
        while let Some(i) = at {
            if arena[i].enter == op.id() {
                return Ok((
                    State::Unknown(LockGap::RepeatedAcquire(op.id())),
                    before,
                    None,
                    false,
                ));
            }
            at = arena[i].parent;
        }
        let next = if let Some(&i) = intern.get(&(op.id(), top)) {
            i
        } else {
            let i = arena.len();
            push(
                arena,
                Stack {
                    enter: op.id(),
                    parent: top,
                },
            )?;
            intern
                .try_reserve(1)
                .map_err(crate::RequestError::Allocation)?;
            intern.insert((op.id(), top), i);
            i
        };
        Ok((State::Known(Some(next)), before, None, false))
    } else {
        let Some(top) = top else {
            return Ok((
                State::Unknown(LockGap::NoLocalAcquire(op.id())),
                State::Unknown(LockGap::NoLocalAcquire(op.id())),
                None,
                false,
            ));
        };
        let acquired = arena[top].enter;
        if refs.same(storage, f, g, acquired, op.id())? {
            Ok((
                State::Known(arena[top].parent),
                State::Bottom,
                Some(acquired),
                true,
            ))
        } else {
            let bad = State::Unknown(LockGap::Reference {
                enter: acquired,
                exit: op.id(),
            });
            Ok((bad, bad, None, false))
        }
    }
}
pub(super) struct LockPositions<'s> {
    states: &'s [State],
    arena: &'s [Stack],
}
impl LockPositions<'_> {
    pub fn held(&self, node: NodeId, enter: crate::p2::OpId) -> Result<(), LockSeparation> {
        let mut at = match self.states[node.0 as usize] {
            State::Known(at) => at,
            State::Unknown(gap) => return Err(LockSeparation::State(gap)),
            State::Bottom => return Err(LockSeparation::Unreachable),
        };
        while let Some(index) = at {
            if self.arena[index].enter == enter {
                return Ok(());
            }
            at = self.arena[index].parent;
        }
        Err(LockSeparation::NotHeld(node))
    }
}
pub(super) fn construct(
    storage: &StorageMethod<'_>,
    f: &mut RegionFacts,
    g: &Graph,
) -> Result<(), RegionError> {
    if !storage
        .execution()
        .operations()
        .iter()
        .any(|op| matches!(op.kind(), OpKind::Monitor { .. }))
    {
        return Ok(());
    }
    let mut flow = LockFlow {
        captures: CaptureStability::new(storage, f, g)?,
        arena: Vec::new(),
        intern: HashMap::new(),
    };
    let mut states = filled(f.nodes.len(), State::Bottom)?;
    states[0] = State::Known(None);
    let mut queue = Vec::new();
    let mut queued = filled(f.nodes.len(), false)?;
    push(&mut queue, NodeId(0))?;
    queued[0] = true;
    while let Some(node) = queue.pop() {
        queued[node.0 as usize] = false;
        let (normal, exception, _, _) =
            transfer(storage, f, g, &mut flow, node, states[node.0 as usize])?;
        let mut targets = Vec::new();
        for &l in &g.outgoing[node.0 as usize] {
            push(&mut targets, (f.links[l.0 as usize].to, normal))?;
        }
        for &e in &g.exceptions[node.0 as usize] {
            if let Some(to) = Graph::exception_target(f, storage, e) {
                push(&mut targets, (to, exception))?;
            }
        }
        for (to, input) in targets {
            let merged = join(states[to.0 as usize], input, to);
            if merged != states[to.0 as usize] {
                states[to.0 as usize] = merged;
                if !queued[to.0 as usize] {
                    queued[to.0 as usize] = true;
                    push(&mut queue, to)?;
                }
            }
        }
    }
    for op in storage.execution().operations() {
        if !matches!(op.kind(), OpKind::Monitor { .. }) {
            continue;
        }
        let node = g.op_node[op.id().0 as usize];
        let (normal, exception, matched, no_throw) =
            transfer(storage, f, g, &mut flow, node, states[node.0 as usize])?;
        if let State::Unknown(gap) = normal {
            push(
                &mut f.issues,
                PlanningIssue::Lock {
                    operation: op.id(),
                    gap,
                },
            )?;
        }
        if no_throw {
            for &edge in &g.exceptions[node.0 as usize] {
                if let EdgeRoute::Exception { delivery, .. } = &mut f.edges[edge.0 as usize].route {
                    *delivery = ExceptionDelivery::ImpossibleRelease(op.id());
                }
            }
        }
        push(
            &mut f.locks,
            LockEvent {
                operation: op.id(),
                receiver: receiver(storage, op.id()),
                before: expose(states[node.0 as usize], &flow.arena)?,
                normal: expose(normal, &flow.arena)?,
                exception: expose(exception, &flow.arena)?,
                matched,
                release_cannot_throw: no_throw,
            },
        )?;
    }
    super::scopes::construct(
        storage,
        f,
        g,
        LockPositions {
            states: &states,
            arena: &flow.arena,
        },
    )
}
