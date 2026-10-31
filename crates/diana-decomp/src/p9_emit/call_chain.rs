use crate::p8_java::call_chain::{Body, CallForm, Statement};
use crate::pipeline::call_chain::{Target, ValueId};
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
                match &call.form {
                    CallForm::Invoke {
                        receiver,
                        result,
                        upcast,
                    } => {
                        if matches!(call.target, Target::Constructor(_)) {
                            return Err(RequestError::InternalInvariant(
                                "constructor acquired ordinary call storage",
                            ));
                        }
                        if let Some(local) = result {
                            output.push(&local.source_type, budget)?;
                            output.push(" ", budget)?;
                            value(local.value, output, budget)?;
                            output.push(" = ", budget)?;
                        }
                        match (&call.qualifier, receiver) {
                            (Some(qualifier), None) => output.push(qualifier, budget)?,
                            (None, Some((receiver, _))) => {
                                if let Some(upcast) = upcast {
                                    output.push("((", budget)?;
                                    output.push(upcast, budget)?;
                                    output.push(") ", budget)?;
                                }
                                value(*receiver, output, budget)?;
                                if upcast.is_some() {
                                    output.push(")", budget)?;
                                }
                            }
                            _ => {
                                return Err(RequestError::InternalInvariant(
                                    "owned call changed receiver kind",
                                ));
                            }
                        }
                        output.push(".", budget)?;
                        output.push(&call.method, budget)?;
                    }
                    CallForm::Construct {
                        binding,
                        source_type,
                    } => {
                        if !matches!(call.target, Target::Constructor(_))
                            || binding.init != call.pc
                            || binding.allocation.0.0 >= call.pc.0
                        {
                            return Err(RequestError::InternalInvariant(
                                "deferred allocation lost paired source event",
                            ));
                        }
                        let Some(qualifier) = &call.qualifier else {
                            return Err(RequestError::InternalInvariant(
                                "constructor lost class spelling",
                            ));
                        };
                        output.push(source_type, budget)?;
                        output.push(" ", budget)?;
                        value(binding.value, output, budget)?;
                        output.push(" = new ", budget)?;
                        output.push(qualifier, budget)?;
                    }
                }
                output.push("(", budget)?;
                for (i, &argument) in call.arguments.iter().enumerate() {
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
