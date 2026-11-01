use crate::p8_java::casts::CastBody;
use crate::pipeline::casts::CastValueId;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
fn value(
    id: CastValueId,
    parameters: usize,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    output.push(
        if usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)? < parameters {
            "p"
        } else {
            "v"
        },
        budget,
    )?;
    output.unsigned(u64::from(id.0), budget)
}
pub(super) fn write(
    body: &CastBody,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for cast in &body.casts {
        budget.charge_work(1)?;
        output.push(cast.kind.spelling(), budget)?;
        output.push(" ", budget)?;
        value(cast.result, body.parameter_count, output, budget)?;
        output.push(" = (", budget)?;
        output.push(cast.kind.spelling(), budget)?;
        output.push(") ", budget)?;
        value(cast.source, body.parameter_count, output, budget)?;
        output.push(";\n", budget)?;
    }
    output.push("return;\n", budget)
}
