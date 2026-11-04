use super::build::*;
use super::*;
use crate::RequestError;
use crate::p1::CheckedReferenceFact;
use crate::p2::{EdgeRole, InvokeKind, OpId, OpKind};
use crate::p3::{EntryRole, SsaMethod, Target, TransferOrigin, ValueId, ValueOrigin};
use crate::raw::ReferenceSlot;

const INIT_NAME: [u16; 6] = crate::substrate::owned::ascii_units(*b"<init>");

#[derive(Clone, Copy)]
enum InitializationEvent {
    Constructor(usize),
    UnresolvedDirect(Identity),
}

fn receiver(
    ssa: &SsaMethod<'_>,
    facts: &TypeFacts,
    operation: OpId,
) -> (Option<crate::p3::StateId>, Identity) {
    let state = ssa
        .facts()
        .operation_uses(operation)
        .and_then(Result::ok)
        .and_then(|uses| uses.first())
        .map(|u| u.state());
    let identity = state
        .and_then(|state| state_value(ssa.facts(), state))
        .and_then(|value| facts.value(value))
        .map_or(Identity::Unknown, |value| value.identity);
    (state, identity)
}
fn exceptional(ssa: &SsaMethod<'_>, origin: TransferOrigin) -> bool {
    match origin {
        TransferOrigin::Edge(id) => ssa
            .execution()
            .edges()
            .get(id.0 as usize)
            .is_some_and(|e| matches!(e.role(), EdgeRole::Exception { .. })),
        _ => false,
    }
}
fn tracked(ssa: &SsaMethod<'_>, value: ValueId) -> bool {
    match ssa.facts().values()[value.0 as usize].origin() {
        ValueOrigin::Entry(EntryRole::This) => {
            ssa.execution().declaration().name.units() == INIT_NAME
        }
        ValueOrigin::Operation { operation, .. } => matches!(
            ssa.execution().operations()[operation.0 as usize].kind(),
            OpKind::NewInstance { .. }
        ),
        _ => false,
    }
}
fn disjoint(ssa: &SsaMethod<'_>, a: ValueId, b: ValueId) -> bool {
    fn fresh(ssa: &SsaMethod<'_>, v: ValueId) -> bool {
        match ssa.facts().values()[v.0 as usize].origin() {
            ValueOrigin::Operation { operation, .. } => matches!(
                ssa.execution().operations()[operation.0 as usize].kind(),
                OpKind::NewInstance { .. }
                    | OpKind::NewArray { .. }
                    | OpKind::FilledNewArray { .. }
            ),
            _ => false,
        }
    }
    fn entry(ssa: &SsaMethod<'_>, v: ValueId) -> bool {
        matches!(
            ssa.facts().values()[v.0 as usize].origin(),
            ValueOrigin::Entry(_)
        )
    }
    a != b
        && ((fresh(ssa, a) && (fresh(ssa, b) || entry(ssa, b))) || (entry(ssa, a) && fresh(ssa, b)))
}
fn after_constructor(bits: u8, known: bool, exception: bool) -> u8 {
    let mut out = bits & (8 | 16);
    if bits & 1 != 0 {
        out |= if exception {
            4
        } else if known {
            2
        } else {
            8
        };
    }
    if bits & 2 != 0 {
        out |= 16;
    }
    if bits & 4 != 0 {
        out |= if exception { 4 } else { 16 };
    }
    out
}
fn classify(bits: u8) -> Initialization {
    match bits {
        1 => Initialization::BeforeConstructor,
        2 => Initialization::Initialized,
        4 => Initialization::ConstructorFailed,
        0 | 8 => Initialization::Unknown,
        16 => Initialization::ConstructorStateConflict,
        _ => Initialization::Mixed(InitializationStates(bits)),
    }
}
pub(super) fn construct(ssa: &SsaMethod<'_>, f: &mut TypeFacts) -> Result<(), TypeError> {
    let s = ssa.facts();
    let flow = ssa.execution();
    let mut unresolved_direct = Vec::new();
    for op in flow.operations() {
        let kind = match op.kind() {
            OpKind::Invoke { kind, .. } | OpKind::UnresolvedInvoke { kind, .. } => *kind,
            _ => continue,
        };
        let Some(Ok(CheckedReferenceFact::Method {
            name,
            owner_type,
            prototype,
            ..
        })) = flow.reference_fact(op.pc(), ReferenceSlot::First)
        else {
            if kind == InvokeKind::Direct {
                let (_, identity) = receiver(ssa, f, op.id());
                push(&mut unresolved_direct, (op.id(), identity))?;
                issue(
                    f,
                    IssueSite::Operation(op.id()),
                    Problem::Missing(MissingFact::ReferenceDeclaration),
                )?;
            }
            continue;
        };
        if !name.matches(&INIT_NAME).map_err(RequestError::from)? {
            continue;
        }
        let (receiver, identity) = receiver(ssa, f, op.id());
        let relation = if kind != InvokeKind::Direct
            || prototype.result().is_some()
            || flow
                .reference_use_issue(op.pc(), ReferenceSlot::First)
                .is_some()
        {
            ConstructorRelation::Unknown(MissingFact::ReferenceDeclaration)
        } else if let Identity::Root(root) = identity {
            match s.values()[root.0 as usize].origin() {
                ValueOrigin::Entry(EntryRole::This) => {
                    let current = flow.owner_facts();
                    if flow.declaration().name.units() != INIT_NAME {
                        ConstructorRelation::NotAllocation
                    } else if *owner_type == current.type_id() {
                        ConstructorRelation::This
                    } else {
                        match current.superclass() {
                            crate::p0::Fact::Present(_)
                                if Some(*owner_type) == current.superclass_type_id() =>
                            {
                                ConstructorRelation::Super
                            }
                            crate::p0::Fact::Present(_) | crate::p0::Fact::Absent => {
                                ConstructorRelation::OtherOwner
                            }
                            crate::p0::Fact::Unavailable(_) => {
                                issue(
                                    f,
                                    IssueSite::Operation(op.id()),
                                    Problem::OwnerDeclarationUnavailable,
                                )?;
                                ConstructorRelation::Unknown(
                                    MissingFact::InitializationOwnerRelation,
                                )
                            }
                            crate::p0::Fact::NotRead => ConstructorRelation::Unknown(
                                MissingFact::InitializationOwnerRelation,
                            ),
                        }
                    }
                }
                ValueOrigin::Operation { operation, .. }
                    if matches!(
                        flow.operations()[operation.0 as usize].kind(),
                        OpKind::NewInstance { .. }
                    ) =>
                {
                    let allocated = &flow.operations()[operation.0 as usize];
                    if flow
                        .reference_use_issue(allocated.pc(), ReferenceSlot::First)
                        .is_some()
                        || !matches!(
                            flow.reference_fact(allocated.pc(), ReferenceSlot::First),
                            Some(Ok(CheckedReferenceFact::Type { .. }))
                        )
                    {
                        ConstructorRelation::Unknown(MissingFact::ReferenceDeclaration)
                    } else if matches!(allocated.kind(),OpKind::NewInstance {type_id} if type_id==owner_type)
                    {
                        ConstructorRelation::Allocation
                    } else {
                        ConstructorRelation::OtherOwner
                    }
                }
                _ => ConstructorRelation::NotAllocation,
            }
        } else if identity == Identity::Multiple {
            ConstructorRelation::AmbiguousIdentity
        } else {
            ConstructorRelation::Unknown(MissingFact::InitializationReceiver)
        };
        if relation == ConstructorRelation::OtherOwner {
            issue(
                f,
                IssueSite::Operation(op.id()),
                Problem::ConstructorOwnerMismatch,
            )?;
        }
        if relation == ConstructorRelation::AmbiguousIdentity {
            issue(f, IssueSite::Operation(op.id()), Problem::AmbiguousIdentity)?;
        }
        if let ConstructorRelation::Unknown(missing) = relation {
            issue(f, IssueSite::Operation(op.id()), Problem::Missing(missing))?;
        }
        push(
            &mut f.constructors,
            ConstructorFact {
                operation: op.id(),
                receiver,
                identity,
                relation,
            },
        )?;
    }
    let mut use_indices = lists(f.values.len())?;
    for (i, u) in s.uses().iter().enumerate() {
        let Some(value) = s.readable_value(u) else {
            if f.uses[i].requirement.families.contains(Family::Reference) {
                push(
                    &mut f.initialization,
                    InitializationUse {
                        site: u.site(),
                        point: u.point(),
                        state: Initialization::SourceUnavailable,
                    },
                )?;
            }
            continue;
        };
        let value_facts = &f.values[value.0 as usize];
        if !value_facts.families.contains(Family::Reference) {
            continue;
        }
        match value_facts.identity {
            Identity::Root(root) if tracked(ssa, root) => {
                push(&mut use_indices[root.0 as usize], i)?
            }
            Identity::Root(_) => push(
                &mut f.initialization,
                InitializationUse {
                    site: u.site(),
                    point: u.point(),
                    state: Initialization::Initialized,
                },
            )?,
            _ if value_facts.literal().is_some_and(|b| b.value == 0) => push(
                &mut f.initialization,
                InitializationUse {
                    site: u.site(),
                    point: u.point(),
                    state: Initialization::Initialized,
                },
            )?,
            _ => {
                push(
                    &mut f.initialization,
                    InitializationUse {
                        site: u.site(),
                        point: u.point(),
                        state: Initialization::Unknown,
                    },
                )?;
                issue(f, IssueSite::Use(u.site()), Problem::AmbiguousIdentity)?;
            }
        }
    }
    if use_indices.iter().all(Vec::is_empty) {
        return Ok(());
    }
    let mut incoming = lists(s.points().len())?;
    let mut outgoing = lists(s.points().len())?;
    for t in s.transfers() {
        if let Target::Point(target) = t.target() {
            push(&mut incoming[target.0 as usize], t.id())?;
            push(&mut outgoing[t.from().0 as usize], t.id())?;
        }
    }
    let mut events = filled(s.points().len(), None)?;
    for (i, c) in f.constructors.iter().enumerate() {
        let site = s
            .operation_site(c.operation)
            .ok_or(TypeError::InternalInvariant(
                "constructor has no execution site",
            ))?;
        events[site.execute().0 as usize] = Some(InitializationEvent::Constructor(i));
    }
    for (operation, identity) in unresolved_direct {
        let site = s
            .operation_site(operation)
            .ok_or(TypeError::InternalInvariant(
                "unresolved direct call has no execution site",
            ))?;
        events[site.execute().0 as usize] = Some(InitializationEvent::UnresolvedDirect(identity));
    }
    let mut demand = filled(s.points().len(), false)?;
    let mut state = filled(s.points().len(), 0u8)?;
    let mut queued = filled(s.points().len(), false)?;
    let mut queue = Vec::new();
    for (index, uses) in use_indices.iter().enumerate() {
        if uses.is_empty() {
            continue;
        }
        let root = ValueId(id(index)?);
        let definition = s.values()[index].point();
        demand.fill(false);
        state.fill(0);
        queued.fill(false);
        queue.clear();
        for &i in uses {
            let p = s.uses()[i].point().0 as usize;
            if !demand[p] {
                demand[p] = true;
                push(&mut queue, p)?;
            }
        }
        while let Some(p) = queue.pop() {
            if p == definition.0 as usize {
                continue;
            }
            for &t in &incoming[p] {
                let pred = s.transfers()[t.0 as usize].from().0 as usize;
                if !demand[pred] {
                    demand[pred] = true;
                    push(&mut queue, pred)?;
                }
            }
        }
        state[definition.0 as usize] = 1;
        push(&mut queue, definition.0 as usize)?;
        queued[definition.0 as usize] = true;
        while let Some(p) = queue.pop() {
            queued[p] = false;
            for &id in &outgoing[p] {
                let transfer = &s.transfers()[id.0 as usize];
                let Target::Point(next) = transfer.target() else {
                    continue;
                };
                let next = next.0 as usize;
                if !demand[next] || next == definition.0 as usize {
                    continue;
                }
                let mut bits = state[p];
                if let Some(event) = events[p] {
                    match event {
                        InitializationEvent::Constructor(index) => {
                            let c = &f.constructors[index];
                            if c.identity == Identity::Root(root) {
                                bits = after_constructor(
                                    bits,
                                    matches!(
                                        c.relation,
                                        ConstructorRelation::Allocation
                                            | ConstructorRelation::This
                                            | ConstructorRelation::Super
                                    ),
                                    exceptional(ssa, transfer.origin()),
                                );
                            } else if bits != 2
                                && !matches!(c.identity,Identity::Root(other) if disjoint(ssa,root,other))
                            {
                                bits |= 8;
                            }
                        }
                        InitializationEvent::UnresolvedDirect(identity) => {
                            // Neither completion is a proven no-initialization path.
                            // Scope uncertainty using only existing receiver provenance.
                            if bits != 2
                                && !matches!(identity,Identity::Root(other) if disjoint(ssa,root,other))
                            {
                                bits |= 8;
                            }
                        }
                    }
                }
                let new = state[next] | bits;
                if new != state[next] {
                    state[next] = new;
                    if !queued[next] {
                        queued[next] = true;
                        push(&mut queue, next)?;
                    }
                }
            }
        }
        for &i in uses {
            let u = &s.uses()[i];
            let bits = state[u.point().0 as usize];
            push(
                &mut f.initialization,
                InitializationUse {
                    site: u.site(),
                    point: u.point(),
                    state: classify(bits),
                },
            )?;
            if bits & 16 != 0 {
                issue(
                    f,
                    IssueSite::Use(u.site()),
                    Problem::InitializationStateConflict,
                )?;
            }
        }
    }
    // Records retain UseSite identities; no register environment or global
    // initialized flag is reconstructed.
    Ok(())
}
