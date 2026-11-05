use super::build::*;
use super::*;
use crate::p2::{EdgeRole, EdgeTarget, MoveKind, NormalEdge, OpKind};
use crate::p3::{PointId, Target, TransferId, TransferOrigin, UseSite};
use crate::p4::{AllocationRelation, ConstructorRelation, Initialization, TypedMethod};

fn source_ready(typed: &TypedMethod<'_>, read: StorageRead, at: PointId) -> bool {
    match read {
        StorageRead::Literal { .. } => true,
        StorageRead::Value { value, .. } => typed
            .ssa_facts()
            .dominates(typed.ssa_facts().values()[value.0 as usize].point(), at),
        StorageRead::Snapshot { .. } => false,
    }
}
fn same_read(a: StorageRead, b: StorageRead) -> bool {
    match (a, b) {
        (
            StorageRead::Value {
                local: a,
                value: av,
                view: ai,
                ..
            },
            StorageRead::Value {
                local: b,
                value: bv,
                view: bi,
                ..
            },
        ) => a == b && av == bv && ai == bi,
        (
            StorageRead::Literal {
                bits: a, view: ai, ..
            },
            StorageRead::Literal {
                bits: b, view: bi, ..
            },
        ) => a == b && ai == bi,
        _ => a == b,
    }
}
fn normal_success(
    typed: &TypedMethod<'_>,
    op: crate::p2::OpId,
) -> Result<(TransferId, PointId), StorageError> {
    let s = typed.ssa_facts();
    let from = s
        .operation_site(op)
        .ok_or(StorageError::InternalInvariant(
            "construction lost its execution point",
        ))?
        .execute();
    s.transfers().iter().find_map(|t| {
        if t.from()!=from { return None; }
        if matches!(t.origin(),TransferOrigin::Edge(e) if matches!(typed.execution().edges()[e.0 as usize].role(),EdgeRole::Exception{..})) { return None; }
        if let Target::Point(p)=t.target() {Some((t.id(),p))} else {None}
    }).ok_or(StorageError::InternalInvariant("constructor has no normal continuation"))
}
#[derive(Eq, PartialEq)]
struct Handler {
    order: u32,
    catch: Option<crate::TypeId>,
    target: Option<crate::p2::BlockId>,
}
struct Corridor {
    members: Vec<crate::p2::OpId>,
    edges: Vec<crate::p2::EdgeId>,
}
fn protection(typed: &TypedMethod<'_>, op: crate::p2::OpId) -> Result<Vec<Handler>, StorageError> {
    let mut result = Vec::new();
    for e in typed
        .execution()
        .edges()
        .iter()
        .filter(|e| e.source_op() == Some(op))
    {
        if let EdgeRole::Exception { order, catch, .. } = e.role() {
            push(
                &mut result,
                Handler {
                    order,
                    catch,
                    target: match e.target() {
                        EdgeTarget::Block(b) => Some(b),
                        EdgeTarget::Exit(_) => None,
                    },
                },
            )?;
        }
    }
    Ok(result)
}
fn corridor(
    typed: &TypedMethod<'_>,
    allocation: crate::p2::OpId,
    init: crate::p2::OpId,
) -> Result<Result<Corridor, Separation>, StorageError> {
    let flow = typed.execution();
    let mut members = Vec::new();
    let mut edges = Vec::new();
    let mut block = flow.operations()[allocation.0 as usize].block();
    let mut next = allocation.0;
    let mut visited = filled(flow.blocks().len(), false)?;
    loop {
        if visited[block.0 as usize] {
            return Ok(Err(Separation::ControlBoundary(block)));
        }
        visited[block.0 as usize] = true;
        for index in flow
            .block(block)
            .ok_or(StorageError::InternalInvariant(
                "construction corridor lost its block",
            ))?
            .operations()
        {
            if index < next {
                continue;
            }
            let op = &flow.operations()[index as usize];
            push(&mut members, op.id())?;
            if op.id() == init {
                return Ok(Ok(Corridor { members, edges }));
            }
        }
        let mut outgoing = flow
            .edges()
            .iter()
            .filter(|e| e.from() == block && matches!(e.role(), EdgeRole::Normal(_)));
        let Some(edge) = outgoing.next() else {
            return Ok(Err(Separation::ControlBoundary(block)));
        };
        if outgoing.next().is_some()
            || !matches!(
                edge.role(),
                EdgeRole::Normal(NormalEdge::Goto | NormalEdge::Fallthrough)
            )
        {
            return Ok(Err(Separation::ControlBoundary(block)));
        }
        let EdgeTarget::Block(target) = edge.target() else {
            return Ok(Err(Separation::ControlBoundary(block)));
        };
        if flow
            .edges()
            .iter()
            .filter(|e| e.target() == EdgeTarget::Block(target))
            .count()
            != 1
        {
            return Ok(Err(Separation::ControlBoundary(target)));
        }
        push(&mut edges, edge.id())?;
        block = target;
        next = 0;
    }
}
// Removing the constructor's normal transfer asks whether an original use can
// still observe the early committed carrier. It is not a second SSA solver.
fn without_success(
    typed: &TypedMethod<'_>,
    omitted: TransferId,
) -> Result<Vec<bool>, StorageError> {
    let s = typed.ssa_facts();
    let mut seen = filled(s.points().len(), false)?;
    let mut queue = Vec::new();
    let entry = s
        .points()
        .iter()
        .find(|p| p.kind() == crate::p3::PointKind::Entry)
        .ok_or(StorageError::InternalInvariant(
            "construction has no method entry",
        ))?
        .id();
    push(&mut queue, entry)?;
    seen[entry.0 as usize] = true;
    let mut outgoing = Vec::new();
    for _ in s.points() {
        push(&mut outgoing, Vec::new())?;
    }
    for t in s.transfers() {
        if t.id() != omitted
            && let Target::Point(p) = t.target()
        {
            push(&mut outgoing[t.from().0 as usize], p)?;
        }
    }
    while let Some(p) = queue.pop() {
        for &q in &outgoing[p.0 as usize] {
            if !seen[q.0 as usize] {
                seen[q.0 as usize] = true;
                push(&mut queue, q)?;
            }
        }
    }
    Ok(seen)
}
fn candidate(
    typed: &TypedMethod<'_>,
    f: &StorageFacts,
    index: usize,
    allocation: crate::p2::OpId,
) -> Result<Result<ConstructionGroup, Separation>, StorageError> {
    let s = typed.ssa_facts();
    let flow = typed.execution();
    let constructor = f.constructions[index].fact;
    let init = constructor.operation;
    let receiver = typed
        .facts()
        .initialization()
        .iter()
        .find(|u| matches!(u.site,UseSite::Operation{operation,argument:0} if operation==init));
    let Some(receiver) = receiver else {
        return Ok(Err(Separation::NoAllocationBinding));
    };
    if receiver.state != Initialization::BeforeConstructor {
        return Ok(Err(Separation::Initialization(receiver.state)));
    }
    if constructor.relation != ConstructorRelation::Allocation {
        return Ok(Err(Separation::NonAllocationConstructor));
    }
    if constructor
        .allocation
        .is_none_or(|a| a.operation != allocation || a.relation != AllocationRelation::Same)
    {
        return Ok(Err(Separation::InstanceNotEstablished));
    }
    let Corridor {
        members,
        edges: interior_edges,
    } = match corridor(typed, allocation, init)? {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    for &operation in &members {
        if f.operation_emission[operation.0 as usize] != OperationEmission::Ordinary {
            return Ok(Err(Separation::SharedAllocation));
        }
        if operation == allocation || operation == init {
            continue;
        }
        let op = &flow.operations()[operation.0 as usize];
        let absorb = match op.kind() {
            OpKind::Move {
                kind: MoveKind::Object,
            } => typed
                .facts()
                .initialization()
                .iter()
                .find(
                    |u| matches!(u.site,UseSite::Operation{operation: o,argument:0}if o==operation),
                )
                .is_some_and(|u| {
                    u.allocation.is_some_and(|a| {
                        a.operation == allocation && a.relation == AllocationRelation::Same
                    }) && u.state == Initialization::BeforeConstructor
                }),
            OpKind::Const { .. } => f.operation_definitions(operation).is_some_and(|defs| {
                defs.iter()
                    .all(|d| matches!(d.materialization(), Materialization::Literal(_)))
            }),
            _ => false,
        };
        if !absorb {
            return Ok(Err(Separation::InterveningOperation(operation)));
        }
    }
    for plan in &f.copies {
        if matches!(plan.location,CopyLocation::Transfer{site:TransferSite::NormalEdge(e),..}if interior_edges.contains(&e))
            && !plan.assignments.is_empty()
        {
            return Ok(Err(Separation::TransferCopy(plan.id)));
        }
    }
    if protection(typed, allocation)? != protection(typed, init)? {
        return Ok(Err(Separation::DifferentProtection {
            allocation,
            initialization: init,
        }));
    }
    let at = s
        .operation_site(allocation)
        .ok_or(StorageError::InternalInvariant(
            "allocation lost its execute point",
        ))?
        .execute();
    let (success, normal) = normal_success(typed, init)?;
    let normal_single = s
        .transfers()
        .iter()
        .filter(|t| t.target() == Target::Point(normal))
        .count()
        == 1;
    let mut unsafe_reach = None;
    for usage in s.uses() {
        let Some(value) = value(s, usage.state()) else {
            continue;
        };
        if !typed
            .facts()
            .value(value)
            .is_some_and(|v| v.allocations().contains(&allocation))
        {
            continue;
        }
        let own =
            matches!(usage.site(),UseSite::Operation{operation,..}if members.contains(&operation));
        if !own && !(normal_single && s.dominates(normal, usage.point())) {
            if unsafe_reach.is_none() {
                unsafe_reach = Some(without_success(typed, success)?);
            }
            if !unsafe_reach
                .as_ref()
                .is_some_and(|r| r[usage.point().0 as usize])
            {
                continue;
            }
            let origin = typed
                .facts()
                .initialization()
                .iter()
                .find(|u| u.site == usage.site())
                .and_then(|u| u.allocation);
            if !origin.is_some_and(|a| {
                a.operation == allocation && a.relation == AllocationRelation::Different
            }) {
                return Ok(Err(Separation::ObservedBeforeSuccess(usage.site())));
            }
        }
    }
    let mut arguments = Vec::new();
    let inputs = match f.operation_uses(init) {
        Some(Ok(v)) => v,
        Some(Err(g)) => return Ok(Err(Separation::MissingStorage(g))),
        None => {
            return Err(StorageError::InternalInvariant(
                "constructor lost storage inputs",
            ));
        }
    };
    for input in inputs.iter().skip(1) {
        let read = match input.read() {
            Ok(r) => r,
            Err(g) => return Ok(Err(Separation::MissingStorage(g))),
        };
        if !source_ready(typed, read, at) {
            return Ok(Err(Separation::ArgumentNotAvailable(input.site())));
        }
        push(&mut arguments, read)?;
    }
    let mut snapshots = Vec::new();
    for plan in &f.copies {
        if matches!(plan.location,CopyLocation::BeforeThrow(ThrowSite::Operation(op))if op==allocation||op==init)
        {
            if !plan.is_complete() {
                return Ok(Err(Separation::SnapshotMismatch(plan.id)));
            }
            push(&mut snapshots, plan.id)?;
        }
    }
    let prelude = snapshots.first().copied();
    if let Some(first) = prelude {
        let base = &f.copies[first.0 as usize];
        for a in &base.assignments {
            if !a.source.is_ok_and(|r| source_ready(typed, r, at)) {
                return Ok(Err(Separation::SnapshotMismatch(first)));
            }
        }
        for &p in snapshots.iter().skip(1) {
            let other = &f.copies[p.0 as usize];
            if base.assignments.len() != other.assignments.len()
                || !base.assignments.iter().all(|a| {
                    other.assignments.iter().any(|b| {
                        a.destination == b.destination
                            && matches!((a.source,b.source),(Ok(a),Ok(b)) if same_read(a,b))
                    })
                })
            {
                return Ok(Err(Separation::SnapshotMismatch(p)));
            }
        }
        // Every throwing stage must have the same selected save obligations.
        for op in [allocation, init] {
            if !snapshots.iter().any(|p|matches!(f.copies[p.0 as usize].location,CopyLocation::BeforeThrow(ThrowSite::Operation(o))if o==op)){return Ok(Err(Separation::SnapshotMismatch(first)));}
        }
    }
    let mut publications = Vec::new();
    for &op in &members {
        for output in s
            .operation_outputs(op)
            .ok_or(StorageError::InternalInvariant(
                "construction member lost outputs",
            ))?
        {
            if let Some(v) = s.output_value(output)
                && let Some(local) = f.binding(v).and_then(StorageBinding::local)
            {
                if let Some(gap) = f.local_gap(local) {
                    return Ok(Err(Separation::MissingStorage(gap)));
                }
                push(&mut publications, Publication { value: v, local })?;
            }
        }
    }
    Ok(Ok(ConstructionGroup {
        id: GroupId(id(f.groups.len())?),
        allocation,
        initialization: init,
        normal,
        members,
        interior_edges,
        arguments,
        publications,
        prelude,
        snapshot_members: snapshots,
    }))
}
pub(super) fn construct(typed: &TypedMethod<'_>, f: &mut StorageFacts) -> Result<(), StorageError> {
    f.operation_emission = filled(
        typed.execution().operations().len(),
        OperationEmission::Ordinary,
    )?;
    f.copy_emission = filled(f.copies.len(), CopyEmission::Original)?;
    for op in typed.execution().operations() {
        if matches!(op.kind(), OpKind::NewInstance { .. }) {
            push(
                &mut f.allocation_plans,
                AllocationPlan {
                    operation: op.id(),
                    mode: ConstructionMode::Explicit(Separation::NoEstablishedConstructor),
                },
            )?;
        }
    }
    for i in 0..f.constructions.len() {
        let Some(origin) = f.constructions[i].fact.allocation else {
            if matches!(
                f.constructions[i].fact.relation,
                ConstructorRelation::This
                    | ConstructorRelation::Super
                    | ConstructorRelation::Ancestor
                    | ConstructorRelation::OtherOwner
                    | ConstructorRelation::NotAllocation
            ) {
                f.constructions[i].mode =
                    ConstructionMode::Explicit(Separation::NonAllocationConstructor);
            }
            continue;
        };
        let allocation = origin.operation;
        let result = candidate(typed, f, i, allocation)?;
        let mode = match result {
            Err(reason) => ConstructionMode::Explicit(reason),
            Ok(group) => {
                let id = group.id;
                for &op in &group.members {
                    let role = if op == group.allocation {
                        ConstructionRole::Allocation
                    } else if op == group.initialization {
                        ConstructionRole::Initialization
                    } else if matches!(
                        typed.execution().operations()[op.0 as usize].kind(),
                        OpKind::Const { .. }
                    ) {
                        ConstructionRole::Literal
                    } else {
                        ConstructionRole::Alias
                    };
                    f.operation_emission[op.0 as usize] =
                        OperationEmission::Construction { group: id, role };
                }
                for &p in &group.snapshot_members {
                    f.copy_emission[p.0 as usize] = CopyEmission::Construction { group: id };
                }
                for publication in &group.publications {
                    f.bindings[publication.value.0 as usize] = StorageBinding::Deferred {
                        local: publication.local,
                        group: id,
                    };
                    for definition in &mut f.definitions {
                        if typed
                            .ssa_facts()
                            .state(definition.state)
                            .is_some_and(|state| {
                                state.kind() == crate::p3::StateKind::Value(publication.value)
                            })
                        {
                            definition.materialization = Materialization::Deferred {
                                group: id,
                                local: publication.local,
                            };
                        }
                    }
                }
                push(&mut f.groups, group)?;
                ConstructionMode::Natural(id)
            }
        };
        f.constructions[i].mode = mode;
        if let Some(plan) = f
            .allocation_plans
            .iter_mut()
            .find(|p| p.operation == allocation)
            && !matches!(plan.mode, ConstructionMode::Natural(_))
        {
            plan.mode = mode;
        }
    }
    Ok(())
}
