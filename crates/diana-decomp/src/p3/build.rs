#[cfg(test)]
use super::check;
use super::scratch::{bug, filled, id, lists, push};
use super::{dominance, graph, rename, *};
use crate::p2::{ExecutionFacts, FlowMethod, OpKind};
use crate::{FailureKind, RegisterSlot, SlotWidth};
use std::collections::TryReserveError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SsaError {
    #[error("SSA allocation failed: {0}")]
    Allocation(#[source] TryReserveError),
    #[error("SSA capacity is not representable on this platform")]
    PlatformCapacity,
    #[cfg(test)]
    #[error("SSA correspondence failed at {point:?}: {obligation}")]
    Correspondence {
        point: PointId,
        obligation: &'static str,
    },
    #[error("BUG: {0}")]
    InternalInvariant(&'static str),
}
impl SsaError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Allocation(_) | Self::PlatformCapacity => FailureKind::ResourceExhausted,
            _ => FailureKind::InternalInvariant,
        }
    }
}
pub(crate) enum SsaBuild<'a> {
    Ready(SsaMethod<'a>),
    Stopped(SsaStop<'a>),
}
pub(crate) fn build_ssa<'a>(
    _task: &crate::engine::MethodTask,
    flow: FlowMethod<'a>,
) -> SsaBuild<'a> {
    let mut facts = SsaFacts::empty();
    let result = construct(flow.facts(), &mut facts);
    match result {
        Ok(()) => SsaBuild::Ready(SsaMethod { flow, facts }),
        Err(error) => SsaBuild::Stopped(SsaStop {
            flow,
            error,
            partial: SsaPartial { facts },
        }),
    }
}

