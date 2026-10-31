use crate::p8_java::call_chain::{Body, Statement};
use crate::pipeline::call_chain::ValueId;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
fn value(
    id: ValueId,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    output.push("v", budget)?;
    output.unsigned(u64::from(id.0), budget)
}
pub(super) fn write(
    body: &Body,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for statement in &body.statements {
        budget.charge_work(1)?;
        match statement {
            Statement::Constant { value: id, literal } => {
                output.push("int ", budget)?;
                value(*id, output, budget)?;
                output.push(" = ", budget)?;
                output.arguments(format_args!("{literal}"), budget)?;
                output.push(";\n", budget)?;
            }
            Statement::Call(id) => {
                let call = body
                    .calls
                    .get(usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?)
                    .ok_or(RequestError::InternalInvariant("owned chain lost call"))?;
                let crate::p8_java::call_chain::Call {
                    receiver,
                    arguments,
                    result,
                    qualifier,
                    method,
                    ..
                } = call;
                if let Some(local) = result {
                    output.push(&local.source_type, budget)?;
                    output.push(" ", budget)?;
                    value(local.value, output, budget)?;
                    output.push(" = ", budget)?;
                }
                match (qualifier, receiver) {
                    (Some(qualifier), None) => output.push(qualifier, budget)?,
                    (None, Some((receiver, _))) => value(*receiver, output, budget)?,
                    _ => {
                        return Err(RequestError::InternalInvariant(
                            "owned call chain changed receiver kind",
                        ));
                    }
                }
                output.push(".", budget)?;
                output.push(method, budget)?;
                output.push("(", budget)?;
                for (i, &argument) in arguments.iter().enumerate() {
                    budget.charge_work(1)?;
                    if i != 0 {
                        output.push(", ", budget)?;
                    }
                    value(argument, output, budget)?;
                }
                output.push(");\n", budget)?;
            }
        }
    }
    output.push("return ", budget)?;
    value(body.returned, output, budget)?;
    output.push(";\n", budget)
}
