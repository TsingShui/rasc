use crate::p8_java::MethodBody;
use crate::p8_java::branch::{Assignment, Diamond, Expression as BranchExpression, LocalId};
use crate::pipeline::branch::Kind;
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
        MethodBody::HeapAccess(body) => super::heap_access::write(body, &mut output, budget)?,
        MethodBody::Structured(body) => {
            super::structured::write(body, &method.type_names, &mut output, budget)?
        }
        MethodBody::Diamond(body) => diamond(body, &method.type_names, &mut output, budget)?,
        MethodBody::Dispatcher(body) => {
            super::dispatcher::write(body, &method.type_names, &mut output, budget)?
        }
    }
    output.push("}\n", budget)?;
    budget.checkpoint()?;
    Ok(output.finish())
}
pub(super) fn returned(
    value: crate::p8_java::branch::ValueUse,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    output.push("return ", budget)?;
    value_use(value, output, budget)?;
    output.push(";\n", budget)
}
pub(super) fn value_use(
    value: crate::p8_java::branch::ValueUse,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    use crate::p8_java::branch::ValueUse;
    match value {
        ValueUse::DoubleLiteral { bits, .. } => super::literals::fp(bits, true, output, budget)?,
        ValueUse::FloatLiteral { bits, .. } => {
            super::literals::fp(u64::from(bits), false, output, budget)?
        }
        ValueUse::Null { kind, .. } => {
            output.push("(", budget)?;
            output.push(kind.spelling(), budget)?;
            output.push(") null", budget)?;
        }
        ValueUse::UpcastObject(value) => {
            output.push("(java.lang.Object) ", budget)?;
            local(value, output, budget)?;
        }
        ValueUse::Direct(value) => local(value, output, budget)?,
        ValueUse::Boolean01(value) => {
            output.push("(", budget)?;
            local(value, output, budget)?;
            output.push(" != 0)", budget)?;
        }
        ValueUse::Narrow { value, kind } => {
            output.push("(", budget)?;
            output.push(kind.spelling(), budget)?;
            output.push(") ", budget)?;
            local(value, output, budget)?;
        }
        ValueUse::BooleanWord(value) => {
            output.push("(", budget)?;
            local(value, output, budget)?;
            output.push(" ? 1 : 0)", budget)?;
        }
    }
    Ok(())
}
pub(super) fn local(
    id: LocalId,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    output.push("v", budget)?;
    output.unsigned(u64::from(id.0), budget)
}
pub(super) fn assignments(
    statements: &[Assignment],
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for statement in statements {
        budget.charge_work(1)?;
        local(statement.target, output, budget)?;
        output.push(" = ", budget)?;
        match statement.expression {
            BranchExpression::Unary(unary, source) => {
                use crate::pipeline::scalar::Unary;
                match unary {
                    Unary::Neg | Unary::NegLong | Unary::NegFloat | Unary::NegDouble => {
                        output.push("-", budget)?
                    }
                    Unary::Not | Unary::NotLong => output.push("~", budget)?,
                    _ => {
                        let kind = match unary {
                            Unary::Byte => "byte",
                            Unary::Char => "char",
                            Unary::Short => "short",
                            _ => unary.signature().1.spelling(),
                        };
                        output.push("(", budget)?;
                        output.push(kind, budget)?;
                        output.push(") ", budget)?;
                    }
                }
                output.push("(", budget)?;
                value_use(source, output, budget)?;
                output.push(")", budget)?;
            }
            BranchExpression::ReverseSubtract(literal, source) => {
                output.arguments(format_args!("{literal} - "), budget)?;
                value_use(source, output, budget)?;
            }
            BranchExpression::Compare(a, compare, b) => super::comparison::write(
                compare,
                |right, output, budget| value_use(if right { b } else { a }, output, budget),
                output,
                budget,
            )?,
            BranchExpression::Binary(left, kind, ref right) => {
                value_use(left, output, budget)?;
                output.push(
                    match kind {
                        crate::pipeline::branch::Arithmetic::Add => " + ",
                        crate::pipeline::branch::Arithmetic::Sub => " - ",
                        crate::pipeline::branch::Arithmetic::Mul => " * ",
                        crate::pipeline::branch::Arithmetic::And => " & ",
                        crate::pipeline::branch::Arithmetic::Or => " | ",
                        crate::pipeline::branch::Arithmetic::Xor => " ^ ",
                        crate::pipeline::branch::Arithmetic::Div => " / ",
                        crate::pipeline::branch::Arithmetic::Rem => " % ",
                        crate::pipeline::branch::Arithmetic::Shl => " << ",
                        crate::pipeline::branch::Arithmetic::Shr => " >> ",
                        crate::pipeline::branch::Arithmetic::Ushr => " >>> ",
                    },
                    budget,
                )?;
                match right {
                    crate::p8_java::branch::Right::Local(view) => value_use(*view, output, budget)?,
                    crate::p8_java::branch::Right::Literal(value) => {
                        output.arguments(format_args!("{value}"), budget)?
                    }
                }
            }
            BranchExpression::Local(view) => value_use(view, output, budget)?,
            BranchExpression::ConstantWide(value) => {
                output.arguments(format_args!("{value}L"), budget)?
            }
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
    type_names: &crate::p8_java::types::TypeNames,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for declaration in &body.locals {
        budget.charge_work(1)?;
        output.push(type_names.name(declaration.kind, budget)?, budget)?;
        output.push(" ", budget)?;
        local(declaration.id, output, budget)?;
        if let crate::p6_variables::normal::Initializer::Parameter(parameter) =
            declaration.initializer
        {
            match parameter {
                crate::p6_variables::normal::Parameter::This => output.push(" = this", budget)?,
                crate::p6_variables::normal::Parameter::Explicit(index) => {
                    output.push(" = p", budget)?;
                    output.unsigned(u64::from(index), budget)?;
                }
            }
        }
        output.push(";\n", budget)?;
    }
    assignments(&body.entry, output, budget)?;
    output.push("if (", budget)?;
    match body.condition_kind {
        Kind::Float | Kind::Long | Kind::Double => {
            return Err(RequestError::InternalInvariant(
                "unlowered FP branch entered verified Java",
            ));
        }
        Kind::Boolean => {
            if body.opcode == 0x38 {
                output.push("!", budget)?;
            }
            local(body.condition, output, budget)?;
        }
        Kind::Object
        | Kind::String
        | Kind::PrimitiveArray(_)
        | Kind::ObjectArray
        | Kind::Nominal(_) => {
            local(body.condition, output, budget)?;
            output.push(
                if body.opcode == 0x38 {
                    " == null"
                } else {
                    " != null"
                },
                budget,
            )?;
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
    output.push("}\n", budget)?;
    returned(body.returned, output, budget)
}
