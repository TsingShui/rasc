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
        MethodBody::HeapAccess(body) => super::heap_access::write(body, &mut output, budget)?,
        MethodBody::FreshArray(body) => super::fresh_array::write(body, &mut output, budget)?,
        MethodBody::ZeroFill(body) => super::zero_fill::write(body, &mut output, budget)?,
        MethodBody::Casts(body) => super::casts::write(body, &mut output, budget)?,
        MethodBody::Structured(body) => super::structured::write(body, &mut output, budget)?,
        MethodBody::Constructor(body) => super::constructor::write(body, &mut output, budget)?,
        MethodBody::CallChain(body) => super::call_chain::write(body, &mut output, budget)?,
        MethodBody::Receiver(body) => {
            if body.invoke.0 >= body.result.0 || body.result.0 >= body.returned.0 {
                return Err(RequestError::InternalInvariant(
                    "verified receiver origins were reordered",
                ));
            }
            super::prefix::write(
                &body.prefix,
                super::prefix::Scope::Before(body.invoke),
                &mut output,
                budget,
            )?;
            output.push(
                match body.expression {
                    crate::p8_java::receiver::ReceiverExpression::Parameter => "return p0.",
                    crate::p8_java::receiver::ReceiverExpression::This => "return this.",
                    crate::p8_java::receiver::ReceiverExpression::Super => "return super.",
                },
                budget,
            )?;
            output.push(&body.method, budget)?;
            output.push("()", budget)?;
            if let crate::pipeline::receiver::ResultUse::Add { literal, .. } = body.use_result {
                output.push(" + (", budget)?;
                output.arguments(format_args!("{literal}"), budget)?;
                output.push(")", budget)?;
            }
            output.push(";\n", budget)?;
        }
        MethodBody::Scalar {
            definitions,
            locals,
            returned,
            typing,
        } => scalar(definitions, locals, *returned, typing, &mut output, budget)?,
        MethodBody::Diamond(body) => diamond(body, &mut output, budget)?,
        MethodBody::Dispatcher(body) => super::dispatcher::write(body, &mut output, budget)?,
    }
    output.push("}\n", budget)?;
    budget.checkpoint()?;
    Ok(output.finish())
}
fn scalar(
    definitions: &[Definition],
    locals: &[ScalarLocalId],
    returned: ValueId,
    typing: &crate::p5_types::Solved,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    use crate::p5_types::Conversion;
    use crate::pipeline::scalar::Unary;
    let at = |id: ValueId| -> Result<usize, RequestError> {
        usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)
    };
    let named = |id: ValueId,
                 output: &mut TextBuffer,
                 budget: &mut MethodBudget<'_, '_>|
     -> Result<(), RequestError> {
        let local = locals.get(at(id)?).ok_or(RequestError::InternalInvariant(
            "verified scalar use has no local",
        ))?;
        output.push("v", budget)?;
        output.unsigned(u64::from(local.0), budget)
    };
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        if matches!(
            definition.expression,
            Expression::Constant(_) | Expression::ConstantWide(_)
        ) {
            continue;
        }
        let views = typing
            .uses
            .get(index)
            .ok_or(RequestError::InternalInvariant(
                "verified scalar lost its use conversions",
            ))?;
        for (position, conversion) in [views.left, views.right].into_iter().enumerate() {
            budget.charge_work(1)?;
            if let Some(Conversion::LiteralView { value, kind }) = conversion {
                output.push(kind.spelling()?, budget)?;
                output.arguments(format_args!(" c{index}_{position} = "), budget)?;
                super::literals::numeric(
                    &definitions[at(value)?].expression,
                    kind,
                    output,
                    budget,
                )?;
                output.push(";\n", budget)?;
            }
        }
        output.push(typing.types()[index].spelling()?, budget)?;
        output.push(" ", budget)?;
        named(
            ValueId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?),
            output,
            budget,
        )?;
        output.push(" = ", budget)?;
        let operand = |value: ValueId,
                       position: usize,
                       output: &mut TextBuffer,
                       budget: &mut MethodBudget<'_, '_>|
         -> Result<(), RequestError> {
            let conversion = if position == 0 {
                views.left
            } else {
                views.right
            };
            match conversion {
                Some(Conversion::BooleanToInt(original)) if original.0 == value.0 => {
                    output.push("(", budget)?;
                    named(value, output, budget)?;
                    output.push(" ? 1 : 0)", budget)
                }
                Some(Conversion::Proven01ToBoolean(original)) if original.0 == value.0 => {
                    output.push("(", budget)?;
                    named(value, output, budget)?;
                    output.push(" != 0)", budget)
                }
                Some(Conversion::Identity(original)) if original.0 == value.0 => {
                    named(value, output, budget)
                }
                Some(Conversion::LiteralView {
                    value: original, ..
                }) if original.0 == value.0 => {
                    output.arguments(format_args!("c{index}_{position}"), budget)
                }
                _ => Err(RequestError::InternalInvariant(
                    "verified scalar operand changed its use site",
                )),
            }
        };
        match definition.expression {
            Expression::String(ref units) => super::literals::string(units, output, budget)?,
            Expression::Parameter(parameter) => {
                output.push("p", budget)?;
                output.unsigned(u64::from(parameter), budget)?;
            }
            Expression::Copy(value) => operand(value, 0, output, budget)?,
            Expression::Unary(op, value) => {
                match op {
                    Unary::Neg | Unary::NegLong | Unary::NegFloat | Unary::NegDouble => {
                        output.push("-", budget)?
                    }
                    Unary::Not | Unary::NotLong => output.push("~", budget)?,
                    Unary::Byte => output.push("(byte) ", budget)?,
                    Unary::Char => output.push("(char) ", budget)?,
                    Unary::Short => output.push("(short) ", budget)?,
                    Unary::IntToLong | Unary::LongToInt | Unary::Convert { .. } => {
                        output.push("(", budget)?;
                        output.push(op.signature().1.spelling()?, budget)?;
                        output.push(") ", budget)?;
                    }
                }
                operand(value, 0, output, budget)?;
            }
            Expression::Compare(a, op, b) => super::comparison::write(
                op,
                |right, output, budget| {
                    operand(
                        if right { b } else { a },
                        usize::from(right),
                        output,
                        budget,
                    )
                },
                output,
                budget,
            )?,
            Expression::Binary(a, op, b) => {
                operand(a, 0, output, budget)?;
                output.push(
                    match op {
                        Binary::Add => " + ",
                        Binary::Sub => " - ",
                        Binary::Mul => " * ",
                        Binary::Div => " / ",
                        Binary::Rem => " % ",
                        Binary::And => " & ",
                        Binary::Or => " | ",
                        Binary::Xor => " ^ ",
                        Binary::Shl => " << ",
                        Binary::Shr => " >> ",
                        Binary::Ushr => " >>> ",
                    },
                    budget,
                )?;
                operand(b, 1, output, budget)?;
            }
            Expression::Constant(_) | Expression::ConstantWide(_) => {
                return Err(RequestError::InternalInvariant(
                    "literal entered a nonliteral statement",
                ));
            }
        }
        output.push(";\n", budget)?;
    }
    output.push("return ", budget)?;
    match typing.returned {
        Conversion::Identity(value) if value.0 == returned.0 => named(value, output, budget)?,
        Conversion::BooleanToInt(value) if value.0 == returned.0 => {
            output.push("(", budget)?;
            named(value, output, budget)?;
            output.push(" ? 1 : 0)", budget)?;
        }
        Conversion::Proven01ToBoolean(value) if value.0 == returned.0 => {
            output.push("(", budget)?;
            named(value, output, budget)?;
            output.push(" != 0)", budget)?;
        }
        Conversion::LiteralView { value, kind } if value.0 == returned.0 => {
            super::literals::numeric(&definitions[at(value)?].expression, kind, output, budget)?
        }
        _ => {
            return Err(RequestError::InternalInvariant(
                "verified scalar return changed its use site",
            ));
        }
    }
    output.push(";\n", budget)
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
            BranchExpression::Compare(a, compare, b) => super::comparison::write(
                compare,
                |right, output, budget| local(if right { b } else { a }, output, budget),
                output,
                budget,
            )?,
            BranchExpression::Binary(left, kind, ref right) => {
                local(left, output, budget)?;
                output.push(
                    match kind {
                        crate::pipeline::branch::Arithmetic::Add => " + ",
                        crate::pipeline::branch::Arithmetic::Sub => " - ",
                        crate::pipeline::branch::Arithmetic::Mul => " * ",
                        crate::pipeline::branch::Arithmetic::And => " & ",
                    },
                    budget,
                )?;
                match right {
                    crate::p8_java::branch::Right::Local(id) => local(*id, output, budget)?,
                    crate::p8_java::branch::Right::Literal(value) => {
                        output.arguments(format_args!("{value}"), budget)?
                    }
                }
            }
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
                Kind::Object => "java.lang.Object ",
                Kind::String => "java.lang.String ",
                Kind::Float => "float ",
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
        Kind::Float => {
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
        Kind::Object | Kind::String => {
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