#[derive(Clone, Copy)]
pub(super) struct UseSeed {
    pub site: UseSite,
    pub point: PointId,
    pub slot: SlotId,
}
pub(super) struct Work {
    pub defs: Vec<Vec<StateId>>,
    pub phis: Vec<Vec<PhiId>>,
    pub uses: Vec<Vec<usize>>,
    pub slot_defs: Vec<Vec<PointId>>,
    pub slot_uses: Vec<Vec<PointId>>,
    pub seeds: Vec<UseSeed>,
}
impl Work {
    pub fn new(f: &SsaFacts) -> Result<Self, SsaError> {
        let mut work = Self {
            defs: lists(f.points.len())?,
            phis: lists(f.points.len())?,
            uses: lists(f.points.len())?,
            slot_defs: lists(f.slots.len())?,
            slot_uses: lists(f.slots.len())?,
            seeds: Vec::new(),
        };
        for state in &f.states {
            if !matches!(state.kind, StateKind::Phi(_)) {
                push(&mut work.defs[state.point.0 as usize], state.id)?;
                push(&mut work.slot_defs[state.slot.0 as usize], state.point)?;
            }
        }
        for phi in &f.phis {
            push(&mut work.phis[phi.point.0 as usize], phi.id)?;
        }
        Ok(work)
    }
    fn usage(&mut self, seed: UseSeed) -> Result<(), SsaError> {
        push(&mut self.uses[seed.point.0 as usize], self.seeds.len())?;
        push(&mut self.slot_uses[seed.slot.0 as usize], seed.point)?;
        push(&mut self.seeds, seed)
    }
}
pub(super) fn slot(f: &SsaFacts, s: RegisterSlot) -> Result<SlotId, SsaError> {
    f.slots
        .binary_search_by_key(&(s.start.0, s.words()), |r| (r.start.0, r.words()))
        .map(|i| SlotId(i as u32))
        .map_err(|_| bug("P2 slot missing from fixed SSA slot set"))
}
pub(super) fn state(
    f: &mut SsaFacts,
    slot: SlotId,
    point: PointId,
    kind: StateKind,
) -> Result<StateId, SsaError> {
    let state = StateId(id(f.states.len())?);
    let availability = Availability(match kind {
        StateKind::Value(_) => 1,
        StateKind::Undefined => 2,
        StateKind::Invalidated { .. } => 4,
        StateKind::OutsideFrame | StateKind::Unavailable { .. } => 8,
        StateKind::Phi(_) => 0,
    });
    push(
        &mut f.states,
        StateDefinition {
            id: state,
            slot,
            point,
            kind,
            availability,
        },
    )?;
    Ok(state)
}
pub(super) fn value(
    f: &mut SsaFacts,
    slot: SlotId,
    point: PointId,
    origin: ValueOrigin,
) -> Result<ValueId, SsaError> {
    let value = ValueId(id(f.values.len())?);
    push(
        &mut f.values,
        ValueDefinition {
            id: value,
            slot,
            point,
            origin,
        },
    )?;
    Ok(value)
}
pub(super) fn overlaps(a: RegisterSlot, b: RegisterSlot) -> bool {
    u64::from(a.start.0) < u64::from(b.start.0) + u64::from(b.words())
        && u64::from(b.start.0) < u64::from(a.start.0) + u64::from(a.words())
}
fn construct(flow: ExecutionFacts<'_, '_>, f: &mut SsaFacts) -> Result<(), SsaError> {
    for &s in flow.slots() {
        push(&mut f.slots, s)?;
    }
    if let Some(s) = flow.entry().this() {
        push(&mut f.slots, s)?;
    }
    for p in flow.entry().parameters() {
        push(&mut f.slots, p.slot())?;
    }
    f.slots.sort_unstable_by_key(|s| (s.start.0, s.words()));
    f.slots.dedup();
    id(f.slots.len())?;
    let graph = graph::construct(flow, f)?;
    let mut entry = filled(f.slots.len(), None)?;
    if let Some(s) = flow.entry().this() {
        entry[slot(f, s)?.0 as usize] = Some(EntryRole::This);
    }
    for (i, p) in flow.entry().parameters().iter().enumerate() {
        entry[slot(f, p.slot())?.0 as usize] = Some(EntryRole::Parameter(id(i)?));
    }
    for (i, role) in entry.into_iter().enumerate() {
        let slot = SlotId(id(i)?);
        let kind = match role {
            Some(role) => StateKind::Value(value(f, slot, PointId(0), ValueOrigin::Entry(role))?),
            None if !flow.slot_is_valid(f.slots[i]) => StateKind::OutsideFrame,
            None => StateKind::Undefined,
        };
        state(f, slot, PointId(0), kind)?;
    }
    let mut affected = Vec::new();
    for op in flow.operations() {
        f.operations[op.id().0 as usize].output_start = id(f.outputs.len())?;
        f.operations[op.id().0 as usize].output_len = id(flow.writes(op).len())?;
        let Some(point) = f.operations[op.id().0 as usize].commit else {
            continue;
        };
        let start = f.outputs.len();
        affected.clear();
        for (i, &written) in flow.writes(op).iter().enumerate() {
            for prior in &f.outputs[start..] {
                if overlaps(f.slots[prior.slot.0 as usize], written) {
                    return Err(bug("normal outputs overlap without an unambiguous commit"));
                }
            }
            let key = slot(f, written)?;
            let origin = if matches!(op.kind(), OpKind::MoveException) {
                ValueOrigin::Exception(op.id())
            } else {
                ValueOrigin::Operation {
                    operation: op.id(),
                    output: id(i)?,
                }
            };
            let val = if op.kind().output_is_established() && flow.slot_is_valid(written) {
                Some(value(f, key, point, origin)?)
            } else {
                None
            };
            let kind = match val {
                Some(v) => StateKind::Value(v),
                None => StateKind::Unavailable { operation: op.id() },
            };
            let def = state(f, key, point, kind)?;
            push(
                &mut f.outputs,
                OutputBinding {
                    operation: op.id(),
                    output: id(i)?,
                    point,
                    slot: key,
                    state: def,
                },
            )?;
            // Width is One/Two: only starts r-1, r, r+1 can overlap.
            for begin in [
                written.start.0.checked_sub(1),
                Some(written.start.0),
                written.start.0.checked_add(1),
            ]
            .into_iter()
            .flatten()
            {
                for width in [SlotWidth::One, SlotWidth::Two] {
                    let candidate = RegisterSlot::new(crate::RegisterId(begin), width);
                    if overlaps(candidate, written)
                        && let Ok(key) = slot(f, candidate)
                    {
                        push(&mut affected, key)?;
                    }
                }
            }
        }
        affected.sort_unstable();
        affected.dedup();
        for &key in &affected {
            if !f.outputs[start..].iter().any(|o| o.slot == key) {
                let kind = if f.outputs[start..]
                    .iter()
                    .any(|o| f.output_value(o).is_none())
                {
                    StateKind::Unavailable { operation: op.id() }
                } else {
                    StateKind::Invalidated { operation: op.id() }
                };
                state(f, key, point, kind)?;
            }
        }
    }
    let mut work = Work::new(f)?;
    for op in flow.operations() {
        f.operations[op.id().0 as usize].use_start = id(work.seeds.len())?;
        let Ok(reads) = flow.reads(op) else {
            continue;
        };
        f.operations[op.id().0 as usize].use_len = id(reads.len())?;
        let point = f.operations[op.id().0 as usize].execute;
        for (i, &s) in reads.iter().enumerate() {
            work.usage(UseSeed {
                site: UseSite::Operation {
                    operation: op.id(),
                    argument: id(i)?,
                },
                point,
                slot: slot(f, s)?,
            })?;
        }
    }
    for block in flow.blocks() {
        if let Some(point) = graph.terminals[block.id().0 as usize] {
            f.terminals[block.id().0 as usize] = Some(TerminalSite {
                block: block.id(),
                point,
                use_start: id(work.seeds.len())?,
                use_len: id(flow.terminal_reads(block).len())?,
            });
            for (i, &s) in flow.terminal_reads(block).iter().enumerate() {
                work.usage(UseSeed {
                    site: UseSite::Terminal {
                        block: block.id(),
                        argument: id(i)?,
                    },
                    point,
                    slot: slot(f, s)?,
                })?;
            }
        }
    }
    let dom = dominance::construct(f, &graph)?;
    #[cfg(test)]
    dom.verify(f, &graph)?;
    rename::place(f, &graph, &dom, &mut work)?;
    rename::bind(f, &graph, &dom, &work)?;
    rename::compact(f)?;
    rename::availability(f)?;
    #[cfg(test)]
    check::verify(flow, f, &graph)?;
    Ok(())
}
