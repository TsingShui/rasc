//! Original obligations and sparse reverse-path validation of published bindings.
//! This does not call the SSA renamer or recover bindings for a consumer.
use super::build::{Work, slot};
use super::graph::Graph;
use super::scratch::{bug, filled, id, lists, push};
use super::*;
use crate::p2::{EdgeRole, EdgeTarget, ExecutionFacts, ExitKind, OpKind};
use std::collections::HashMap;

fn arc_key(from: PointId, target: Target, origin: TransferOrigin) -> (u32, u8, u32, u8, u32) {
    let (tag, dest) = match target {
        Target::Point(p) => (0, p.0),
        Target::Exit(e) => (1, e.0),
    };
    let (kind, identity) = match origin {
        TransferOrigin::Entry => (0, 0),
        TransferOrigin::Continue => (1, 0),
        TransferOrigin::NormalCommit(o) => (2, o.0),
        TransferOrigin::Edge(e) => (3, e.0),
        TransferOrigin::Return(e) => (4, e.0),
    };
    (from.0, tag, dest, kind, identity)
}
fn graph_obligations(
    flow: ExecutionFacts<'_, '_>,
    f: &SsaFacts,
    graph: &Graph,
) -> Result<(), SsaError> {
    if f.points.first().map(|p| (p.id, p.kind)) != Some((PointId(0), PointKind::Entry))
        || f.operations.len() != flow.operations().len()
        || f.block_entries.len() != flow.blocks().len()
        || f.terminals.len() != flow.blocks().len()
    {
        return Err(bug("SSA position directories are incomplete"));
    }
    let mut owned = filled(f.points.len(), false)?;
    owned[0] = true;
    let mut claim = |p: PointId, kind: PointKind| -> Result<(), SsaError> {
        let index = p.0 as usize;
        if f.points
            .get(index)
            .is_none_or(|p| p.id.0 as usize != index || p.kind != kind)
            || owned[index]
        {
            return Err(bug("state point has duplicate or wrong original ownership"));
        }
        owned[index] = true;
        Ok(())
    };
    let mut expected = Vec::new();
    push(
        &mut expected,
        arc_key(
            PointId(0),
            Target::Point(f.block_entries[0]),
            TransferOrigin::Entry,
        ),
    )?;
    let mut tails = filled(flow.blocks().len(), PointId(0))?;
    for block in flow.blocks() {
        let mut previous = f.block_entries[block.id().0 as usize];
        claim(previous, PointKind::BlockEntry(block.id()))?;
        for index in block.operations() {
            let operation = &flow.operations()[index as usize];
            let site = f.operations[index as usize];
            if site.operation != operation.id()
                || site.commit_rule != operation.normal().commit()
                || site.commit_pc != operation.normal().commit_pc()
            {
                return Err(bug("operation site identity differs"));
            }
            claim(site.execute, PointKind::Execute(operation.id()))?;
            push(
                &mut expected,
                arc_key(
                    previous,
                    Target::Point(site.execute),
                    TransferOrigin::Continue,
                ),
            )?;
            if flow.writes(operation).is_empty() != site.commit.is_none() {
                return Err(bug("normal commit position differs from P2 outputs"));
            }
            previous = if let Some(commit) = site.commit {
                claim(commit, PointKind::Commit(operation.id()))?;
                push(
                    &mut expected,
                    arc_key(
                        site.execute,
                        Target::Point(commit),
                        TransferOrigin::NormalCommit(operation.id()),
                    ),
                )?;
                commit
            } else {
                site.execute
            };
        }
        if block.terminal().is_some() {
            let terminal = graph.terminals[block.id().0 as usize]
                .ok_or(bug("terminal has no state position"))?;
            let site = f.terminals[block.id().0 as usize]
                .ok_or(bug("terminal binding directory omitted a position"))?;
            if site.block != block.id() || site.point != terminal {
                return Err(bug("terminal binding directory has a foreign position"));
            }
            claim(terminal, PointKind::Terminal(block.id()))?;
            push(
                &mut expected,
                arc_key(previous, Target::Point(terminal), TransferOrigin::Continue),
            )?;
            previous = terminal;
        }
        if block.terminal().is_none() && f.terminals[block.id().0 as usize].is_some() {
            return Err(bug(
                "non-terminal block has a terminal binding directory entry",
            ));
        }
        tails[block.id().0 as usize] = previous;
    }
    if owned.iter().any(|v| !v) {
        return Err(bug("state position has no original owner"));
    }
    for edge in flow.edges() {
        let from = match edge.role() {
            EdgeRole::Normal(_) => tails[edge.from().0 as usize],
            EdgeRole::Exception { .. } => match edge.source_op() {
                Some(op) => f.operations[op.0 as usize].execute,
                None => tails[edge.from().0 as usize],
            },
        };
        let target = match edge.target() {
            EdgeTarget::Block(b) => Target::Point(f.block_entries[b.0 as usize]),
            EdgeTarget::Exit(e) => Target::Exit(e),
        };
        push(
            &mut expected,
            arc_key(from, target, TransferOrigin::Edge(edge.id())),
        )?;
    }
    for exit in flow.exits() {
        if exit.kind() == ExitKind::Return {
            push(
                &mut expected,
                arc_key(
                    tails[exit.block().0 as usize],
                    Target::Exit(exit.id()),
                    TransferOrigin::Return(exit.id()),
                ),
            )?;
        }
    }
    let mut actual = Vec::new();
    for (i, t) in f.transfers.iter().enumerate() {
        if t.id.0 as usize != i {
            return Err(bug("state transfer identity differs"));
        }
        push(&mut actual, arc_key(t.from, t.target, t.origin))?;
    }
    expected.sort_unstable();
    actual.sort_unstable();
    if actual != expected {
        return Err(bug(
            "state graph does not preserve P2 normal/exception commits",
        ));
    }
    Ok(())
}

