use super::build::{self, Work};
use super::dominance::Dominance;
use super::graph::Graph;
use super::scratch::{bug, filled, id, lists, push};
use super::*;
use std::collections::BinaryHeap;

pub(super) fn place(
    f: &mut SsaFacts,
    g: &Graph,
    dom: &Dominance,
    w: &mut Work,
) -> Result<(), SsaError> {
    let n = f.points.len();
    let mut live = filled(n, 0usize)?;
    let mut killed = filled(n, 0usize)?;
    let mut frontier = filled(n, 0usize)?;
    let mut scanned = filled(n, 0usize)?;
    let mut queue = Vec::new();
    let mut subtree = Vec::new();
    let mut roots = BinaryHeap::new();
    for slot in 0..f.slots.len() {
        if w.slot_uses[slot].is_empty() || w.slot_defs[slot].iter().all(|p| p.0 == 0) {
            continue;
        }
        let stamp = slot + 1;
        queue.clear();
        for p in &w.slot_defs[slot] {
            killed[p.0 as usize] = stamp;
        }
        for p in &w.slot_uses[slot] {
            let p = p.0 as usize;
            if live[p] != stamp {
                live[p] = stamp;
                push(&mut queue, p)?;
            }
        }
        while let Some(p) = queue.pop() {
            for &edge in g.incoming.row(p) {
                let pred = f.transfers[edge].from.0 as usize;
                if killed[pred] != stamp && live[pred] != stamp {
                    live[pred] = stamp;
                    push(&mut queue, pred)?;
                }
            }
        }
        roots.clear();
        for p in &w.slot_defs[slot] {
            if p.0 != 0 {
                roots.try_reserve(1).map_err(SsaError::Allocation)?;
                roots.push((dom.depth[p.0 as usize], p.0 as usize));
            }
        }
        while let Some((root_depth, root)) = roots.pop() {
            subtree.clear();
            if scanned[root] != stamp {
                scanned[root] = stamp;
                push(&mut subtree, root)?;
            }
            while let Some(node) = subtree.pop() {
                for &edge in g.outgoing.row(node) {
                    let next = Graph::successor(f, edge);
                    if dom.depth[next] > root_depth || frontier[next] == stamp {
                        continue;
                    }
                    frontier[next] = stamp;
                    if live[next] != stamp {
                        continue;
                    }
                    let point = PointId(id(next)?);
                    let key = SlotId(id(slot)?);
                    let phi = PhiId(id(f.phis.len())?);
                    let value = build::value(f, key, point, ValueOrigin::Phi(phi))?;
                    let state = build::state(f, key, point, StateKind::Phi(phi))?;
                    push(
                        &mut f.phis,
                        Phi {
                            id: phi,
                            point,
                            slot: key,
                            state,
                            value,
                            incoming: Vec::new(),
                        },
                    )?;
                    push(&mut w.phis[next], phi)?;
                    if killed[next] != stamp {
                        roots.try_reserve(1).map_err(SsaError::Allocation)?;
                        roots.push((dom.depth[next], next));
                    }
                }
                for &child in dom.children.row(node) {
                    if scanned[child] != stamp {
                        scanned[child] = stamp;
                        push(&mut subtree, child)?;
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn bind(f: &mut SsaFacts, g: &Graph, dom: &Dominance, w: &Work) -> Result<(), SsaError> {
    let mut versions: Vec<Vec<StateId>> = lists(f.slots.len())?;
    let mut bindings = filled(w.seeds.len(), None)?;
    let mut walk = Vec::new();
    push(&mut walk, (0usize, false))?;
    while let Some((point, exit)) = walk.pop() {
        if exit {
            for &state in w.defs[point].iter().rev() {
                let slot = f.states[state.0 as usize].slot.0 as usize;
                if versions[slot].pop() != Some(state) {
                    return Err(bug("normal definition version stack disagrees"));
                }
            }
            for &phi in w.phis[point].iter().rev() {
                let phi = &f.phis[phi.0 as usize];
                if versions[phi.slot.0 as usize].pop() != Some(phi.state) {
                    return Err(bug("Phi version stack disagrees"));
                }
            }
            continue;
        }
        for &phi in &w.phis[point] {
            let phi = &f.phis[phi.0 as usize];
            push(&mut versions[phi.slot.0 as usize], phi.state)?;
        }
        for &state in &w.defs[point] {
            push(
                &mut versions[f.states[state.0 as usize].slot.0 as usize],
                state,
            )?;
        }
        for &index in &w.uses[point] {
            let slot = w.seeds[index].slot.0 as usize;
            bindings[index] = Some(
                *versions[slot]
                    .last()
                    .ok_or(bug("use has a Pending source after entry"))?,
            );
        }
        for &edge in g.outgoing.row(point) {
            let target = Graph::successor(f, edge);
            for &phi in &w.phis[target] {
                let phi = &mut f.phis[phi.0 as usize];
                let state = *versions[phi.slot.0 as usize]
                    .last()
                    .ok_or(bug("Phi input has a Pending source"))?;
                push(
                    &mut phi.incoming,
                    PhiIncoming {
                        transfer: TransferId(id(edge)?),
                        state,
                    },
                )?;
            }
        }
        push(&mut walk, (point, true))?;
        for &child in dom.children.row(point).iter().rev() {
            push(&mut walk, (child, false))?;
        }
    }
    for (seed, binding) in w.seeds.iter().zip(bindings) {
        push(
            &mut f.uses,
            UseBinding {
                site: seed.site,
                point: seed.point,
                slot: seed.slot,
                state: binding.ok_or(bug("renaming omitted an original use"))?,
            },
        )?;
    }
    Ok(())
}

fn root(aliases: &mut [usize], mut state: usize) -> usize {
    let start = state;
    while aliases[state] != state {
        state = aliases[state];
    }
    let mut next = start;
    while aliases[next] != next {
        let old = aliases[next];
        aliases[next] = state;
        next = old;
    }
    state
}

pub(super) fn compact(f: &mut SsaFacts) -> Result<(), SsaError> {
    if f.phis.is_empty() {
        return Ok(());
    }
    let mut aliases = filled(f.states.len(), 0usize)?;
    for (i, a) in aliases.iter_mut().enumerate() {
        *a = i;
    }
    let mut users = lists(f.states.len())?;
    let mut queue = Vec::new();
    let mut queued = filled(f.phis.len(), true)?;
    for phi in &f.phis {
        push(&mut queue, phi.id)?;
        for input in &phi.incoming {
            push(&mut users[input.state.0 as usize], phi.id)?;
        }
    }
    while let Some(id) = queue.pop() {
        queued[id.0 as usize] = false;
        let phi = &f.phis[id.0 as usize];
        let own = phi.state.0 as usize;
        if root(&mut aliases, own) != own {
            continue;
        }
        let mut candidate = None;
        let mut distinct = false;
        for input in &phi.incoming {
            let source = root(&mut aliases, input.state.0 as usize);
            if source == own {
                continue;
            }
            match candidate {
                None => candidate = Some(source),
                Some(old) if old != source => {
                    distinct = true;
                    break;
                }
                _ => {}
            }
        }
        if distinct {
            continue;
        }
        let Some(target) = candidate else { continue };
        aliases[own] = target;
        for user in std::mem::take(&mut users[own]) {
            if !queued[user.0 as usize] {
                queued[user.0 as usize] = true;
                push(&mut queue, user)?;
            }
            if matches!(f.states[target].kind, StateKind::Phi(_)) {
                push(&mut users[target], user)?;
            }
        }
    }
    let mut needed = filled(f.phis.len(), false)?;
    let mut pending = Vec::new();
    for usage in &f.uses {
        push(&mut pending, usage.state)?;
    }
    while let Some(state) = pending.pop() {
        let state = root(&mut aliases, state.0 as usize);
        if let StateKind::Phi(phi) = f.states[state].kind
            && !needed[phi.0 as usize]
        {
            needed[phi.0 as usize] = true;
            for input in &f.phis[phi.0 as usize].incoming {
                push(&mut pending, input.state)?;
            }
        }
    }
    let mut phi_map = filled(f.phis.len(), None)?;
    let mut phi_count = 0usize;
    for phi in &f.phis {
        if needed[phi.id.0 as usize]
            && root(&mut aliases, phi.state.0 as usize) == phi.state.0 as usize
        {
            phi_map[phi.id.0 as usize] = Some(PhiId(id(phi_count)?));
            phi_count += 1;
        }
    }
    if phi_count == f.phis.len() {
        return Ok(());
    }
    let mut state_map = filled(f.states.len(), None)?;
    let mut count = 0usize;
    for state in &f.states {
        if !matches!(state.kind,StateKind::Phi(p) if phi_map[p.0 as usize].is_none()) {
            state_map[state.id.0 as usize] = Some(StateId(id(count)?));
            count += 1;
        }
    }
    for s in 0..state_map.len() {
        let resolved = root(&mut aliases, s);
        if resolved != s {
            state_map[s] = state_map[resolved];
        }
    }
    let mut value_map = filled(f.values.len(), None)?;
    count = 0;
    for value in &f.values {
        if !matches!(value.origin,ValueOrigin::Phi(p) if phi_map[p.0 as usize].is_none()) {
            value_map[value.id.0 as usize] = Some(ValueId(id(count)?));
            count += 1;
        }
    }
    for mut state in std::mem::take(&mut f.states) {
        if matches!(state.kind,StateKind::Phi(p) if phi_map[p.0 as usize].is_none()) {
            continue;
        }
        state.id =
            state_map[state.id.0 as usize].ok_or(bug("state compaction lost a definition"))?;
        state.kind = match state.kind {
            StateKind::Value(v) => {
                StateKind::Value(value_map[v.0 as usize].ok_or(bug("missing value relocation"))?)
            }
            StateKind::Phi(p) => {
                StateKind::Phi(phi_map[p.0 as usize].ok_or(bug("missing Phi relocation"))?)
            }
            other => other,
        };
        push(&mut f.states, state)?;
    }
    for mut value in std::mem::take(&mut f.values) {
        if matches!(value.origin,ValueOrigin::Phi(p) if phi_map[p.0 as usize].is_none()) {
            continue;
        }
        value.id =
            value_map[value.id.0 as usize].ok_or(bug("value compaction lost a definition"))?;
        if let ValueOrigin::Phi(p) = value.origin {
            value.origin = ValueOrigin::Phi(phi_map[p.0 as usize].ok_or(bug("value lost Phi"))?);
        }
        push(&mut f.values, value)?;
    }
    for usage in &mut f.uses {
        usage.state =
            state_map[usage.state.0 as usize].ok_or(bug("use lost source during compaction"))?;
    }
    for output in &mut f.outputs {
        output.state = state_map[output.state.0 as usize].ok_or(bug("output lost source"))?;
    }
    for mut phi in std::mem::take(&mut f.phis) {
        let Some(new) = phi_map[phi.id.0 as usize] else {
            continue;
        };
        phi.id = new;
        phi.state = state_map[phi.state.0 as usize].ok_or(bug("Phi lost state"))?;
        phi.value = value_map[phi.value.0 as usize].ok_or(bug("Phi lost value"))?;
        for input in &mut phi.incoming {
            input.state =
                state_map[input.state.0 as usize].ok_or(bug("Phi lost incoming state"))?;
        }
        push(&mut f.phis, phi)?;
    }
    Ok(())
}

pub(super) fn availability(f: &mut SsaFacts) -> Result<(), SsaError> {
    if f.phis.is_empty() {
        return Ok(());
    }
    let mut users = lists(f.states.len())?;
    let mut queued = filled(f.phis.len(), true)?;
    let mut queue = Vec::new();
    for phi in &f.phis {
        push(&mut queue, phi.id)?;
        for input in &phi.incoming {
            push(&mut users[input.state.0 as usize], phi.id)?;
        }
    }
    while let Some(id) = queue.pop() {
        queued[id.0 as usize] = false;
        let phi = &f.phis[id.0 as usize];
        let mut bits = 0;
        for input in &phi.incoming {
            bits |= f.states[input.state.0 as usize].availability.0;
        }
        let state = phi.state.0 as usize;
        if f.states[state].availability.0 == bits {
            continue;
        }
        f.states[state].availability.0 = bits;
        for &user in &users[state] {
            if !queued[user.0 as usize] {
                queued[user.0 as usize] = true;
                push(&mut queue, user)?;
            }
        }
    }
    if f.states.iter().any(|s| s.availability.0 == 0) {
        return Err(bug("availability cycle has no grounded source"));
    }
    Ok(())
}
