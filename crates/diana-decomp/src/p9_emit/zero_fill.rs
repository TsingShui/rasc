use crate::p8_java::zero_fill::ZeroFillBody;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
pub(super) fn write(
    body: &ZeroFillBody,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for (i, _) in body.fills.iter().enumerate() {
        budget.charge_work(1)?;
        output.push("int unusedLength", budget)?;
        output.unsigned(
            u64::try_from(i).map_err(|_| RequestError::PlatformCapacity)?,
            budget,
        )?;
        output.push(" = p0.length;\n", budget)?;
    }
    output.push("return;\n", budget)
}
