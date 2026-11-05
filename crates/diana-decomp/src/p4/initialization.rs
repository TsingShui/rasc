mod references;

use super::build::*;
use super::constraints::Work;
use super::dataflow::Dataflow;
use super::references::*;
use super::*;
use crate::RequestError;
use crate::p1::CheckedReferenceFact;
use crate::p2::{EdgeRole, InvokeKind, OpId, OpKind};
use crate::p3::{PointId, SsaMethod, StateId, Target, TransferId, TransferOrigin};
use crate::raw::ReferenceSlot;
use references::*;

const INIT_NAME: [u16; 6] = crate::substrate::owned::ascii_units(*b"<init>");
const BEFORE: u32 = 1;
const INITIALIZED: u32 = 2;
const FAILED: u32 = 4;
const UNKNOWN: u32 = 8;
const CONFLICT: u32 = 16;
const UNAVAILABLE: u32 = 32;
const ALIAS_GAP: u32 = 64;
const NULL: u32 = 128;

#[derive(Clone, Copy)]
enum CallKind {
    Constructor { owner: crate::TypeId, valid: bool },
    UnresolvedDirect,
}
struct Call {
    operation: OpId,
    receiver: Option<StateId>,
    identity: Identity,
    kind: CallKind,
    owners: u32,
    owner_gap: Option<crate::p0::SuperclassGap>,
}
fn calls(ssa: &SsaMethod<'_>, f: &TypeFacts) -> Result<Vec<Call>, TypeError> {
    let flow = ssa.execution();
    let mut calls = Vec::new();
    for op in flow.operations() {
        let kind = match op.kind() {
            OpKind::Invoke { kind, .. } | OpKind::UnresolvedInvoke { kind, .. } => *kind,
            _ => continue,
        };
        let has_receiver = !matches!(kind, InvokeKind::Static | InvokeKind::Custom);
        let kind = match flow.reference_fact(op.pc(), ReferenceSlot::First) {
            Some(Ok(CheckedReferenceFact::Method {
                name,
                owner_type,
                prototype,
                ..
            })) => {
                if !name.matches(&INIT_NAME).map_err(RequestError::from)? {
                    continue;
                }
                CallKind::Constructor {
                    owner: *owner_type,
                    valid: kind == InvokeKind::Direct
                        && prototype.result().is_none()
                        && flow
                            .reference_use_issue(op.pc(), ReferenceSlot::First)
                            .is_none(),
                }
            }
            _ if kind == InvokeKind::Direct => CallKind::UnresolvedDirect,
            _ => continue,
        };
        let receiver = if has_receiver {
            ssa.facts()
                .operation_uses(op.id())
                .and_then(Result::ok)
                .and_then(|uses| uses.first())
                .map(|u| u.state())
        } else {
            None
        };
        let identity = receiver
            .and_then(|s| state_value(ssa.facts(), s))
            .map_or(Identity::Unknown, |v| f.values[v.0 as usize].identity);
        push(
            &mut calls,
            Call {
                operation: op.id(),
                receiver,
                identity,
                kind,
                owners: 0,
                owner_gap: None,
            },
        )?;
    }
    Ok(calls)
}
fn relation(bits: u32, gap: Option<crate::p0::SuperclassGap>) -> ConstructorRelation {
    match bits {
        ALLOCATION => ConstructorRelation::Allocation,
        THIS => ConstructorRelation::This,
        SUPER => ConstructorRelation::Super,
        ANCESTOR => ConstructorRelation::Ancestor,
        OTHER_OWNER => ConstructorRelation::OtherOwner,
        NOT_ALLOCATION => ConstructorRelation::NotAllocation,
        _ if bits & MISSING_REFERENCE != 0 => {
            ConstructorRelation::Unknown(MissingFact::ReferenceDeclaration)
        }
        _ if bits & MISSING_OWNER != 0 => gap.map_or(
            ConstructorRelation::Unknown(MissingFact::InitializationOwnerRelation),
            ConstructorRelation::UnknownOwner,
        ),
        _ if bits & MISSING_RECEIVER != 0 => {
            ConstructorRelation::Unknown(MissingFact::InitializationReceiver)
        }
        _ => ConstructorRelation::AmbiguousIdentity,
    }
}
fn eligible(owners: u32) -> bool {
    owners != 0 && owners & !ELIGIBLE == 0
}
fn pending_owner_is_known(owners: u32) -> bool {
    // Non-allocation sources have no pending constructor obligation. They do
    // not invalidate a matched allocation/this source's successful transition.
    owners & ELIGIBLE != 0 && owners & !(ELIGIBLE | NOT_ALLOCATION) == 0
}
fn after_constructor(bits: u32, known: bool, exception: bool) -> u32 {
    let mut out = bits & (UNKNOWN | CONFLICT | UNAVAILABLE | ALIAS_GAP | NULL);
    if bits & BEFORE != 0 {
        out |= if exception {
            FAILED
        } else if known {
            INITIALIZED
        } else {
            UNKNOWN
        };
    }
    if bits & INITIALIZED != 0 {
        out |= CONFLICT;
    }
    if bits & FAILED != 0 {
        out |= if exception { FAILED } else { CONFLICT };
    }
    out
}
fn classify(bits: u32) -> Initialization {
    if bits & UNAVAILABLE != 0 {
        return Initialization::SourceUnavailable;
    }
    let bits = (bits & !(ALIAS_GAP | NULL)) | if bits & NULL != 0 { INITIALIZED } else { 0 };
    match bits {
        BEFORE => Initialization::BeforeConstructor,
        INITIALIZED => Initialization::Initialized,
        FAILED => Initialization::ConstructorFailed,
        0 | UNKNOWN => Initialization::Unknown,
        CONFLICT => Initialization::ConstructorStateConflict,
        // Only the five public initialization-state bits remain after removing
        // the source-availability and dependency-diagnostic flags above.
        bits => Initialization::Mixed(InitializationStates(bits as u8)),
    }
}
fn exceptional(ssa: &SsaMethod<'_>, transfer: TransferId) -> bool {
    match ssa.facts().transfers()[transfer.0 as usize].origin() {
        TransferOrigin::Edge(edge) => matches!(
            ssa.execution().edges()[edge.0 as usize].role(),
            EdgeRole::Exception { .. }
        ),
        _ => false,
    }
}
fn reference_use(ssa: &SsaMethod<'_>, f: &TypeFacts, index: usize) -> bool {
    let u = &f.uses[index];
    match u.interpretation {
        Interpretation::Direct(Family::Reference) | Interpretation::Null => true,
        Interpretation::Unknown(Problem::SourceUnavailable(_)) => {
            u.requirement.families.selected() == Some(Family::Reference)
        }
        Interpretation::Unknown(_) => {
            u.requirement.families.selected() == Some(Family::Reference)
                && state_value(ssa.facts(), u.state).is_some_and(|v| {
                    let value = &f.values[v.0 as usize];
                    if let Some(bits) = value.literal() {
                        bits.width == crate::SlotWidth::One && bits.value == 0
                    } else {
                        value.families.contains(Family::Reference)
                    }
                })
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct InitQuery {
    state: StateId,
    other: Option<StateId>,
    point: PointId,
}
#[derive(Clone, Copy)]
enum Effect {
    Plain,
    Partition {
        alias: usize,
    },
    Call {
        index: usize,
        observer_alias: Option<usize>,
        other_alias: Option<usize>,
        collapse: bool,
        exception: bool,
    },
}
const ROW_BITS: u32 = 8;
const ROW_MASK: u32 = (1 << ROW_BITS) - 1;
const RELATIONS: [u32; 3] = [SAME, DIFFERENT, MAY_ALIAS];

fn uncertain(bits: u32, alias_gap: bool) -> u32 {
    // A possibly aliased call cannot revoke an established initialized state.
    // Confirmed same-object constructor calls still detect repeated construction.
    if bits & (BEFORE | FAILED | UNKNOWN | CONFLICT) != 0 {
        bits | UNKNOWN | if alias_gap { ALIAS_GAP } else { 0 }
    } else {
        bits
    }
}
fn apply_call(call: &Call, alias: u32, exception: bool, bits: u32) -> u32 {
    if alias == DIFFERENT {
        return bits;
    }
    match call.kind {
        CallKind::Constructor { .. } if alias == SAME => {
            after_constructor(bits, pending_owner_is_known(call.owners), exception)
        }
        _ => uncertain(
            bits,
            alias != SAME && call.receiver.is_some() && call.owners & MISSING_RECEIVER == 0,
        ),
    }
}
struct Solver<'s, 'a> {
    refs: References<'s, 'a>,
    calls: Vec<Call>,
    incoming: Vec<Vec<TransferId>>,
    events: Vec<Option<usize>>,
    aliases: Aliases,
    init: Dataflow<InitQuery, Effect>,
}
impl Solver<'_, '_> {
    fn query(
        &mut self,
        state: StateId,
        other: Option<StateId>,
        point: PointId,
    ) -> Result<usize, TypeError> {
        let point = if other.is_none() && (self.calls.is_empty() || self.refs.is_null(state)) {
            self.refs.definition(state).unwrap_or(point)
        } else {
            self.refs
                .canonical(state, other, point, &self.incoming, Some(&self.events))
        };
        self.init.intern(InitQuery {
            state,
            other,
            point,
        })
    }
    fn partition(
        &mut self,
        target: usize,
        query: InitQuery,
        other: StateId,
    ) -> Result<(), TypeError> {
        let source = self.query(query.state, None, query.point)?;
        let alias = alias_query(
            &self.refs,
            &self.incoming,
            &mut self.aliases,
            query.state,
            other,
            query.point,
        )?;
        self.init.link(target, source, Effect::Partition { alias })
    }
    fn link(
        &mut self,
        target: usize,
        state: StateId,
        other: Option<StateId>,
        transfer: TransferId,
    ) -> Result<(), TypeError> {
        let t = &self.refs.ssa.facts().transfers()[transfer.0 as usize];
        let point = t.from();
        let mut compared = other;
        let effect = if let Some(index) = self.events[point.0 as usize] {
            let call = &self.calls[index];
            let mut observer_alias = None;
            let mut other_alias = None;
            if let Some(receiver) = call.receiver {
                let alias = alias_query(
                    &self.refs,
                    &self.incoming,
                    &mut self.aliases,
                    state,
                    receiver,
                    point,
                )?;
                if self.refs.is_null(state)
                    && call.owners != 0
                    && call.owners & !(ELIGIBLE | OTHER_OWNER) == 0
                {
                    self.aliases.nodes[alias].seed = DIFFERENT;
                }
                if let Some(other) = other {
                    observer_alias = Some(alias);
                    other_alias = Some(alias_query(
                        &self.refs,
                        &self.incoming,
                        &mut self.aliases,
                        other,
                        receiver,
                        point,
                    )?);
                } else {
                    compared = Some(receiver);
                }
            }
            Effect::Call {
                index,
                observer_alias,
                other_alias,
                collapse: other.is_none(),
                exception: exceptional(self.refs.ssa, transfer),
            }
        } else {
            Effect::Plain
        };
        let source = self.query(state, compared, point)?;
        self.init.link(target, source, effect)
    }
    fn build(&mut self) -> Result<(), TypeError> {
        let mut index = 0;
        while index < self.init.nodes.len() {
            let query = self.init.nodes[index].key;
            let InitQuery {
                state,
                other,
                point,
            } = query;
            if !self.refs.available(state) {
                self.init.nodes[index].seed |= if other.is_some() {
                    UNAVAILABLE << (2 * ROW_BITS)
                } else {
                    UNAVAILABLE
                };
            }
            let source = self.refs.source(state)?;
            let at_birth =
                self.refs.definition(state) == Some(point) && matches!(source, Source::Leaf);
            if let Some(other) = other {
                let other_at_birth = self.refs.definition(other) == Some(point)
                    && matches!(self.refs.source(other)?, Source::Leaf);
                if state == other
                    || self.refs.is_null(state)
                    || at_birth
                    || other_at_birth
                    || self.refs.definition(state).is_none()
                    || self.refs.definition(other).is_none()
                {
                    self.partition(index, query, other)?;
                    index += 1;
                    continue;
                }
            } else if self.refs.is_null(state) {
                self.init.nodes[index].seed = NULL;
                index += 1;
                continue;
            } else if self.refs.definition(state).is_none() {
                self.init.nodes[index].seed = UNAVAILABLE;
                index += 1;
                continue;
            } else if at_birth {
                self.init.nodes[index].seed = self.refs.birth(state);
                index += 1;
                continue;
            }
            // Both captured bindings cross the same original transfer before
            // their facts are joined. Phi alternatives are not Cartesian products.
            for i in 0..self.incoming[point.0 as usize].len() {
                let transfer = self.incoming[point.0 as usize][i];
                let state = self.refs.rewrite(state, point, transfer)?;
                let other = other
                    .map(|s| self.refs.rewrite(s, point, transfer))
                    .transpose()?;
                self.link(index, state, other, transfer)?;
            }
            index += 1;
        }
        aliases(&self.refs, &self.incoming, &mut self.aliases)?;
        let calls = &self.calls;
        let aliases = &self.aliases;
        self.init.solve(
            |q| {
                if q.other.is_some() {
                    (UNKNOWN | ALIAS_GAP) << (2 * ROW_BITS)
                } else {
                    UNKNOWN | ALIAS_GAP
                }
            },
            |effect, bits| match effect {
                Effect::Plain => bits,
                Effect::Partition { alias } => {
                    let mut out = 0;
                    for (row, relation) in RELATIONS.iter().enumerate() {
                        if aliases.nodes[alias].bits & relation != 0 {
                            out |= bits << (row as u32 * ROW_BITS);
                        }
                    }
                    out
                }
                Effect::Call {
                    index,
                    observer_alias,
                    other_alias,
                    collapse,
                    exception,
                } => {
                    // A paired query stores initialization states separately for
                    // equal, distinct, and unproved reference relationships.
                    let mut out = 0;
                    let paired = !collapse || calls[index].receiver.is_some();
                    if !paired {
                        return uncertain(bits, false);
                    }
                    for (row, &relation) in RELATIONS.iter().enumerate() {
                        let state = (bits >> (row as u32 * ROW_BITS)) & ROW_MASK;
                        if state == 0 {
                            continue;
                        }
                        let observer = observer_alias.map_or(MAY_ALIAS, |a| aliases.nodes[a].bits);
                        let other = other_alias.map_or(MAY_ALIAS, |a| aliases.nodes[a].bits);
                        let alias = if collapse || other == SAME {
                            relation
                        } else if relation == SAME {
                            other
                        } else {
                            observer
                        };
                        let updated = apply_call(&calls[index], alias, exception, state);
                        if collapse {
                            out |= updated;
                        } else {
                            out |= updated << (row as u32 * ROW_BITS);
                        }
                    }
                    out
                }
            },
        )
    }
}

pub(super) fn construct(
    ssa: &SsaMethod<'_>,
    f: &mut TypeFacts,
    work: &Work,
) -> Result<(), TypeError> {
    super::allocation::sources(ssa, f, work)?;
    let refs = References::new(ssa, &f.values, &f.names, work)?;
    let mut calls = calls(ssa, f)?;
    owners(&refs, &mut calls)?;
    for call in &calls {
        let site = IssueSite::Operation(call.operation);
        if matches!(call.kind, CallKind::Constructor { .. }) {
            let relation = relation(call.owners, call.owner_gap);
            push(
                &mut f.constructors,
                ConstructorFact {
                    operation: call.operation,
                    receiver: call.receiver,
                    identity: call.identity,
                    relation,
                    allocation: None,
                },
            )?;
            if relation == ConstructorRelation::AmbiguousIdentity && !eligible(call.owners) {
                push(
                    &mut f.issues,
                    TypeIssue {
                        site,
                        problem: Problem::AmbiguousIdentity,
                    },
                )?;
            }
        }
        for (flag, problem) in [
            (OTHER_OWNER, Problem::ConstructorOwnerMismatch),
            (
                MISSING_REFERENCE,
                Problem::Missing(MissingFact::ReferenceDeclaration),
            ),
            (
                MISSING_RECEIVER,
                Problem::Missing(MissingFact::InitializationReceiver),
            ),
            (
                MISSING_OWNER,
                Problem::Missing(MissingFact::InitializationOwnerRelation),
            ),
            (OWNER_UNAVAILABLE, Problem::OwnerDeclarationUnavailable),
        ] {
            if call.owners & flag != 0 {
                push(&mut f.issues, TypeIssue { site, problem })?;
            }
        }
    }

    let mut incoming = lists(ssa.facts().points().len())?;
    for t in ssa.facts().transfers() {
        if let Target::Point(target) = t.target() {
            push(&mut incoming[target.0 as usize], t.id())?;
        }
    }
    let mut events = filled(ssa.facts().points().len(), None)?;
    for (index, call) in calls.iter().enumerate() {
        let execute = ssa
            .facts()
            .operation_site(call.operation)
            .ok_or(TypeError::InternalInvariant(
                "initialization event has no execution point",
            ))?
            .execute();
        events[execute.0 as usize] = Some(index);
    }
    let mut solver = Solver {
        refs,
        calls,
        incoming,
        events,
        aliases: Aliases::new(),
        init: Dataflow::new(),
    };
    let mut queries = Vec::new();
    for (index, u) in ssa.facts().uses().iter().enumerate() {
        if reference_use(ssa, f, index) {
            let query = solver.query(u.state(), None, u.point())?;
            let allocation = solver.refs.value(u.state()).and_then(|v| {
                let [operation] = solver.refs.values[v.0 as usize].allocations.as_slice() else {
                    return None;
                };
                Some(*operation)
            });
            let allocation = if let Some(operation) = allocation {
                let output = ssa
                    .facts()
                    .operation_outputs(operation)
                    .and_then(|o| o.first())
                    .ok_or(TypeError::InternalInvariant(
                        "allocation origin lost its output",
                    ))?;
                let alias = if ssa.facts().dominates(output.point(), u.point()) {
                    Some(alias_query(
                        &solver.refs,
                        &solver.incoming,
                        &mut solver.aliases,
                        u.state(),
                        output.state(),
                        u.point(),
                    )?)
                } else {
                    None
                };
                Some((operation, alias))
            } else {
                None
            };
            push(&mut queries, (index, query, allocation))?;
        }
    }
    solver.build()?;
    for (index, query, allocation) in queries {
        let u = &ssa.facts().uses()[index];
        let bits = solver.init.nodes[query].bits;
        push(
            &mut f.initialization,
            InitializationUse {
                site: u.site(),
                point: u.point(),
                state: classify(bits),
                allocation: allocation.map(|(operation, alias)| AllocationBinding {
                    operation,
                    relation: match alias.map(|a| solver.aliases.nodes[a].bits) {
                        Some(SAME) => AllocationRelation::Same,
                        Some(DIFFERENT) => AllocationRelation::Different,
                        _ => AllocationRelation::Unknown,
                    },
                }),
            },
        )?;
        if bits & ALIAS_GAP != 0 {
            push(
                &mut f.issues,
                TypeIssue {
                    site: IssueSite::Use(u.site()),
                    problem: Problem::AmbiguousIdentity,
                },
            )?;
        }
        if bits & CONFLICT != 0 {
            push(
                &mut f.issues,
                TypeIssue {
                    site: IssueSite::Use(u.site()),
                    problem: Problem::InitializationStateConflict,
                },
            )?;
        }
    }
    for constructor in &mut f.constructors {
        constructor.allocation = f.initialization.iter().find(|u| matches!(u.site, crate::p3::UseSite::Operation { operation, argument: 0 } if operation == constructor.operation)).and_then(|u| u.allocation);
    }
    Ok(())
}
