use crate::p8_java::MethodBody;
use crate::p8_java::branch::{Assignment, Diamond, Expression as BranchExpression, LocalId};
use crate::pipeline::branch::Kind;
use crate::pipeline::scalar::{Binary, Definition, Expression, LocalId as ScalarLocalId, ValueId};
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError, VerifiedJavaMethod};

pub(crate) fn java(
    method: &VerifiedJavaMethod,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<String, RequestError> {
    let mut output = TextBuffer::method(budget);
    output.push(&method.header, budget)?;
    output.push(" {\n", budget)?;
    match &method.body {
        MethodBody::Scalar {
            definitions,
            locals,
            returned,
        } => scalar(definitions, locals, *returned, &mut output, budget)?,
        MethodBody::Diamond(body) => diamond(body, &mut output, budget)?,
    }
    output.push("}\n", budget)?;
    budget.checkpoint()?;
    Ok(output.finish())
}
fn scalar(
    definitions: &[Definition],
    locals: &[ScalarLocalId],
    returned: ValueId,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        output.push("int v", budget)?;
        let target = locals.get(index).ok_or(RequestError::InternalInvariant(
            "verified definition lost its local",
        ))?;
        output.unsigned(u64::from(target.0), budget)?;
        output.push(" = ", budget)?;
        let local = |id: ValueId,
                     output: &mut TextBuffer,
                     budget: &mut MethodBudget<'_, '_>|
         -> Result<(), RequestError> {
            let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
            let local = locals.get(index).ok_or(RequestError::InternalInvariant(
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
            Expression::Copy(id) => local(id, output, budget)?,
            Expression::Binary(a, kind, b) => {
                local(a, output, budget)?;
                output.push(
                    match kind {
                        Binary::Add => " + ",
                        Binary::Sub => " - ",
                        Binary::Mul => " * ",
                    },
                    budget,
                )?;
                local(b, output, budget)?;
            }
        }
        output.push(";\n", budget)?;
    }
    let returned = usize::try_from(returned.0).map_err(|_| RequestError::PlatformCapacity)?;
    let local = locals.get(returned).ok_or(RequestError::InternalInvariant(
        "verified return has no local",
    ))?;
    output.push("return v", budget)?;
    output.unsigned(u64::from(local.0), budget)?;
    output.push(";\n", budget)
}
fn local(
    id: LocalId,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    output.push("v", budget)?;
    output.unsigned(u64::from(id.0), budget)
}
fn assignments(
    statements: &[Assignment],
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for statement in statements {
        budget.charge_work(1)?;
        local(statement.target, output, budget)?;
        output.push(" = ", budget)?;
        match statement.expression {
            BranchExpression::Local(id) => local(id, output, budget)?,
            BranchExpression::Constant(value) => {
                output.arguments(format_args!("{value}"), budget)?
            }
        }
        output.push(";\n", budget)?;
    }
    Ok(())
}
fn diamond(
    body: &Diamond,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for declaration in &body.locals {
        budget.charge_work(1)?;
        output.push(
            match declaration.kind {
                Kind::Int => "int ",
                Kind::Boolean => "boolean ",
            },
            budget,
        )?;
        local(declaration.id, output, budget)?;
        if let Some(parameter) = declaration.parameter {
            output.push(" = p", budget)?;
            output.unsigned(u64::from(parameter), budget)?;
        }
        output.push(";\n", budget)?;
    }
    assignments(&body.entry, output, budget)?;
    output.push("if (", budget)?;
    match body.condition_kind {
        Kind::Boolean => {
            if body.opcode == 0x38 {
                output.push("!", budget)?;
            }
            local(body.condition, output, budget)?;
        }
        Kind::Int => {
            local(body.condition, output, budget)?;
            output.push(
                match body.opcode {
                    0x38 => " == 0",
                    0x39 => " != 0",
                    0x3a => " < 0",
                    0x3b => " >= 0",
                    0x3c => " > 0",
                    0x3d => " <= 0",
                    _ => {
                        return Err(RequestError::InternalInvariant(
                            "verified diamond comparison is invalid",
                        ));
                    }
                },
                budget,
            )?;
        }
    }
    output.push(") {\n", budget)?;
    assignments(&body.yes, output, budget)?;
    output.push("} else {\n", budget)?;
    assignments(&body.no, output, budget)?;
    output.push("}\nreturn ", budget)?;
    local(body.returned, output, budget)?;
    output.push(";\n", budget)
}
