use super::build::*;
use super::constraints::{RuleKind, Work};
use super::*;
use crate::p3::{SsaFacts, ValueId};

fn merge_identity(target: Identity, source: Identity) -> Identity {
    match (target, source) {
        (Identity::Multiple, _) | (_, Identity::Multiple) => Identity::Multiple,
        (Identity::Unknown, x) => x,
        (x, Identity::Unknown) => x,
        (Identity::Root(a), Identity::Root(b)) if a == b => target,
        _ => Identity::Multiple,
    }
}
fn copy_source(f: &mut TypeFacts, to: ValueId, from: ValueId) -> Result<bool, TypeError> {
    let target = to.0 as usize;
    let source = from.0 as usize;
    let mut changed = false;
    let len = f.values[source].types.len();
    for i in 0..len {
        let name = f.values[source].types[i];
        if !f.values[target].types.contains(&name) {
            push(&mut f.values[target].types, name)?;
            changed = true;
        }
    }
    if f.values[source].type_unknown && !f.values[target].type_unknown {
        f.values[target].type_unknown = true;
        changed = true;
    }
    let source_bits = f.values[source].literal;
    if let Some(bits) = source_bits {
        match f.values[target].literal {
            None => {
                f.values[target].literal = Some(bits);
                changed = true;
            }
            Some(old) if old != bits && !f.values[target].mixed_literal => {
                f.values[target].mixed_literal = true;
                changed = true;
            }
            _ => {}
        }
    }
    if f.values[source].mixed_literal && !f.values[target].mixed_literal {
        f.values[target].mixed_literal = true;
        changed = true;
    }
    if let Some(range) = f.values[source].range {
        let new = match f.values[target].range {
            None => range,
            Some(old) => IntegralRange {
                min: old.min.min(range.min),
                max: old.max.max(range.max),
            },
        };
        if f.values[target].range != Some(new) {
            f.values[target].range = Some(new);
            changed = true;
        }
    }
    if f.values[source].range_unknown && !f.values[target].range_unknown {
        f.values[target].range_unknown = true;
        changed = true;
    }
    let identity = merge_identity(f.values[target].identity, f.values[source].identity);
    if identity != f.values[target].identity {
        f.values[target].identity = identity;
        changed = true;
    }
    Ok(changed)
}
fn unavailable(f: &mut TypeFacts, to: ValueId) -> bool {
    let v = &mut f.values[to.0 as usize];
    let changed =
        !v.type_unknown || !v.mixed_literal || !v.range_unknown || v.identity != Identity::Multiple;
    v.type_unknown = true;
    v.mixed_literal = true;
    v.range_unknown = true;
    v.identity = Identity::Multiple;
    changed
}
pub(super) fn sources(s: &SsaFacts, f: &mut TypeFacts, w: &Work) -> Result<(), TypeError> {
    let mut users = lists(f.values.len())?;
    let mut pending = filled(f.values.len(), false)?;
    for rule in &w.rules {
        pending[rule.target.0 as usize] = true;
    }
    // Unknown identity at a seed is not a promise that it aliases a later root.
    for (i, v) in f.values.iter_mut().enumerate() {
        if !pending[i] && v.identity == Identity::Unknown {
            v.identity = Identity::Multiple;
        }
    }
    let mut queue = Vec::new();
    let mut queued = filled(w.rules.len(), false)?;
    for (i, rule) in w.rules.iter().enumerate() {
        queued[i] = true;
        push(&mut queue, i)?;
        for &input in &rule.inputs {
            if let Some(v) = state_value(s, input) {
                push(&mut users[v.0 as usize], i)?;
            }
            if !s
                .state(input)
                .is_some_and(|s| s.availability().is_available())
            {
                unavailable(f, rule.target);
            }
        }
    }
    loop {
        while let Some(i) = queue.pop() {
            queued[i] = false;
            let rule = &w.rules[i];
            let mut changed = false;
            for &input in &rule.inputs {
                if let Some(v) = state_value(s, input) {
                    changed |= copy_source(f, rule.target, v)?;
                } else {
                    changed |= unavailable(f, rule.target);
                }
            }
            if rule.kind == RuleKind::Phi
                && let Identity::Root(root) = f.values[rule.target.0 as usize].identity
            {
                let from = s.values()[root.0 as usize].point();
                let to = s.values()[rule.target.0 as usize].point();
                // An allocation site reached from an earlier iteration is not
                // the same dynamic object as this iteration's definition.
                if !s.dominates(from, to) {
                    f.values[rule.target.0 as usize].identity = Identity::Multiple;
                    changed = true;
                }
            }
            if changed {
                for &user in &users[rule.target.0 as usize] {
                    if !queued[user] {
                        queued[user] = true;
                        push(&mut queue, user)?;
                    }
                }
            }
        }
        let mut added = false;
        for rule in &w.rules {
            let v = &mut f.values[rule.target.0 as usize];
            if v.literal.is_none() && !v.mixed_literal {
                v.mixed_literal = true;
                v.range_unknown = true;
                v.identity = Identity::Multiple;
                for &user in &users[rule.target.0 as usize] {
                    if !queued[user] {
                        queued[user] = true;
                        push(&mut queue, user)?;
                    }
                }
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    Ok(())
}
fn root(parent: &mut [usize], mut node: usize) -> usize {
    let start = node;
    while parent[node] != node {
        node = parent[node];
    }
    let mut next = start;
    while parent[next] != next {
        let old = parent[next];
        parent[next] = node;
        next = old;
    }
    node
}
fn union(parent: &mut [usize], size: &mut [usize], allowed: &mut [Families], a: usize, b: usize) {
    let mut a = root(parent, a);
    let mut b = root(parent, b);
    if a == b {
        return;
    }
    let common = allowed[a].intersect(allowed[b]);
    // A contradictory consumer cannot erase an independently established producer.
    // Keep the failed relation local; its use/incoming view records the conflict.
    if common.is_conflict() {
        return;
    }
    if size[a] < size[b] {
        std::mem::swap(&mut a, &mut b);
    }
    parent[b] = a;
    size[a] += size[b];
    allowed[a] = common;
}
pub(super) fn families(s: &SsaFacts, f: &mut TypeFacts, w: &mut Work) -> Result<(), TypeError> {
    let mut parent = Vec::new();
    let mut allowed = Vec::new();
    for (i, value) in f.values.iter().enumerate() {
        push(&mut parent, i)?;
        // Producer/opcode seeds and complete component projections establish
        // bounds. Propagated declaration subsets do not cover unknown sources;
        // Copy/Phi representation relations are joined below instead.
        push(&mut allowed, value.families)?;
    }
    // Constrain runtime Phi destinations before union, not the independently
    // interpreted literals or fixed producers on other incoming paths.
    for rule in &w.rules {
        if rule.kind != RuleKind::Phi || f.values[rule.target.0 as usize].literal().is_some() {
            continue;
        }
        for &input in &rule.inputs {
            if !s
                .state(input)
                .is_some_and(|state| state.availability().is_available())
            {
                continue;
            }
            if let Some(bits) = state_value(s, input).and_then(|v| f.values[v.0 as usize].literal())
            {
                let allowed = &mut allowed[rule.target.0 as usize];
                *allowed = allowed.intersect(literal_families(bits));
            }
        }
    }
    let mut size = filled(f.values.len(), 1usize)?;
    for rule in &w.rules {
        if !matches!(rule.kind, RuleKind::Copy | RuleKind::Phi)
            || f.values[rule.target.0 as usize].literal().is_some()
        {
            continue;
        }
        for &input in &rule.inputs {
            if let Some(v) = state_value(s, input)
                && f.values[v.0 as usize].literal().is_none()
            {
                union(
                    &mut parent,
                    &mut size,
                    &mut allowed,
                    rule.target.0 as usize,
                    v.0 as usize,
                );
            }
        }
    }
    for &(a, b) in &w.compatible {
        if let (Some(a), Some(b)) = (
            state_value(s, s.uses()[a].state()),
            state_value(s, s.uses()[b].state()),
        ) && f.values[a.0 as usize].literal().is_none()
            && f.values[b.0 as usize].literal().is_none()
        {
            let ar = root(&mut parent, a.0 as usize);
            let br = root(&mut parent, b.0 as usize);
            if !allowed[ar]
                .intersect(allowed[br])
                .intersect(Families::INT_REF)
                .is_conflict()
            {
                union(
                    &mut parent,
                    &mut size,
                    &mut allowed,
                    a.0 as usize,
                    b.0 as usize,
                );
            }
        }
    }
    let mut demands = filled(f.values.len(), Families(31))?;
    for use_view in &f.uses {
        if let Some(v) = state_value(s, use_view.state)
            && f.values[v.0 as usize].literal().is_none()
        {
            let r = root(&mut parent, v.0 as usize);
            // Requirements contradicted by a fixed producer fail at this use.
            if !allowed[r]
                .intersect(use_view.requirement.families)
                .is_conflict()
            {
                demands[r] = demands[r].intersect(use_view.requirement.families);
            }
        }
    }
    for i in 0..f.values.len() {
        let r = root(&mut parent, i);
        f.values[i].families = allowed[r].intersect(demands[r]);
        if f.values[i].families.is_conflict() {
            issue(
                f,
                IssueSite::Value(ValueId(id(i)?)),
                Problem::FamilyConflict,
            )?;
        }
    }
    for &(a, b) in &w.compatible {
        let mut common = Families::INT_REF;
        for state in [s.uses()[a].state(), s.uses()[b].state()] {
            if let Some(v) = state_value(s, state) {
                let fact = &f.values[v.0 as usize];
                if fact.literal().is_none() {
                    common = common.intersect(fact.families);
                } else if fact.literal().is_some_and(|b| b.value != 0) {
                    common = common.intersect(mask(Family::Integral));
                }
            }
        }
        for index in [a, b] {
            f.uses[index].requirement.families =
                f.uses[index].requirement.families.intersect(common);
        }
    }
    for value in &mut f.values {
        if value.literal().is_some_and(|b| b.value == 0) {
            value.identity = Identity::Unknown;
        }
    }
    Ok(())
}
