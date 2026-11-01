use crate::p8_java::array_access::{Body, Statement};
use crate::pipeline::array_access::{ElementUse, Return, ValueId, index};
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
fn value(
    id: ValueId,
    body: &Body,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    output.push(
        if index(id)? < body.parameter_count {
            "p"
        } else {
            "v"
        },
        budget,
    )?;
    output.unsigned(u64::from(id.0), budget)
}
fn declare(
    id: ValueId,
    body: &Body,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let kind = body
        .types
        .get(index(id)?)
        .ok_or(RequestError::InternalInvariant(
            "array statement lost typed definition",
        ))?;
    output.push(kind.spelling(), budget)?;
    output.push(" ", budget)?;
    value(id, body, output, budget)?;
    output.push(" = ", budget)
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
                declare(*id, body, output, budget)?;
                output.arguments(format_args!("{literal}"), budget)?;
            }
            Statement::Call { result, call } => {
                if let Some(id) = result {
                    declare(*id, body, output, budget)?;
                }
                if !call.arguments.is_empty() {
                    return Err(RequestError::InternalInvariant(
                        "array zero-argument call acquired arguments",
                    ));
                }
                output.push(&call.qualifier, budget)?;
                output.push(".", budget)?;
                output.push(&call.method, budget)?;
                output.push("()", budget)?;
            }
            Statement::Load {
                array,
                index: at,
                result,
            } => {
                declare(*result, body, output, budget)?;
                value(*array, body, output, budget)?;
                output.push("[", budget)?;
                value(*at, body, output, budget)?;
                output.push("]", budget)?;
            }
            Statement::Store {
                array,
                index: at,
                value: element,
            } => {
                value(*array, body, output, budget)?;
                output.push("[", budget)?;
                value(*at, body, output, budget)?;
                output.push("] = ", budget)?;
                match element {
                    ElementUse::Value(id) => value(*id, body, output, budget)?,
                    ElementUse::Null(_) => output.push("null", budget)?,
                }
            }
            Statement::Length { array, result } => {
                declare(*result, body, output, budget)?;
                value(*array, body, output, budget)?;
                output.push(".length", budget)?;
            }
        }
        output.push(";\n", budget)?;
    }
    match body.returned {
        Return::Void => output.push("return;\n", budget),
        Return::Int(id) => {
            output.push("return ", budget)?;
            value(id, body, output, budget)?;
            output.push(";\n", budget)
        }
    }
}