fn definitions(flow: ExecutionFacts<'_, '_>, f: &SsaFacts, w: &Work) -> Result<(), SsaError> {
    let mut state_owned = filled(f.states.len(), false)?;
    let mut value_owned = filled(f.values.len(), false)?;
    let mut entry_roles = filled(f.slots.len(), None)?;
    if let Some(s) = flow.entry().this() {
        entry_roles[slot(f, s)?.0 as usize] = Some(EntryRole::This);
    }
    for (i, p) in flow.entry().parameters().iter().enumerate() {
        entry_roles[slot(f, p.slot())?.0 as usize] = Some(EntryRole::Parameter(id(i)?));
    }
    if w.defs[0].len() != f.slots.len() {
        return Err(bug("entry does not define every tracked slot state"));
    }
    let mut entry_seen = filled(f.slots.len(), false)?;
    for &s in &w.defs[0] {
        let state = &f.states[s.0 as usize];
        if std::mem::replace(&mut entry_seen[state.slot.0 as usize], true) {
            return Err(bug("entry has duplicate exact-slot states"));
        }
        match (entry_roles[state.slot.0 as usize], state.kind) {
            (None, StateKind::Undefined) if flow.slot_is_valid(f.slots[state.slot.0 as usize]) => {}
            (None, StateKind::OutsideFrame)
                if !flow.slot_is_valid(f.slots[state.slot.0 as usize]) => {}
            (Some(role), StateKind::Value(v)) => {
                let value = &f.values[v.0 as usize];
                if value.point != PointId(0)
                    || value.slot != state.slot
                    || value.origin != ValueOrigin::Entry(role)
                    || value_owned[v.0 as usize]
                {
                    return Err(bug("parameter definition differs from exact P2 entry slot"));
                }
                value_owned[v.0 as usize] = true;
            }
            _ => {
                return Err(bug(
                    "entry manufactured a readable value or lost a parameter",
                ));
            }
        }
        state_owned[s.0 as usize] = true;
    }
    let mut cursor = 0usize;
    for op in flow.operations() {
        let site = f.operations[op.id().0 as usize];
        let commit = site.commit;
        if site.output_start != id(cursor)? || site.output_len != id(flow.writes(op).len())? {
            return Err(bug("operation output span differs from original outputs"));
        }
        for (i, &written) in flow.writes(op).iter().enumerate() {
            let output = f.outputs.get(cursor).ok_or(bug("normal output omitted"))?;
            cursor += 1;
            let key = slot(f, written)?;
            if output.operation != op.id()
                || output.output != id(i)?
                || output.slot != key
                || Some(output.point) != commit
            {
                return Err(bug("normal output lost its original operation/slot/commit"));
            }
            let state = &f.states[output.state.0 as usize];
            if state.slot != key
                || state.point != output.point
                || state_owned[output.state.0 as usize]
            {
                return Err(bug("normal output state identity differs"));
            }
            let established = !matches!(
                op.kind(),
                OpKind::UnresolvedInvoke {
                    result: crate::p2::InvokeResult::Unavailable,
                    ..
                } | OpKind::UnresolvedResult { .. }
            ) && flow.slot_is_valid(written);
            match f.output_value(output) {
                Some(v) if established => {
                    let value = &f.values[v.0 as usize];
                    let expected = if matches!(op.kind(), OpKind::MoveException) {
                        ValueOrigin::Exception(op.id())
                    } else {
                        ValueOrigin::Operation {
                            operation: op.id(),
                            output: id(i)?,
                        }
                    };
                    if state.kind != StateKind::Value(v)
                        || value.slot != key
                        || value.point != output.point
                        || value.origin != expected
                        || value_owned[v.0 as usize]
                    {
                        return Err(bug("normal value origin differs"));
                    }
                    value_owned[v.0 as usize] = true;
                }
                None if !established
                    && state.kind == (StateKind::Unavailable { operation: op.id() }) => {}
                _ => {
                    return Err(bug(
                        "unavailable output was promoted to a value or a known value was lost",
                    ));
                }
            }
            state_owned[output.state.0 as usize] = true;
        }
        let Some(point) = commit else { continue };
        for &state in &w.defs[point.0 as usize] {
            let def = &f.states[state.0 as usize];
            if state_owned[state.0 as usize] {
                continue;
            }
            let tracked = f.slots[def.slot.0 as usize];
            let unknown = matches!(
                op.kind(),
                OpKind::UnresolvedInvoke {
                    result: crate::p2::InvokeResult::Unavailable,
                    ..
                } | OpKind::UnresolvedResult { .. }
            ) || flow.writes(op).iter().any(|s| !flow.slot_is_valid(*s));
            let expected = if unknown {
                StateKind::Unavailable { operation: op.id() }
            } else {
                StateKind::Invalidated { operation: op.id() }
            };
            if def.kind != expected
                || !flow.writes(op).iter().any(|d| {
                    tracked != *d
                        && u64::from(tracked.start.0) < u64::from(d.start.0) + u64::from(d.words())
                        && u64::from(d.start.0)
                            < u64::from(tracked.start.0) + u64::from(tracked.words())
                })
            {
                return Err(bug(
                    "slot invalidation has no overlapping original normal write",
                ));
            }
            state_owned[state.0 as usize] = true;
        }
        for written in flow.writes(op) {
            let begin = written.start.0.saturating_sub(1);
            let end = u64::from(written.start.0) + u64::from(written.words());
            let first = f.slots.partition_point(|s| s.start.0 < begin);
            for (offset, tracked) in f.slots[first..].iter().enumerate() {
                if u64::from(tracked.start.0) >= end {
                    break;
                }
                if u64::from(tracked.start.0) + u64::from(tracked.words())
                    <= u64::from(written.start.0)
                {
                    continue;
                }
                let slot = SlotId(id(first + offset)?);
                if w.defs[point.0 as usize]
                    .iter()
                    .filter(|&&s| f.states[s.0 as usize].slot == slot)
                    .count()
                    != 1
                {
                    return Err(bug(
                        "normal write omitted or duplicated an exact/overlapping state definition",
                    ));
                }
            }
        }
    }
    if cursor != f.outputs.len() {
        return Err(bug("SSA has foreign normal outputs"));
    }
    for (i, phi) in f.phis.iter().enumerate() {
        let s = &f.states[phi.state.0 as usize];
        let v = &f.values[phi.value.0 as usize];
        if phi.id.0 as usize != i
            || s.point != phi.point
            || s.slot != phi.slot
            || s.kind != StateKind::Phi(phi.id)
            || v.point != phi.point
            || v.slot != phi.slot
            || v.origin != ValueOrigin::Phi(phi.id)
            || state_owned[phi.state.0 as usize]
            || value_owned[phi.value.0 as usize]
        {
            return Err(bug("Phi identity, width or definition owner differs"));
        }
        state_owned[phi.state.0 as usize] = true;
        value_owned[phi.value.0 as usize] = true;
    }
    if state_owned.iter().any(|v| !v) || value_owned.iter().any(|v| !v) {
        return Err(bug("SSA contains foreign/unowned definitions"));
    }
    Ok(())
}

