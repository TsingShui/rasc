use crate::p5_variables::LocalId;
use crate::p7_java::MethodBody;
use crate::p7_java::{Assignment, Expression as BranchExpression};
use crate::substrate::owned::TextBuffer;
use crate::{JavaMethod, RequestError, RunContext};

pub(crate) fn java(
    method: &JavaMethod,
    budget: &mut RunContext<'_>,
) -> Result<String, RequestError> {
    let mut output = TextBuffer::new();
    output.push(method.source_header(), budget)?;
    output.push(" {\n", budget)?;
    if let crate::p7_java::initializers::MemberSyntax::Initializer { always_abrupt } =
        method.syntax()
    {
        output.push("initExit: {\n", budget)?;
        if always_abrupt {
            output.push("if (true) {\n", budget)?;
        }
    }
    match method.body() {
        MethodBody::Protected(body) => {
            super::protected::write(body, method.type_names(), &mut output, budget)?
        }
        MethodBody::Structured(body) => {
            super::structured::write(body, method.type_names(), &mut output, budget)?
        }
        MethodBody::Dispatcher(body) => {
            super::dispatcher::write(body, method.type_names(), &mut output, budget)?
        }
    }
    if let crate::p7_java::initializers::MemberSyntax::Initializer { always_abrupt } =
        method.syntax()
    {
        if always_abrupt {
            output.push("}\n", budget)?;
        }
        output.push("}\n", budget)?;
    }
    output.push("}\n", budget)?;
    budget.checkpoint()?;
    Ok(output.finish())
}
pub(super) fn thrown(
    transfer: &crate::p7_java::dispatcher::Transfer,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    output.push("throw ", budget)?;
    match transfer {
        crate::p7_java::dispatcher::Transfer::Throw(value) => value_use(*value, output, budget)?,
        crate::p7_java::dispatcher::Transfer::Rethrow { handler, .. } => {
            output.push("caught", budget)?;
            output.unsigned(u64::from(handler.0), budget)?;
        }
        _ => {
            return Err(RequestError::InternalInvariant(
                "throw renderer lost checked transfer",
            ));
        }
    }
    output.push(";\n", budget)
}

pub(super) fn returned(
    value: crate::p7_java::ValueUse,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    output.push("return ", budget)?;
    value_use(value, output, budget)?;
    output.push(";\n", budget)
}
pub(super) fn value_use(
    value: crate::p7_java::ValueUse,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    value_with(value, output, budget, local)
}
pub(super) fn chain_argument(
    argument: &crate::p7_java::constructor_entry::PrologueArgument,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    value_with(
        argument.view,
        output,
        budget,
        |_, output, budget| match argument.source {
            crate::p7_java::constructor_entry::Argument::Parameter(index) => {
                output.push("p", budget)?;
                output.unsigned(u64::from(index), budget)
            }
            crate::p7_java::constructor_entry::Argument::Int(value) => {
                output.arguments(format_args!("{value}"), budget)
            }
            crate::p7_java::constructor_entry::Argument::Long(value) => {
                output.arguments(format_args!("{value}L"), budget)
            }
        },
    )
}
fn value_with(
    value: crate::p7_java::ValueUse,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
    mut source: impl FnMut(LocalId, &mut TextBuffer, &mut RunContext<'_>) -> Result<(), RequestError>,
) -> Result<(), RequestError> {
    use crate::p7_java::ValueUse;
    match value {
        ValueUse::DoubleLiteral { bits, .. } => super::literals::fp(bits, true, output, budget)?,
        ValueUse::FloatLiteral { bits, .. } => {
            super::literals::fp(u64::from(bits), false, output, budget)?
        }
        ValueUse::UpcastObject(value) => {
            output.push("(java.lang.Object) ", budget)?;
            source(value, output, budget)?;
        }
        ValueUse::Direct(value) => source(value, output, budget)?,
        ValueUse::NullAssignment(_) => output.push("null", budget)?,
        ValueUse::ObjectArrayView(value) => {
            output.push("((java.lang.Object[]) ", budget)?;
            source(value, output, budget)?;
            output.push(")", budget)?;
        }
        ValueUse::Boolean01(value) => {
            output.push("(", budget)?;
            source(value, output, budget)?;
            output.push(" != 0)", budget)?;
        }
        ValueUse::Narrow { value, kind } => {
            output.push("(", budget)?;
            output.push(kind.spelling(), budget)?;
            output.push(") ", budget)?;
            source(value, output, budget)?;
        }
        ValueUse::BooleanWord(value) => {
            output.push("(", budget)?;
            source(value, output, budget)?;
            output.push(" ? 1 : 0)", budget)?;
        }
    }
    Ok(())
}
pub(super) fn local(
    id: LocalId,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    output.push("v", budget)?;
    output.unsigned(u64::from(id.0), budget)
}
pub(super) fn assignments(
    statements: &[Assignment],
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for statement in statements {
        budget.checkpoint()?;
        local(statement.target, output, budget)?;
        output.push(" = ", budget)?;
        match statement.expression {
            BranchExpression::Unary(unary, source) => {
                use crate::p7_java::numeric_expression::Unary;
                match unary {
                    Unary::Negate => output.push("-", budget)?,
                    Unary::Complement => output.push("~", budget)?,
                    Unary::Cast(kind) => {
                        output.push("(", budget)?;
                        output.push(kind.spelling(), budget)?;
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
            BranchExpression::Compare(ref tree) => super::comparison::write(tree, output, budget)?,
            BranchExpression::Binary(left, kind, ref right) => {
                value_use(left, output, budget)?;
                output.push(
                    match kind {
                        crate::substrate::operators::Arithmetic::Add => " + ",
                        crate::substrate::operators::Arithmetic::Sub => " - ",
                        crate::substrate::operators::Arithmetic::Mul => " * ",
                        crate::substrate::operators::Arithmetic::And => " & ",
                        crate::substrate::operators::Arithmetic::Or => " | ",
                        crate::substrate::operators::Arithmetic::Xor => " ^ ",
                        crate::substrate::operators::Arithmetic::Div => " / ",
                        crate::substrate::operators::Arithmetic::Rem => " % ",
                        crate::substrate::operators::Arithmetic::Shl => " << ",
                        crate::substrate::operators::Arithmetic::Shr => " >> ",
                        crate::substrate::operators::Arithmetic::Ushr => " >>> ",
                    },
                    budget,
                )?;
                match right {
                    crate::p7_java::Right::Local(view) => value_use(*view, output, budget)?,
                    crate::p7_java::Right::Literal(value) => {
                        output.arguments(format_args!("{value}"), budget)?
                    }
                }
            }
            BranchExpression::Local(view) => value_use(view, output, budget)?,
            BranchExpression::CopyLiteral(crate::p5_variables::CopySource::Zero {
                view, ..
            }) => output.push(view.spelling(), budget)?,
            BranchExpression::CopyLiteral(crate::p5_variables::CopySource::Local(_)) => {
                return Err(RequestError::InternalInvariant(
                    "local emitted as literal copy",
                ));
            }
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
