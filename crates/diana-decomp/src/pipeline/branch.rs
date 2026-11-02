use crate::input::MethodInput;
use crate::substrate::outcome::Outcome;
use crate::{DeclarationCatalog, MethodBudget, OutputReason, RequestError};

// Consumed directly by the scheduler; boxing would add a fallible allocation per method.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Analysis<'a> {
    Ready {
        storage: crate::p6_variables::StorageBody<'a>,
        declaration_failure: Option<OutputReason>,
    },
    Declined(OutputReason),
}
#[cfg(test)]
impl<'a> Analysis<'a> {
    pub(crate) fn into_storage(self) -> Option<crate::p6_variables::StorageBody<'a>> {
        match self {
            Self::Ready { storage, .. } => Some(storage),
            Self::Declined(_) => None,
        }
    }
}
pub(crate) fn build<'a>(
    schedule: &crate::engine::SchedulePermit,
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Analysis<'a>, RequestError> {
    let flow = match crate::p2_flow::decode(schedule, input, budget)? {
        Outcome::Ready(flow) => flow,
        Outcome::Declined(reason) => return Ok(Analysis::Declined(reason)),
    };
    let ssa = crate::p3_ssa::build(schedule, flow, budget)?;
    let capture = if input.access_flags() & 0x10008 == 0x10000
        && ssa
            .operations
            .first()
            .is_some_and(|op| matches!(op.op, crate::p2_flow::Op::FieldStore(..)))
    {
        let ownership = crate::input::ownership::collect(input.program(), budget)?;
        match crate::p4_calls::capture::collect(schedule, input, &ownership, catalog, budget)? {
            Some(declaration) => {
                crate::p8_java::capture::verify(schedule, input, &declaration, budget)?
            }
            None => None,
        }
    } else {
        None
    };
    let resolved =
        crate::p4_calls::collect(schedule, input, catalog, ssa, capture.as_ref(), budget)?;
    let declaration_failure = resolved.decline_reason;
    let typed = match crate::p5_types::solve(schedule, input, resolved, capture.as_ref(), budget)? {
        Outcome::Ready(typed) => typed,
        Outcome::Declined(reason) => {
            return Ok(Analysis::Declined(declaration_failure.unwrap_or(reason)));
        }
    };
    match crate::p6_variables::allocate(schedule, typed, budget)? {
        Outcome::Ready(storage) => Ok(Analysis::Ready {
            storage,
            declaration_failure,
        }),
        Outcome::Declined(reason) => Ok(Analysis::Declined(declaration_failure.unwrap_or(reason))),
    }
}