struct Replay<'a> {
    facts: &'a SsaFacts,
    graph: &'a Graph,
    work: &'a Work,
    claims: HashMap<(PointId, SlotId), StateId>,
    pending: Vec<PointId>,
}
impl Replay<'_> {
    fn source(&mut self, point: PointId, slot: SlotId, expected: StateId) -> Result<(), SsaError> {
        let definition = &self.facts.states[expected.0 as usize];
        if definition.slot != slot || !self.facts.dominates(definition.point, point) {
            return Err(SsaError::Correspondence {
                point,
                obligation: "exact-slot definition availability/dominance",
            });
        }
        self.pending.clear();
        push(&mut self.pending, point)?;
        while let Some(point) = self.pending.pop() {
            if let Some(&old) = self.claims.get(&(point, slot)) {
                if old != expected {
                    return Err(SsaError::Correspondence {
                        point,
                        obligation: "inconsistent reaching state without Phi",
                    });
                }
                continue;
            }
            self.claims.try_reserve(1).map_err(SsaError::Allocation)?;
            self.claims.insert((point, slot), expected);
            let definitions = &self.work.defs[point.0 as usize];
            let mut anchor = definitions
                .binary_search_by_key(&slot, |s| self.facts.states[s.0 as usize].slot)
                .ok()
                .map(|index| definitions[index]);
            let phis = &self.work.phis[point.0 as usize];
            if let Ok(index) =
                phis.binary_search_by_key(&slot, |p| self.facts.phis[p.0 as usize].slot)
                && anchor
                    .replace(self.facts.phis[phis[index].0 as usize].state)
                    .is_some()
            {
                return Err(bug("Phi and normal definition collide"));
            }
            if let Some(state) = anchor {
                if state != expected {
                    return Err(SsaError::Correspondence {
                        point,
                        obligation: "nearest original definition differs from SSA binding",
                    });
                }
            } else {
                let incoming = self.graph.incoming.row(point.0 as usize);
                if incoming.is_empty() {
                    return Err(bug("binding reaches entry without an initial state"));
                }
                for &edge in incoming {
                    push(&mut self.pending, self.facts.transfers[edge].from)?;
                }
            }
        }
        Ok(())
    }
}

