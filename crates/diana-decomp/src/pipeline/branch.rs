use crate::input::MethodInput;
use crate::substrate::outcome::Outcome;
use crate::{DeclarationCatalog, MethodBudget, OutputReason, RequestError};

pub(crate) use crate::p2_flow::normal::{
    BlockId, CallId, DeclarationType, EdgeId, Op, ResultType, Terminal,
};
pub(crate) use crate::p3_ssa::normal::{BoundOp, Definition, Sym};
pub(crate) use crate::p5_types::normal::{Kind, TypedBody};

// Consumed directly by the scheduler; boxing would add a fallible allocation per method.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Analysis<'a> {
    Ready {
        storage: crate::p6_variables::normal::StorageBody<'a>,
        declaration_failure: Option<OutputReason>,
    },
    Declined(OutputReason),
}
#[cfg(test)]
impl<'a> Analysis<'a> {
    pub(crate) fn into_storage(self) -> Option<crate::p6_variables::normal::StorageBody<'a>> {
        match self {
            Self::Ready { storage, .. } => Some(storage),
            Self::Declined(_) => None,
        }
    }
}
pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Analysis<'a>, RequestError> {
    let flow = match crate::p2_flow::normal::decode(input, budget)? {
        Outcome::Ready(flow) => flow,
        Outcome::Declined(reason) => return Ok(Analysis::Declined(reason)),
    };
    let ssa = crate::p3_ssa::normal::build(flow, budget)?;
    let resolved = crate::p4_calls::normal::collect(input, catalog, ssa, budget)?;
    let declaration_failure = resolved.decline_reason;
    let typed = match crate::p5_types::normal::solve(input, resolved, budget)? {
        Outcome::Ready(typed) => typed,
        Outcome::Declined(reason) => {
            return Ok(Analysis::Declined(declaration_failure.unwrap_or(reason)));
        }
    };
    match crate::p6_variables::normal::allocate(typed, budget)? {
        Outcome::Ready(storage) => Ok(Analysis::Ready {
            storage,
            declaration_failure,
        }),
        Outcome::Declined(reason) => Ok(Analysis::Declined(declaration_failure.unwrap_or(reason))),
    }
}
