use crate::p3_ssa::{Definition, SsaBody, ValueId};
use crate::{MethodBudget, RequestError};

/// Recognizes an original zero constant through ordinary moves only. Phi and
/// runtime computations do not establish a zero bit pattern.
pub(crate) fn proven(
    ssa: &SsaBody<'_>,
    mut value: ValueId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for _ in 0..ssa.values.len() {
        budget.charge_work(1)?;
        if ssa.is_zero(value)? {
            return Ok(true);
        }
        let index = usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?;
        let Some(Definition::OpResult(id)) = ssa.values.get(index) else {
            return Ok(false);
        };
        let operation = ssa.operation(*id)?;
        if !matches!(operation.op, crate::p2_flow::Op::Move(..)) {
            return Ok(false);
        }
        value = operation.input(0)?;
    }
    Err(RequestError::InternalInvariant(
        "cyclic zero move provenance",
    ))
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

pub(crate) fn evidence(
    ssa: &SsaBody<'_>,
    value: ValueId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ZeroEvidence>, RequestError> {
    Ok(proven(ssa, value, budget)?.then_some(ZeroEvidence { value }))
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
