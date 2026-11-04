use crate::p2_flow::Op;
use crate::p3_ssa::{Definition, SsaBody, Sym, ValueId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

const ZERO: u8 = 1;
const OTHER: u8 = 2;

fn index(value: ValueId) -> Result<usize, RequestError> {
    usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)
}

/// Finite provenance facts for word-zero constants, moves and Phi inputs.
/// Runtime computations and unresolved cycles cannot supply zero evidence.
pub(crate) struct ZeroFacts {
    facts: Vec<u8>,
}
impl ZeroFacts {
    pub(super) fn build(
        ssa: &SsaBody<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<Self, RequestError> {
        let mut facts = Vec::new();
        let mut dependents = Vec::new();
        let mut pending = Vec::new();
        for (at, definition) in ssa.values.iter().enumerate() {
            budget.checkpoint()?;
            let value = ValueId(u32::try_from(at).map_err(|_| RequestError::PlatformCapacity)?);
            let fact = match definition {
                Definition::Phi => 0,
                Definition::OpResult(id) => match ssa.operation(*id)?.op {
                    Op::Constant(_, 0) => ZERO,
                    Op::Move(..) => 0,
                    _ => OTHER,
                },
                _ => OTHER,
            };
            push(&mut facts, fact, budget)?;
            push(&mut dependents, Vec::new(), budget)?;
            if fact != 0 {
                push(&mut pending, value, budget)?;
            }
        }
        for operation in &ssa.operations {
            budget.checkpoint()?;
            if matches!(operation.op, Op::Move(..)) {
                let result = operation.result.ok_or(RequestError::InternalInvariant(
                    "zero provenance move lost result",
                ))?;
                push(&mut dependents[index(operation.input(0)?)?], result, budget)?;
            }
        }
        for phi in &ssa.phis {
            budget.checkpoint()?;
            for &(_, input) in &phi.inputs {
                budget.checkpoint()?;
                let Sym::Value(input) = input else {
                    return Err(RequestError::InternalInvariant(
                        "zero provenance lost Phi input",
                    ));
                };
                push(&mut dependents[index(input)?], phi.result, budget)?;
            }
        }
        propagate(&mut facts, &dependents, &mut pending, budget)?;
        for (at, fact) in facts.iter_mut().enumerate() {
            budget.checkpoint()?;
            if *fact == 0 {
                *fact = OTHER;
                push(
                    &mut pending,
                    ValueId(u32::try_from(at).map_err(|_| RequestError::PlatformCapacity)?),
                    budget,
                )?;
            }
        }
        propagate(&mut facts, &dependents, &mut pending, budget)?;
        Ok(Self { facts })
    }

    pub(crate) fn contains(&self, value: ValueId) -> Result<bool, RequestError> {
        let fact = self
            .facts
            .get(index(value)?)
            .ok_or(RequestError::InternalInvariant(
                "foreign zero provenance value",
            ))?;
        Ok(*fact == ZERO)
    }

    pub(crate) fn evidence(&self, value: ValueId) -> Result<Option<ZeroEvidence>, RequestError> {
        Ok(self.contains(value)?.then_some(ZeroEvidence { value }))
    }
}

fn propagate(
    facts: &mut [u8],
    dependents: &[Vec<ValueId>],
    pending: &mut Vec<ValueId>,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    while let Some(value) = pending.pop() {
        budget.checkpoint()?;
        let fact = facts[index(value)?];
        for &dependent in &dependents[index(value)?] {
            budget.checkpoint()?;
            let target = &mut facts[index(dependent)?];
            let merged = *target | fact;
            if merged != *target {
                *target = merged;
                push(pending, dependent, budget)?;
            }
        }
    }
    Ok(())
}

/// Independent replay of constant provenance against original SSA definitions.
pub(crate) fn proven(
    ssa: &SsaBody<'_>,
    value: ValueId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let mut seen = Vec::new();
    let mut pending = Vec::new();
    for _ in &ssa.values {
        push(&mut seen, false, budget)?;
    }
    push(&mut pending, value, budget)?;
    let mut anchored = false;
    while let Some(value) = pending.pop() {
        budget.checkpoint()?;
        let at = index(value)?;
        let visited = seen
            .get_mut(at)
            .ok_or(RequestError::InternalInvariant("foreign zero replay value"))?;
        if *visited {
            continue;
        }
        *visited = true;
        match ssa.values[at] {
            Definition::OpResult(id) => {
                let operation = ssa.operation(id)?;
                match operation.op {
                    Op::Constant(_, 0) => anchored = true,
                    Op::Move(..) => push(&mut pending, operation.input(0)?, budget)?,
                    _ => return Ok(false),
                }
            }
            Definition::Phi => {
                let mut found = false;
                for phi in &ssa.phis {
                    budget.checkpoint()?;
                    if phi.result == value {
                        found = true;
                        for &(_, input) in &phi.inputs {
                            budget.checkpoint()?;
                            let Sym::Value(input) = input else {
                                return Ok(false);
                            };
                            push(&mut pending, input, budget)?;
                        }
                    }
                }
                if !found {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(anchored)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ZeroEvidence {
    value: ValueId,
}
impl ZeroEvidence {
    pub(crate) fn value(self) -> ValueId {
        self.value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ZeroView {
    Int,
    Boolean,
    Float,
    Null,
}
impl ZeroView {
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Int => "0",
            Self::Boolean => "false",
            Self::Float => "0.0f",
            Self::Null => "null",
        }
    }
}
pub(crate) fn view(kind: super::Kind) -> Option<ZeroView> {
    match kind {
        super::Kind::Int => Some(ZeroView::Int),
        super::Kind::Boolean => Some(ZeroView::Boolean),
        super::Kind::Float => Some(ZeroView::Float),
        kind if super::reference_roots::reference(kind) => Some(ZeroView::Null),
        _ => None,
    }
}
