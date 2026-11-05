use super::build::*;
use super::*;
use crate::RequestError;
use crate::p2::{EdgeRole, OpKind};
use crate::p3::{PointKind, Target, TransferOrigin};
use crate::p4::TypedMethod;
use std::collections::HashMap;

struct Plans {
    locations: HashMap<CopyLocation, PlanId>,
    destinations: HashMap<(PlanId, LocalId), CopyId>,
}
impl Plans {
    fn new() -> Self {
        Self {
            locations: HashMap::new(),
            destinations: HashMap::new(),
        }
    }
    fn at(&mut self, f: &mut StorageFacts, location: CopyLocation) -> Result<PlanId, StorageError> {
        if let Some(&plan) = self.locations.get(&location) {
            return Ok(plan);
        }
        self.locations
            .try_reserve(1)
            .map_err(RequestError::Allocation)?;
        let id = PlanId(id(f.copies.len())?);
        push(
            &mut f.copies,
            CopyPlan {
                id,
                location,
                assignments: Vec::new(),
                steps: Vec::new(),
            },
        )?;
        self.locations.insert(location, id);
        Ok(id)
    }
    fn assignment(
        &mut self,
        f: &mut StorageFacts,
        plan: PlanId,
        origin: CopyOrigin,
        destination: Result<LocalId, StorageGap>,
        source: Result<StorageRead, StorageGap>,
        dependency: Option<LocalId>,
    ) -> Result<CopyId, StorageError> {
        if let Ok(local) = destination {
            if let Some(&copy) = self.destinations.get(&(plan, local)) {
                let old = &f.copies[plan.0 as usize].assignments[copy.0 as usize];
                if !same_read(old.source, source) {
                    return Err(StorageError::InternalInvariant(
                        "one transfer requires conflicting writes to one storage",
                    ));
                }
                return Ok(copy);
            }
            self.destinations
                .try_reserve(1)
                .map_err(RequestError::Allocation)?;
        }
        let entries = &mut f.copies[plan.0 as usize].assignments;
        let copy = CopyId(id(entries.len())?);
        let issue = destination.err().or(source.err());
        push(
            entries,
            CopyAssignment {
                id: copy,
                origin,
                destination,
                source,
                dependency,
                issue,
            },
        )?;
        if let Ok(local) = destination {
            self.destinations.insert((plan, local), copy);
        }
        Ok(copy)
    }
}
fn same_read(a: Result<StorageRead, StorageGap>, b: Result<StorageRead, StorageGap>) -> bool {
    match (a, b) {
        (
            Ok(StorageRead::Value {
                local: a, view: av, ..
            }),
            Ok(StorageRead::Value {
                local: b, view: bv, ..
            }),
        ) => a == b && av == bv,
        (
            Ok(StorageRead::Literal {
                bits: a, view: av, ..
            }),
            Ok(StorageRead::Literal {
                bits: b, view: bv, ..
            }),
        ) => a == b && av == bv,
        _ => a == b,
    }
}
fn target(
    f: &StorageFacts,
    value: crate::p3::ValueId,
) -> Result<Result<LocalId, StorageGap>, StorageError> {
    match f.bindings[value.0 as usize] {
        StorageBinding::Unavailable(gap) => Ok(Err(gap)),
        binding => binding
            .local()
            .map(Ok)
            .ok_or(StorageError::InternalInvariant(
                "runtime Phi has no storage target",
            )),
    }
}
fn location(
    typed: &TypedMethod<'_>,
    transfer: crate::p3::TransferId,
) -> Result<CopyLocation, StorageError> {
    let s = typed.ssa_facts();
    let t = &s.transfers()[transfer.0 as usize];
    let site = match t.origin() {
        TransferOrigin::Entry => TransferSite::InitialEntry,
        TransferOrigin::NormalCommit(op) => TransferSite::AfterCommit(op),
        TransferOrigin::Edge(edge) => TransferSite::NormalEdge(edge),
        TransferOrigin::Continue => match t.target() {
            Target::Point(point) => match s.points()[point.0 as usize].kind() {
                PointKind::Execute(op) => TransferSite::BeforeOperation(op),
                PointKind::Terminal(block) => TransferSite::BeforeTerminal(block),
                _ => {
                    return Err(StorageError::InternalInvariant(
                        "continuation copy lost its original execution position",
                    ));
                }
            },
            Target::Exit(_) => {
                return Err(StorageError::InternalInvariant(
                    "continuation copy targets an exit",
                ));
            }
        },
        TransferOrigin::Return(_) => {
            return Err(StorageError::InternalInvariant(
                "Phi incoming is a return exit",
            ));
        }
    };
    Ok(CopyLocation::Transfer { transfer, site })
}
pub(super) fn construct(typed: &TypedMethod<'_>, f: &mut StorageFacts) -> Result<(), StorageError> {
    let s = typed.ssa_facts();
    let flow = typed.execution();
    let mut plans = Plans::new();
    let mut shadows = filled(s.phis().len(), None)?;
    for phi_view in typed.facts().phis() {
        let phi =
            s.phis()
                .get(phi_view.phi().0 as usize)
                .ok_or(StorageError::InternalInvariant(
                    "Phi view lost its SSA definition",
                ))?;
        let literal = matches!(
            f.bindings[phi.value().0 as usize],
            StorageBinding::Literal(_)
        );
        for incoming in phi_view.incoming() {
            let origin = CopyOrigin::Incoming {
                phi: phi.id(),
                transfer: incoming.transfer,
            };
            let source = super::bindings::read(s, f, incoming.state, incoming.interpretation);
            let dependency = super::bindings::dependency(s, f, incoming.state);
            let t = &s.transfers()[incoming.transfer.0 as usize];
            let exceptional = matches!(t.origin(),TransferOrigin::Edge(edge) if matches!(flow.edges()[edge.0 as usize].role(),EdgeRole::Exception { .. }));
            let route = if literal {
                match source {
                    Ok(_) => TransferRoute::Literal,
                    Err(gap) => TransferRoute::Unavailable(gap),
                }
            } else if exceptional {
                match target(f, phi.value())? {
                    Err(gap) => TransferRoute::Unavailable(gap),
                    Ok(destination) => {
                        let (snapshot, receive_plan, receive) =
                            if let Some(existing) = shadows[phi.id().0 as usize] {
                                existing
                            } else {
                                let block = match s.points()[phi.point().0 as usize].kind() {
                                    PointKind::BlockEntry(block) => block,
                                    _ => {
                                        return Err(StorageError::InternalInvariant(
                                            "exception Phi lost its original handler entry",
                                        ));
                                    }
                                };
                                let capture = flow
                                    .block(block)
                                    .ok_or(StorageError::InternalInvariant(
                                        "handler entry lost its block",
                                    ))?
                                    .operations()
                                    .next()
                                    .map(|i| &flow.operations()[i as usize])
                                    .filter(|op| matches!(op.kind(), OpKind::MoveException))
                                    .map(|op| op.id());
                                let storage_type = f.locals[destination.0 as usize].storage_type;
                                let local = local(
                                    f,
                                    storage_type,
                                    LocalKind::Snapshot {
                                        phi: phi.id(),
                                        entry: phi.point(),
                                    },
                                )?;
                                let snapshot = SnapshotId(id(f.snapshots.len())?);
                                push(
                                    &mut f.snapshots,
                                    Snapshot {
                                        id: snapshot,
                                        phi: phi.id(),
                                        entry: phi.point(),
                                        local,
                                    },
                                )?;
                                let plan = plans.at(
                                    f,
                                    CopyLocation::HandlerEntry {
                                        point: phi.point(),
                                        block,
                                        capture,
                                    },
                                )?;
                                let receive = plans.assignment(
                                    f,
                                    plan,
                                    CopyOrigin::HandlerReceive(phi.id()),
                                    Ok(destination),
                                    Ok(StorageRead::Snapshot {
                                        snapshot,
                                        local,
                                        phi: phi.id(),
                                    }),
                                    Some(local),
                                )?;
                                let result = (snapshot, plan, receive);
                                shadows[phi.id().0 as usize] = Some(result);
                                result
                            };
                        let site = match s.points()[t.from().0 as usize].kind() {
                            PointKind::Execute(op) => ThrowSite::Operation(op),
                            PointKind::Terminal(block) => ThrowSite::Terminal(block),
                            _ => {
                                return Err(StorageError::InternalInvariant(
                                    "exception snapshot lost its original throw position",
                                ));
                            }
                        };
                        let plan = plans.at(f, CopyLocation::BeforeThrow(site))?;
                        let shadow = f.snapshots[snapshot.0 as usize].local;
                        let save =
                            plans.assignment(f, plan, origin, Ok(shadow), source, dependency)?;
                        TransferRoute::Exception {
                            snapshot,
                            save_plan: plan,
                            save,
                            receive_plan,
                            receive,
                        }
                    }
                }
            } else {
                let destination = target(f, phi.value())?;
                let at = location(typed, incoming.transfer)?;
                let plan = plans.at(f, at)?;
                let copy = plans.assignment(f, plan, origin, destination, source, dependency)?;
                TransferRoute::Normal { plan, copy }
            };
            if let TransferRoute::Unavailable(gap) = route {
                issue(
                    f,
                    StorageIssueSite::Incoming {
                        phi: phi.id(),
                        transfer: incoming.transfer,
                    },
                    gap,
                )?;
            }
            push(
                &mut f.transfers,
                PhiTransfer {
                    phi: phi.id(),
                    transfer: incoming.transfer,
                    source: incoming.state,
                    view: incoming.interpretation,
                    route,
                },
            )?;
        }
    }
    Ok(())
}
