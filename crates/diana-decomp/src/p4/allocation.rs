use super::build::*;
use super::constraints::Work;
use super::*;
use crate::p2::OpKind;
use crate::p3::{SsaMethod, ValueOrigin};
use crate::substrate::owned;

// A finite projection of existing copy/Phi source rules, not another register
// environment. Missing inputs do not remove known allocation origins.
pub(super) fn sources(
    ssa: &SsaMethod<'_>,
    f: &mut TypeFacts,
    work: &Work,
) -> Result<(), TypeError> {
    let s = ssa.facts();
    let mut users = lists(s.values().len())?;
    for rule in &work.rules {
        for &state in &rule.inputs {
            if let Some(value) = state_value(s, state) {
                push(&mut users[value.0 as usize], rule.target)?;
            }
        }
    }
    let mut queued = filled(s.values().len(), false)?;
    let mut queue = Vec::new();
    for value in s.values() {
        if let ValueOrigin::Operation { operation, .. } = value.origin()
            && matches!(
                ssa.execution().operations()[operation.0 as usize].kind(),
                OpKind::NewInstance { .. }
            )
        {
            push(&mut f.values[value.id().0 as usize].allocations, operation)?;
            push(&mut queue, value.id())?;
            queued[value.id().0 as usize] = true;
        }
    }
    while let Some(value) = queue.pop() {
        queued[value.0 as usize] = false;
        for &target in &users[value.0 as usize] {
            let mut changed = false;
            for i in 0..f.values[value.0 as usize].allocations.len() {
                let operation = f.values[value.0 as usize].allocations[i];
                let origins = &mut f.values[target.0 as usize].allocations;
                if let Err(at) = origins.binary_search(&operation) {
                    owned::reserve(origins, 1)?;
                    origins.insert(at, operation);
                    changed = true;
                }
            }
            if changed && !queued[target.0 as usize] {
                push(&mut queue, target)?;
                queued[target.0 as usize] = true;
            }
        }
    }
    Ok(())
}