pub(super) fn verify(
    flow: ExecutionFacts<'_, '_>,
    f: &SsaFacts,
    graph: &Graph,
) -> Result<(), SsaError> {
    graph_obligations(flow, f, graph)?;
    for (i, s) in f.states.iter().enumerate() {
        if s.id.0 as usize != i
            || s.slot.0 as usize >= f.slots.len()
            || s.point.0 as usize >= f.points.len()
        {
            return Err(bug("state arena identity/range differs"));
        }
    }
    for (i, v) in f.values.iter().enumerate() {
        if v.id.0 as usize != i
            || v.slot.0 as usize >= f.slots.len()
            || v.point.0 as usize >= f.points.len()
        {
            return Err(bug("value arena identity/range differs"));
        }
    }
    for state in &f.states {
        let valid = match state.kind {
            StateKind::Value(v) => (v.0 as usize) < f.values.len(),
            StateKind::Phi(p) => (p.0 as usize) < f.phis.len(),
            _ => true,
        };
        if !valid {
            return Err(bug("state refers outside the definition arenas"));
        }
    }
    for phi in &f.phis {
        if phi.point.0 as usize >= f.points.len()
            || phi.slot.0 as usize >= f.slots.len()
            || phi.state.0 as usize >= f.states.len()
            || phi.value.0 as usize >= f.values.len()
            || phi.incoming.iter().any(|i| {
                i.state.0 as usize >= f.states.len() || i.transfer.0 as usize >= f.transfers.len()
            })
        {
            return Err(bug("Phi contains a foreign arena identity"));
        }
    }
    if f.uses.iter().any(|u| {
        u.state.0 as usize >= f.states.len()
            || u.point.0 as usize >= f.points.len()
            || u.slot.0 as usize >= f.slots.len()
    }) || f.outputs.iter().any(|o| {
        o.state.0 as usize >= f.states.len()
            || f.output_value(o)
                .is_some_and(|v| v.0 as usize >= f.values.len())
    }) {
        return Err(bug("use/output contains a foreign arena identity"));
    }
    let mut expected_slots = Vec::new();
    for &slot in flow.slots() {
        push(&mut expected_slots, slot)?;
    }
    if let Some(slot) = flow.entry().this() {
        push(&mut expected_slots, slot)?;
    }
    for parameter in flow.entry().parameters() {
        push(&mut expected_slots, parameter.slot())?;
    }
    expected_slots.sort_unstable_by_key(|s| (s.start.0, s.words()));
    expected_slots.dedup();
    if expected_slots != f.slots {
        return Err(bug(
            "tracked slots differ from original exact-slot obligations",
        ));
    }
    let mut work = Work::new(f)?;
    for row in &mut work.defs {
        row.sort_unstable_by_key(|s| f.states[s.0 as usize].slot);
    }
    for row in &mut work.phis {
        row.sort_unstable_by_key(|p| f.phis[p.0 as usize].slot);
    }
    for row in &work.defs {
        if row
            .windows(2)
            .any(|pair| f.states[pair[0].0 as usize].slot == f.states[pair[1].0 as usize].slot)
        {
            return Err(bug("duplicate slot definitions at a state point"));
        }
    }
    for row in &work.phis {
        if row
            .windows(2)
            .any(|pair| f.phis[pair[0].0 as usize].slot == f.phis[pair[1].0 as usize].slot)
        {
            return Err(bug("duplicate slot Phis at a state point"));
        }
    }
    definitions(flow, f, &work)?;
    let mut replay = Replay {
        facts: f,
        graph,
        work: &work,
        claims: HashMap::new(),
        pending: Vec::new(),
    };
    let mut cursor = 0;
    for op in flow.operations() {
        let site = f.operations[op.id().0 as usize];
        let reads = flow.reads(op);
        if site.inputs_available != reads.is_ok() || site.use_start != id(cursor)? {
            return Err(bug("operation input availability or start differs"));
        }
        let Ok(reads) = reads else {
            if site.use_len != 0 {
                return Err(bug("unavailable grouping fabricated input bindings"));
            }
            continue;
        };
        if site.use_len != id(reads.len())? {
            return Err(bug("operation use span differs from original reads"));
        }
        for (argument, &s) in reads.iter().enumerate() {
            let usage = f.uses.get(cursor).ok_or(bug("operation use omitted"))?;
            cursor += 1;
            if usage.site
                != (UseSite::Operation {
                    operation: op.id(),
                    argument: id(argument)?,
                })
                || usage.point != f.operations[op.id().0 as usize].execute
                || usage.slot != slot(f, s)?
            {
                return Err(bug("operation input identity/order differs"));
            }
            replay.source(usage.point, usage.slot, usage.state)?;
        }
    }
    for block in flow.blocks() {
        if let Some(site) = f.terminals[block.id().0 as usize]
            && (site.use_start != id(cursor)?
                || site.use_len != id(flow.terminal_reads(block).len())?)
        {
            return Err(bug("terminal use span differs from original reads"));
        }
        for (argument, &s) in flow.terminal_reads(block).iter().enumerate() {
            let usage = f.uses.get(cursor).ok_or(bug("terminal use omitted"))?;
            cursor += 1;
            if usage.site
                != (UseSite::Terminal {
                    block: block.id(),
                    argument: id(argument)?,
                })
                || usage.slot != slot(f, s)?
                || Some(usage.point) != graph.terminals[block.id().0 as usize]
            {
                return Err(bug("terminal input identity/order differs"));
            }
            replay.source(usage.point, usage.slot, usage.state)?;
        }
    }
    if cursor != f.uses.len() {
        return Err(bug("SSA contains foreign uses"));
    }
    for phi in &f.phis {
        let incoming = graph.incoming.row(phi.point.0 as usize);
        let mut actual = Vec::new();
        for input in &phi.incoming {
            let transfer = f
                .transfers
                .get(input.transfer.0 as usize)
                .ok_or(bug("foreign Phi incoming transfer"))?;
            if transfer.target != Target::Point(phi.point) {
                return Err(bug("Phi input belongs to another state point"));
            }
            push(&mut actual, input.transfer.0 as usize)?;
            replay.source(transfer.from, phi.slot, input.state)?;
        }
        actual.sort_unstable();
        let mut expected = Vec::new();
        for &edge in incoming {
            push(&mut expected, edge)?;
        }
        expected.sort_unstable();
        if actual != expected {
            return Err(bug(
                "Phi inputs are not a bijection over incoming transfers",
            ));
        }
    }
    let mut needed = filled(f.phis.len(), false)?;
    let mut pending = Vec::new();
    for usage in &f.uses {
        push(&mut pending, usage.state)?;
    }
    while let Some(s) = pending.pop() {
        if let StateKind::Phi(p) = f.states[s.0 as usize].kind
            && !needed[p.0 as usize]
        {
            needed[p.0 as usize] = true;
            for input in &f.phis[p.0 as usize].incoming {
                push(&mut pending, input.state)?;
            }
        }
    }
    if needed.iter().any(|v| !v) {
        return Err(bug("SSA retained a Phi outside the actual-use closure"));
    }
    check_availability(f)
}

