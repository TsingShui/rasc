use super::super::build::*;
use super::super::dataflow::Dataflow;
use super::super::references::{References, Source};
use super::super::*;
use super::{BEFORE, Call, CallKind, INITIALIZED, NULL, UNAVAILABLE, UNKNOWN};
use crate::TypeId;
use crate::p1::CheckedReferenceFact;
use crate::p2::{ArrayAccess, FieldAccess, OpKind, ValueKind};
use crate::p3::{EntryRole, StateId, ValueId, ValueOrigin};
use crate::raw::ReferenceSlot;

pub(super) const ALLOCATION: u32 = 1;
pub(super) const THIS: u32 = 2;
pub(super) const SUPER: u32 = 4;
pub(super) const OTHER_OWNER: u32 = 8;
pub(super) const NOT_ALLOCATION: u32 = 16;
pub(super) const MISSING_REFERENCE: u32 = 32;
pub(super) const MISSING_RECEIVER: u32 = 64;
pub(super) const MISSING_OWNER: u32 = 128;
pub(super) const OWNER_UNAVAILABLE: u32 = 256;
pub(super) const ANCESTOR: u32 = 512;
pub(super) const ELIGIBLE: u32 = ALLOCATION | THIS | SUPER | ANCESTOR;

impl References<'_, '_> {
    fn has_reference_declaration(&self, value: ValueId) -> bool {
        self.values[value.0 as usize].types.iter().any(|&n| {
            self.names[n.0 as usize]
                .units
                .first()
                .copied()
                .and_then(tag_family)
                == Some(Family::Reference)
        })
    }
    pub fn birth(&self, state: StateId) -> u32 {
        if !self.available(state) {
            return UNAVAILABLE;
        }
        if self.is_null(state) {
            return NULL;
        }
        let Some(value) = self.value(state) else {
            return UNAVAILABLE;
        };
        let flow = self.ssa.execution();
        match self.ssa.facts().values()[value.0 as usize].origin() {
            ValueOrigin::Entry(EntryRole::This)
                if flow.declaration().name.units() == super::INIT_NAME =>
            {
                BEFORE
            }
            ValueOrigin::Entry(EntryRole::This) => INITIALIZED,
            ValueOrigin::Entry(EntryRole::Parameter(_)) => {
                if self.has_reference_declaration(value) {
                    INITIALIZED | NULL
                } else {
                    UNKNOWN
                }
            }
            ValueOrigin::Exception(_) => INITIALIZED,
            ValueOrigin::Operation { operation, .. } => {
                match flow.operations()[operation.0 as usize].kind() {
                    OpKind::NewInstance { .. } => BEFORE,
                    OpKind::NewArray { .. }
                    | OpKind::FilledNewArray { .. }
                    | OpKind::ConstString { .. }
                    | OpKind::ConstClass { .. }
                    | OpKind::ConstMethodHandle { .. }
                    | OpKind::ConstMethodType { .. } => INITIALIZED,
                    OpKind::Field {
                        access: FieldAccess::Get,
                        kind: ValueKind::Object,
                        ..
                    }
                    | OpKind::Array {
                        access: ArrayAccess::Get,
                        kind: ValueKind::Object,
                    } => INITIALIZED | NULL,
                    OpKind::Invoke { .. } | OpKind::UnresolvedInvoke { .. } => {
                        if self.has_reference_declaration(value) {
                            INITIALIZED | NULL
                        } else {
                            UNKNOWN
                        }
                    }
                    _ => UNKNOWN,
                }
            }
            ValueOrigin::Phi(_) => UNKNOWN,
        }
    }
    fn owner_at_leaf(
        &self,
        state: StateId,
        owner: TypeId,
    ) -> Result<(u32, Option<crate::p0::SuperclassGap>), TypeError> {
        if !self.available(state) {
            return Ok((MISSING_RECEIVER, None));
        }
        let Some(value) = self.value(state) else {
            return Ok((MISSING_RECEIVER, None));
        };
        let flow = self.ssa.execution();
        let bits = match self.ssa.facts().values()[value.0 as usize].origin() {
            ValueOrigin::Entry(EntryRole::This)
                if flow.declaration().name.units() == super::INIT_NAME =>
            {
                use crate::p0::{SuperclassGap, SuperclassRelation};
                match flow.owner_relation(owner)? {
                    SuperclassRelation::Same => THIS,
                    SuperclassRelation::Ancestor { depth: 1 } => SUPER,
                    SuperclassRelation::Ancestor { .. } => ANCESTOR,
                    SuperclassRelation::Unrelated => OTHER_OWNER,
                    SuperclassRelation::Unknown(gap) => {
                        let unavailable = matches!(
                            gap,
                            SuperclassGap::Declaration { .. }
                                | SuperclassGap::CurrentDeclarationUnavailable
                                | SuperclassGap::Cycle
                        );
                        return Ok((
                            MISSING_OWNER | if unavailable { OWNER_UNAVAILABLE } else { 0 },
                            Some(gap),
                        ));
                    }
                }
            }
            ValueOrigin::Operation { operation, .. } => {
                let op = &flow.operations()[operation.0 as usize];
                if let OpKind::NewInstance { type_id } = op.kind() {
                    if flow
                        .reference_use_issue(op.pc(), ReferenceSlot::First)
                        .is_some()
                        || !matches!(
                            flow.reference_fact(op.pc(), ReferenceSlot::First),
                            Some(Ok(CheckedReferenceFact::Type { .. }))
                        )
                    {
                        MISSING_REFERENCE
                    } else if *type_id == owner {
                        ALLOCATION
                    } else {
                        OTHER_OWNER
                    }
                } else {
                    NOT_ALLOCATION
                }
            }
            _ => NOT_ALLOCATION,
        };
        Ok((bits, None))
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct OwnerQuery {
    state: StateId,
    call: usize,
}

pub(super) fn owners(refs: &References<'_, '_>, calls: &mut [Call]) -> Result<(), TypeError> {
    let mut graph = Dataflow::<OwnerQuery, ()>::new();
    let mut queries = Vec::new();
    for (call, c) in calls.iter().enumerate() {
        let query = if let (CallKind::Constructor { valid: true, .. }, Some(state)) =
            (c.kind, c.receiver)
        {
            Some(graph.intern(OwnerQuery { state, call })?)
        } else {
            None
        };
        push(&mut queries, query)?;
    }
    let mut index = 0;
    while index < graph.nodes.len() {
        let OwnerQuery { state, call } = graph.nodes[index].key;
        let CallKind::Constructor { owner, .. } = calls[call].kind else {
            return Err(TypeError::InternalInvariant(
                "owner query has no constructor declaration",
            ));
        };
        match refs.source(state)? {
            Source::Alias { input } => {
                let source = graph.intern(OwnerQuery { state: input, call })?;
                graph.link(index, source, ())?;
            }
            Source::Phi(phi) => {
                for input in refs.ssa.facts().phis()[phi.0 as usize].incoming() {
                    let source = graph.intern(OwnerQuery {
                        state: input.state(),
                        call,
                    })?;
                    graph.link(index, source, ())?;
                }
            }
            Source::Leaf => {
                let (bits, gap) = refs.owner_at_leaf(state, owner)?;
                graph.nodes[index].seed = bits;
                if let Some(gap) = gap {
                    calls[call].owner_gap = Some(gap);
                }
            }
        }
        index += 1;
    }
    graph.solve(|_| MISSING_RECEIVER, |(), bits| bits)?;
    for (call, query) in calls.iter_mut().zip(queries) {
        call.owners = match (call.kind, query) {
            (_, Some(query)) => graph.nodes[query].bits,
            (CallKind::Constructor { valid: false, .. } | CallKind::UnresolvedDirect, _) => {
                MISSING_REFERENCE
            }
            _ => MISSING_RECEIVER,
        };
        if call.receiver.is_none_or(|state| !refs.available(state)) {
            call.owners |= MISSING_RECEIVER;
        }
    }
    Ok(())
}
