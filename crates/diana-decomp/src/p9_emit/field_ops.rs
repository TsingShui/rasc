use crate::p8_java::field_ops::{Receiver, Statement};
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};

fn write_receiver(
    receiver: &Receiver,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    match receiver {
        Receiver::Instance(local) => super::java::local(*local, output, budget),
        Receiver::Static(qualifier) => output.push(qualifier, budget),
    }
}
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
            write_receiver(receiver, output, budget)?;
            output.push(".", budget)?;
            output.push(name, budget)?;
        }
        Statement::Store {
            receiver,
            name,
            value,
            ..
        } => {
            write_receiver(receiver, output, budget)?;
            output.push(".", budget)?;
            output.push(name, budget)?;
            output.push(" = ", budget)?;
            super::java::value_use(*value, output, budget)?;
        }
    }
    output.push(";\n", budget)
}
