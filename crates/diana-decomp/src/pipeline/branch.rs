use crate::input::MethodInput;
use crate::p0_knowledge::DeclarationCatalog;
use crate::substrate::outcome::Outcome;
use crate::{MethodBudget, OutputReason, RequestError};

// Consumed directly by the scheduler; boxing would add a fallible allocation per method.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Analysis<'a> {
    Ready {
        storage: crate::p6_variables::StorageBody<'a>,
    },
    Declined(OutputReason),
}
#[cfg(test)]
impl<'a> Analysis<'a> {}
pub(crate) fn build<'a>(
    schedule: &crate::engine::SchedulePermit,
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Analysis<'a>, RequestError> {
    let _p2 = crate::pipeline::phase_span("p2_flow");
    let flow = match crate::p2_flow::decode(schedule, input, budget)? {
        Outcome::Ready(flow) => flow,
        Outcome::Declined(reason) => return Ok(Analysis::Declined(reason)),
    };
    drop(_p2);
    let _p3 = crate::pipeline::phase_span("p3_ssa");
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
    drop(_p3);
    let _p4 = crate::pipeline::phase_span("p4_calls");
    let resolved =
        crate::p4_calls::collect(schedule, input, catalog, ssa, capture.as_ref(), budget)?;
    drop(_p4);
    let _p5 = crate::pipeline::phase_span("p5_types");
    let typed = match crate::p5_types::solve(schedule, input, resolved, capture.as_ref(), budget)? {
        Outcome::Ready(typed) => typed,
        Outcome::Declined(reason) => {
            return Ok(Analysis::Declined(reason));
        }
    };
    drop(_p5);
    let _p6 = crate::pipeline::phase_span("p6_variables");
    match crate::p6_variables::allocate(schedule, typed, budget)? {
        Outcome::Ready(storage) => Ok(Analysis::Ready { storage }),
        Outcome::Declined(reason) => Ok(Analysis::Declined(reason)),
    }
}
