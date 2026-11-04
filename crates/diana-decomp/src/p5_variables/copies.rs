use crate::substrate::owned::reserve;
use crate::{RequestError, RunContext};

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
    pub source: CopySource,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopySource {
    Local(LocalId),
    Zero {
        evidence: crate::p4_types::zero::ZeroEvidence,
        view: crate::p4_types::zero::ZeroView,
        kind: LocalType,
    },
}
impl CopySource {
    pub(crate) fn local(self) -> Option<LocalId> {
        match self {
            Self::Local(local) => Some(local),
            Self::Zero { .. } => None,
        }
    }
}
fn source_kind(
    source: CopySource,
    types: &[LocalType],
    temps: &[Temporary],
) -> Result<LocalType, RequestError> {
    match source {
        CopySource::Local(local) => kind(local, types, temps),
        CopySource::Zero { kind, .. } => Ok(kind),
    }
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
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut RunContext<'_>) -> Result<(), RequestError> {
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
    budget: &mut RunContext<'_>,
) -> Result<CopyPlan, RequestError> {
    let mut pending = Vec::new();
    for (index, copy) in parallel.iter().enumerate() {
        budget.checkpoint()?;
        if ix(copy.target)? >= types.len()
            || copy.source.local().is_some_and(|local| {
                usize::try_from(local.0).map_or(true, |index| index >= types.len())
            })
        {
            return Err(RequestError::InternalInvariant(
                "parallel copy has a foreign local",
            ));
        }
        if types[ix(copy.target)?] != source_kind(copy.source, types, &[])? {
            return Err(RequestError::InternalInvariant(
                "parallel copy requires a conversion",
            ));
        }
        for previous in &parallel[..index] {
            budget.checkpoint()?;
            if previous.target == copy.target {
                return Err(RequestError::InternalInvariant(
                    "parallel copy writes one destination twice",
                ));
            }
        }
        if Some(copy.target) != copy.source.local() {
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
            budget.checkpoint()?;
            let mut needed = false;
            for source in &pending {
                budget.checkpoint()?;
                if Some(copy.target) == source.source.local() {
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
        let source = pending.iter().find_map(|copy| copy.source.local()).ok_or(
            RequestError::InternalInvariant("copy dependencies have no local source"),
        )?;
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
                source: CopySource::Local(source),
            },
            budget,
        )?;
        for copy in &mut pending {
            budget.checkpoint()?;
            if copy.source.local() == Some(source) {
                copy.source = CopySource::Local(local);
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
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    if parallel.is_empty() && plan.steps.is_empty() && plan.temporaries.is_empty() {
        budget.checkpoint()?;
        return Ok(());
    }
    let total = types
        .len()
        .checked_add(plan.temporaries.len())
        .ok_or(RequestError::PlatformCapacity)?;
    let mut state = Vec::new();
    reserve(&mut state, total, budget)?;
    for index in 0..types.len() {
        budget.checkpoint()?;
        state.push(Some(CopySource::Local(LocalId(
            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
        ))));
    }
    for (offset, temporary) in plan.temporaries.iter().enumerate() {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        if source_kind(copy.source, types, &plan.temporaries)?
            != kind(copy.target, types, &plan.temporaries)?
        {
            return Err(RequestError::InternalInvariant(
                "sequential copy changes type",
            ));
        }
        let value =
            match copy.source {
                CopySource::Local(local) => state.get(ix(local)?).copied().flatten().ok_or(
                    RequestError::InternalInvariant("copy reads an unassigned temporary"),
                )?,
                literal @ CopySource::Zero { .. } => literal,
            };
        let target = state
            .get_mut(ix(copy.target)?)
            .ok_or(RequestError::InternalInvariant(
                "copy writes outside its arena",
            ))?;
        *target = Some(value);
    }
    let mut expected = Vec::new();
    reserve(&mut expected, types.len(), budget)?;
    for _ in types {
        budget.checkpoint()?;
        expected.push(None);
    }
    for copy in parallel {
        budget.checkpoint()?;
        let target = ix(copy.target)?;
        if copy.source.local().is_some_and(|local| {
            usize::try_from(local.0).map_or(true, |index| index >= types.len())
        }) || target >= types.len()
            || source_kind(copy.source, types, &[])? != types[target]
        {
            return Err(RequestError::InternalInvariant(
                "parallel specification has invalid typed locals",
            ));
        }
        if expected[target].replace(copy.source).is_some() {
            return Err(RequestError::InternalInvariant(
                "parallel specification has duplicate destinations",
            ));
        }
    }
    for (index, actual) in state.iter().enumerate().take(types.len()) {
        budget.checkpoint()?;
        let local = LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let expected = expected[index].unwrap_or(CopySource::Local(local));
        if *actual != Some(expected) {
            return Err(RequestError::InternalInvariant(
                "copy plan changes simultaneous assignment semantics",
            ));
        }
    }
    Ok(())
}