fn check_availability(f: &SsaFacts) -> Result<(), SsaError> {
    if f.phis.is_empty() {
        for state in &f.states {
            let expected = match state.kind {
                StateKind::Value(_) => 1,
                StateKind::Undefined => 2,
                StateKind::Invalidated { .. } => 4,
                StateKind::OutsideFrame | StateKind::Unavailable { .. } => 8,
                StateKind::Phi(_) => 0,
            };
            if expected == 0 || state.availability.0 != expected {
                return Err(bug("base-state availability differs"));
            }
        }
        return Ok(());
    }
    let mut consumers = lists(f.states.len())?;
    for phi in &f.phis {
        for input in &phi.incoming {
            push(&mut consumers[input.state.0 as usize], phi.state)?;
        }
    }
    let mut expected = filled(f.states.len(), 0u8)?;
    let mut queue = Vec::new();
    for bit in [1u8, 2, 4, 8] {
        queue.clear();
        for s in &f.states {
            let base = match s.kind {
                StateKind::Value(_) => 1,
                StateKind::Undefined => 2,
                StateKind::Invalidated { .. } => 4,
                StateKind::OutsideFrame | StateKind::Unavailable { .. } => 8,
                StateKind::Phi(_) => 0,
            };
            if base == bit {
                expected[s.id.0 as usize] |= bit;
                push(&mut queue, s.id)?;
            }
        }
        while let Some(s) = queue.pop() {
            for &next in &consumers[s.0 as usize] {
                if expected[next.0 as usize] & bit == 0 {
                    expected[next.0 as usize] |= bit;
                    push(&mut queue, next)?;
                }
            }
        }
    }
    if f.states
        .iter()
        .zip(expected)
        .any(|(s, bits)| bits == 0 || s.availability.0 != bits)
    {
        return Err(bug(
            "availability is not grounded in original state sources",
        ));
    }
    Ok(())
}
