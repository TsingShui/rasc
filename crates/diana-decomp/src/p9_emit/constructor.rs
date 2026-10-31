use crate::p8_java::constructor::ConstructorBody;
use crate::pipeline::constructor::Argument;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};
pub(super) fn write(
    body: &ConstructorBody,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    if body.chain_pc.0 >= body.returned.0 || body.arguments.len() != body.chain.int_arity {
        return Err(RequestError::InternalInvariant(
            "constructor chain lost its source position",
        ));
    }
    output.push(
        match body.kind {
            crate::pipeline::constructor::ChainKind::Super => "super(",
            crate::pipeline::constructor::ChainKind::This => "this(",
        },
        budget,
    )?;
    for (index, &argument) in body.arguments.iter().enumerate() {
        budget.charge_work(1)?;
        if index != 0 {
            output.push(", ", budget)?;
        }
        match argument {
            Argument::Parameter(parameter) => {
                output.push("p", budget)?;
                output.unsigned(u64::from(parameter), budget)?;
            }
            Argument::Literal(value) => output.arguments(format_args!("{value}"), budget)?,
        }
    }
    output.push(");\n", budget)?;
    super::prefix::write(
        &body.body,
        super::prefix::Scope::After {
            chain: body.chain_pc,
            returned: body.returned,
        },
        output,
        budget,
    )?;
    output.push("return;\n", budget)
}
