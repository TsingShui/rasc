use crate::pipeline::scalar::Compare;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
pub(super) fn write(
    compare: Compare,
    mut operand: impl FnMut(
        bool,
        &mut TextBuffer,
        &mut MethodBudget<'_, '_>,
    ) -> Result<(), RequestError>,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    operand(false, output, budget)?;
    output.push(" < ", budget)?;
    operand(true, output, budget)?;
    output.push(" ? -1 : ", budget)?;
    operand(false, output, budget)?;
    output.push(" > ", budget)?;
    operand(true, output, budget)?;
    output.push(" ? 1 : ", budget)?;
    if let Some(bias) = compare.bias() {
        operand(false, output, budget)?;
        output.push(" == ", budget)?;
        operand(true, output, budget)?;
        output.arguments(format_args!(" ? 0 : {bias}"), budget)
    } else {
        output.push("0", budget)
    }
}
