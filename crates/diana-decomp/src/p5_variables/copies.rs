use crate::substrate::owned::reserve;
use crate::{MethodBudget, RequestError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LocalId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LocalTypeId(pub u128);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Width {
    One,
    Two,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LocalType {
    pub identity: LocalTypeId,
    pub width: Width,
}
impl LocalType {
    pub fn new(identity: LocalTypeId, words: u8) -> Result<Self, RequestError> {
        let width = match words {
            1 => Width::One,
            2 => Width::Two,
            _ => {
                return Err(RequestError::InternalInvariant(
                    "copy type has an invalid word width",
                ));
            }
        };
        Ok(Self { identity, width })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Copy {
    pub target: LocalId,
    pub source: LocalId,
}
pub(crate) struct Temporary {
    pub local: LocalId,
    pub kind: LocalType,
}
pub(crate) struct CopyPlan {
    pub steps: Vec<Copy>,
    pub temporaries: Vec<Temporary>,
}
fn ix(local: LocalId) -> Result<usize, RequestError> {
    usize::try_from(local.0).map_err(|_| RequestError::PlatformCapacity)
}
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
}
fn kind(
    local: LocalId,
    base: &[LocalType],
    temps: &[Temporary],
) -> Result<LocalType, RequestError> {
    let index = ix(local)?;
    if let Some(kind) = base.get(index) {
        return Ok(*kind);
    }
    let index = index
        .checked_sub(base.len())
        .ok_or(RequestError::InternalInvariant(
            "copy local is outside its arena",
        ))?;
    let temporary = temps.get(index).ok_or(RequestError::InternalInvariant(
        "copy uses an undeclared temporary",
    ))?;
    if temporary.local != local {
        return Err(RequestError::InternalInvariant(
            "copy temporary identity is not monotone",
        ));
    }
    Ok(temporary.kind)
}

pub(crate) fn plan(
    parallel: &[Copy],
    types: &[LocalType],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<CopyPlan, RequestError> {
    let mut pending = Vec::new();
    for (index, copy) in parallel.iter().enumerate() {
        budget.charge_work(1)?;
        if ix(copy.target)? >= types.len() || ix(copy.source)? >= types.len() {
            return Err(RequestError::InternalInvariant(
                "parallel copy has a foreign local",
            ));
        }
        if types[ix(copy.target)?] != types[ix(copy.source)?] {
            return Err(RequestError::InternalInvariant(
                "parallel copy requires a conversion",
            ));
        }
        for previous in &parallel[..index] {
            budget.charge_work(1)?;
            if previous.target == copy.target {
                return Err(RequestError::InternalInvariant(
                    "parallel copy writes one destination twice",
                ));
            }
        }
        if copy.target != copy.source {
            push(&mut pending, *copy, budget)?;
        }
    }
    let mut result = CopyPlan {
        steps: Vec::new(),
        temporaries: Vec::new(),
    };
    while !pending.is_empty() {
        budget.checkpoint()?;
        let mut ready = None;
        for (index, copy) in pending.iter().enumerate() {
            budget.charge_work(1)?;
            let mut needed = false;
            for source in &pending {
                budget.charge_work(1)?;
                if copy.target == source.source {
                    needed = true;
                    break;
                }
            }
            if !needed {
                ready = Some(index);
                break;
            }
        }
        if let Some(index) = ready {
            push(&mut result.steps, pending.swap_remove(index), budget)?;
            continue;
        }
        let source = pending[0].source;
        let index = types
            .len()
            .checked_add(result.temporaries.len())
            .ok_or(RequestError::PlatformCapacity)?;
        let local = LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let source_kind = kind(source, types, &result.temporaries)?;
        push(
            &mut result.temporaries,
            Temporary {
                local,
                kind: source_kind,
            },
            budget,
        )?;
        push(
            &mut result.steps,
            Copy {
                target: local,
                source,
            },
            budget,
        )?;
        for copy in &mut pending {
            budget.charge_work(1)?;
            if copy.source == source {
                copy.source = local;
            }
        }
    }
    verify(parallel, types, &result, budget)?;
    Ok(result)
}

/// Replays a simultaneous old-store correspondence, independently of leaf/cycle
/// choices. SSA supplies edge-source availability; this check adds no defaults.
pub(crate) fn verify(
    parallel: &[Copy],
    types: &[LocalType],
    plan: &CopyPlan,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let total = types
        .len()
        .checked_add(plan.temporaries.len())
        .ok_or(RequestError::PlatformCapacity)?;
    let mut state = Vec::new();
    reserve(&mut state, total, budget)?;
    for index in 0..types.len() {
        budget.charge_work(1)?;
        state.push(Some(LocalId(
            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
        )));
    }
    for (offset, temporary) in plan.temporaries.iter().enumerate() {
        budget.charge_work(1)?;
        let expected = types
            .len()
            .checked_add(offset)
            .ok_or(RequestError::PlatformCapacity)?;
        if ix(temporary.local)? != expected {
            return Err(RequestError::InternalInvariant(
                "temporary overlaps an original local",
            ));
        }
        state.push(None);
    }
    for copy in &plan.steps {
        budget.charge_work(1)?;
        if kind(copy.source, types, &plan.temporaries)?
            != kind(copy.target, types, &plan.temporaries)?
        {
            return Err(RequestError::InternalInvariant(
                "sequential copy changes type",
            ));
        }
        let value = state.get(ix(copy.source)?).copied().flatten().ok_or(
            RequestError::InternalInvariant("copy reads an unassigned temporary"),
        )?;
        let target = state
            .get_mut(ix(copy.target)?)
            .ok_or(RequestError::InternalInvariant(
                "copy writes outside its arena",
            ))?;
        *target = Some(value);
    }
    for (index, actual) in state.iter().enumerate().take(types.len()) {
        budget.charge_work(1)?;
        let local = LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut expected = local;
        let mut supplied = false;
        for copy in parallel {
            budget.charge_work(1)?;
            if ix(copy.source)? >= types.len()
                || ix(copy.target)? >= types.len()
                || types[ix(copy.source)?] != types[ix(copy.target)?]
            {
                return Err(RequestError::InternalInvariant(
                    "parallel specification has invalid typed locals",
                ));
            }
            if copy.target == local {
                if supplied {
                    return Err(RequestError::InternalInvariant(
                        "parallel specification has duplicate destinations",
                    ));
                }
                supplied = true;
                expected = copy.source;
            }
        }
        if *actual != Some(expected) {
            return Err(RequestError::InternalInvariant(
                "copy plan changes simultaneous assignment semantics",
            ));
        }
    }
    Ok(())
}
