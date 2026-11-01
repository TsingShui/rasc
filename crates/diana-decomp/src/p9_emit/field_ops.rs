use crate::p8_java::field_ops::Statement;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};

pub(super) fn write(
    statement: &Statement,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    match statement {
        Statement::Load {
            receiver,
            name,
            result,
            ..
        } => {
            super::java::local(*result, output, budget)?;
            output.push(" = ", budget)?;
            super::java::local(*receiver, output, budget)?;
            output.push(".", budget)?;
            output.push(name, budget)?;
        }
        Statement::Store {
            receiver,
            name,
            value,
            ..
        } => {
            super::java::local(*receiver, output, budget)?;
            output.push(".", budget)?;
            output.push(name, budget)?;
            output.push(" = ", budget)?;
            super::java::value_use(*value, output, budget)?;
        }
    }
    output.push(";\n", budget)
}
