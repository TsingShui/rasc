use crate::pipeline::scalar::{Binary, Expression, ValueId};
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError, VerifiedJavaMethod};

pub(crate) fn java(
    method: &VerifiedJavaMethod,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<String, RequestError> {
    let mut output = TextBuffer::method(budget);
    output.push(&method.header, budget)?;
    output.push(" {\n", budget)?;
    for (index, definition) in method.definitions.iter().enumerate() {
        budget.charge_work(1)?;
        output.push("int v", budget)?;
        output.unsigned(u64::from(method.locals[index].0), budget)?;
        output.push(" = ", budget)?;
        let local = |id: ValueId,
                     output: &mut TextBuffer,
                     budget: &mut MethodBudget<'_, '_>|
         -> Result<(), RequestError> {
            let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
            let local = method
                .locals
                .get(index)
                .ok_or(RequestError::InternalInvariant(
                    "verified scalar use has no local",
                ))?;
            output.push("v", budget)?;
            output.unsigned(u64::from(local.0), budget)
        };
        match definition.expression {
            Expression::Parameter(parameter) => {
                output.push("p", budget)?;
                output.unsigned(u64::from(parameter), budget)?;
            }
            Expression::Constant(value) => output.arguments(format_args!("{value}"), budget)?,
            Expression::Copy(id) => local(id, &mut output, budget)?,
            Expression::Binary(a, kind, b) => {
                local(a, &mut output, budget)?;
                output.push(
                    match kind {
                        Binary::Add => " + ",
                        Binary::Sub => " - ",
                        Binary::Mul => " * ",
                    },
                    budget,
                )?;
                local(b, &mut output, budget)?;
            }
        }
        output.push(";\n", budget)?;
    }
    let returned =
        usize::try_from(method.returned.0).map_err(|_| RequestError::PlatformCapacity)?;
    let local = method
        .locals
        .get(returned)
        .ok_or(RequestError::InternalInvariant(
            "verified scalar return has no local",
        ))?;
    output.push("return v", budget)?;
    output.unsigned(u64::from(local.0), budget)?;
    output.push(";\n}", budget)?;
    budget.checkpoint()?;
    Ok(output.finish())
}
