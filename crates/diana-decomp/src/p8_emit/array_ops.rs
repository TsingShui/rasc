use crate::p7_java::array_ops::Statement;
use crate::substrate::owned::TextBuffer;
use crate::{RequestError, RunContext};

pub(super) fn write(
    statement: &Statement,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    match statement {
        Statement::Length { array, result } => {
            super::java::local(*result, output, budget)?;
            output.push(" = ", budget)?;
            super::java::local(*array, output, budget)?;
            output.push(".length;\n", budget)
        }
        Statement::Load {
            array,
            index,
            result,
        } => {
            super::java::local(*result, output, budget)?;
            output.push(" = ", budget)?;
            super::java::local(*array, output, budget)?;
            output.push("[", budget)?;
            super::java::value_use(*index, output, budget)?;
            output.push("];\n", budget)
        }
        Statement::Store {
            array,
            index,
            value,
        } => {
            super::java::value_use(*array, output, budget)?;
            output.push("[", budget)?;
            super::java::value_use(*index, output, budget)?;
            output.push("] = ", budget)?;
            super::java::value_use(*value, output, budget)?;
            output.push(";\n", budget)
        }
    }
}
