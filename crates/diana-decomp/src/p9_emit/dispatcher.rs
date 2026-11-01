use super::java::{assignments, local};
use crate::p8_java::dispatcher::{Dispatcher, Edge, Transfer};
use crate::pipeline::branch::Kind;
use crate::substrate::owned::TextBuffer;
use crate::{MethodBudget, RequestError};

fn edge(
    edge: &Edge,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    assignments(&edge.copies, output, budget)?;
    output.push("pc = ", budget)?;
    output.unsigned(u64::from(edge.target.0), budget)?;
    output.push(";\ncontinue;\n", budget)
}
pub(super) fn locals(
    body: &Dispatcher,
    type_names: &crate::p8_java::types::TypeNames,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for declaration in &body.locals {
        budget.charge_work(1)?;
        output.push(type_names.name(declaration.kind, budget)?, budget)?;
        output.push(" ", budget)?;
        local(declaration.id, output, budget)?;
        if let Some(parameter) = declaration.parameter {
            match parameter {
                crate::p6_variables::normal::Parameter::This => output.push(" = this", budget)?,
                crate::p6_variables::normal::Parameter::Explicit(index) => {
                    output.push(" = p", budget)?;
                    output.unsigned(u64::from(index), budget)?;
                }
            }
        } else {
            // The SSA/edge definition certificate establishes assignment before
            // every original read; these DA-only values are never observed.
            output.push(
                match declaration.kind {
                    Kind::Int => " = 0",
                    Kind::Boolean => " = false",
                    Kind::Object
                    | Kind::String
                    | Kind::IntArray
                    | Kind::ObjectArray
                    | Kind::Nominal(_) => " = null",
                    Kind::Float => " = 0.0f",
                },
                budget,
            )?;
        }
        output.push(";\n", budget)?;
    }
    assignments(&body.entry, output, budget)?;

    Ok(())
}
pub(super) fn statements(
    case: &crate::p8_java::dispatcher::Case,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for statement in &case.statements {
        budget.charge_work(1)?;
        match statement {
            crate::p8_java::calls::Statement::Cast(cast) => {
                local(cast.target, output, budget)?;
                output.push(" = (", budget)?;
                output.push(cast.kind.spelling(), budget)?;
                output.push(") ", budget)?;
                super::java::value_use(cast.source, output, budget)?;
                output.push(";\n", budget)?;
            }
            crate::p8_java::calls::Statement::String { target, units, .. } => {
                local(*target, output, budget)?;
                output.push(" = ", budget)?;
                super::literals::string(units, output, budget)?;
                output.push(";\n", budget)?;
            }
            crate::p8_java::calls::Statement::ReceiverCall { target, call } => {
                if let Some(target) = target {
                    local(*target, output, budget)?;
                    output.push(" = ", budget)?;
                }
                match call.receiver {
                    crate::p8_java::calls::CallReceiver::Super(_) => {
                        output.push("super", budget)?
                    }
                    crate::p8_java::calls::CallReceiver::Value(value) => {
                        local(value, output, budget)?
                    }
                }
                output.push(".", budget)?;
                output.push(&call.method, budget)?;
                output.push("(", budget)?;
                for (index, &argument) in call.arguments.iter().enumerate() {
                    budget.charge_work(1)?;
                    if index != 0 {
                        output.push(", ", budget)?;
                    }
                    super::java::value_use(argument, output, budget)?;
                }
                output.push(");\n", budget)?;
            }
            crate::p8_java::calls::Statement::Field(statement) => {
                super::field_ops::write(statement, output, budget)?
            }
            crate::p8_java::calls::Statement::Array(statement) => {
                super::array_ops::write(statement, output, budget)?
            }
            crate::p8_java::calls::Statement::Assignment(statement) => {
                assignments(std::slice::from_ref(statement), output, budget)?
            }
            crate::p8_java::calls::Statement::StaticCall { target, call } => {
                if let Some(target) = target {
                    local(*target, output, budget)?;
                    output.push(" = ", budget)?;
                }
                output.push(&call.qualifier, budget)?;
                output.push(".", budget)?;
                output.push(&call.method, budget)?;
                output.push("(", budget)?;
                for (index, &argument) in call.arguments.iter().enumerate() {
                    budget.charge_work(1)?;
                    if index != 0 {
                        output.push(", ", budget)?;
                    }
                    super::java::value_use(argument, output, budget)?;
                }
                output.push(");\n", budget)?;
            }
        }
    }

    Ok(())
}
pub(super) fn open_if(
    transfer: &Transfer,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let Transfer::Branch {
        left,
        right,
        kind,
        opcode,
        ..
    } = transfer
    else {
        return Err(RequestError::InternalInvariant(
            "structured condition lost original branch",
        ));
    };
    output.push("if (", budget)?;
    if *kind == Kind::Boolean && right.is_none() {
        if *opcode == 0x38 {
            output.push("!", budget)?;
        }
        local(*left, output, budget)?;
    } else {
        local(*left, output, budget)?;
        output.push(
            match opcode {
                0x32 | 0x38 => " == ",
                0x33 | 0x39 => " != ",
                0x34 | 0x3a => " < ",
                0x35 | 0x3b => " >= ",
                0x36 | 0x3c => " > ",
                0x37 | 0x3d => " <= ",
                _ => {
                    return Err(RequestError::InternalInvariant(
                        "verified dispatcher comparison is invalid",
                    ));
                }
            },
            budget,
        )?;
        if let Some(right) = right {
            local(*right, output, budget)?;
        } else {
            output.push(
                if matches!(
                    kind,
                    Kind::Object
                        | Kind::String
                        | Kind::IntArray
                        | Kind::ObjectArray
                        | Kind::Nominal(_)
                ) {
                    "null"
                } else {
                    "0"
                },
                budget,
            )?;
        }
    }
    output.push(") {\n", budget)?;
    Ok(())
}

pub(super) fn write(
    body: &Dispatcher,
    type_names: &crate::p8_java::types::TypeNames,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    locals(body, type_names, output, budget)?;
    output.push("int pc = 0;\nwhile (true) {\nswitch (pc) {\n", budget)?;
    for case in &body.cases {
        budget.charge_work(1)?;
        output.push("case ", budget)?;
        output.unsigned(u64::from(case.key.0), budget)?;
        output.push(": {\n", budget)?;
        statements(case, output, budget)?;
        match &case.transfer {
            Transfer::ReturnVoid => output.push("return;\n", budget)?,
            Transfer::Switch {
                form,
                value,
                cases,
                default,
            } => {
                if *form == crate::p7_regions::SwitchForm::Comparisons {
                    for (key, target) in cases {
                        budget.charge_work(1)?;
                        output.push("if (", budget)?;
                        local(*value, output, budget)?;
                        output.push(" == ", budget)?;
                        output.arguments(format_args!("{key}"), budget)?;
                        output.push(") {\n", budget)?;
                        edge(target, output, budget)?;
                        output.push("}\n", budget)?;
                    }
                    edge(default, output, budget)?;
                } else {
                    output.push("switch (", budget)?;
                    local(*value, output, budget)?;
                    output.push(") {\n", budget)?;
                    for (key, target) in cases {
                        budget.charge_work(1)?;
                        output.push("case ", budget)?;
                        output.arguments(format_args!("{key}"), budget)?;
                        output.push(": {\n", budget)?;
                        edge(target, output, budget)?;
                        output.push("}\n", budget)?;
                    }
                    output.push("default: {\n", budget)?;
                    edge(default, output, budget)?;
                    output.push("}\n}\n", budget)?;
                }
            }
            Transfer::Return(value) => super::java::returned(*value, output, budget)?,
            Transfer::Jump(target) => edge(target, output, budget)?,
            Transfer::Branch { yes, no, .. } => {
                open_if(&case.transfer, output, budget)?;
                edge(yes, output, budget)?;
                output.push("} else {\n", budget)?;
                edge(no, output, budget)?;
                output.push("}\n", budget)?;
            }
        }
        output.push("}\n", budget)?;
    }
    output.push("default: throw new AssertionError();\n}\n}\n", budget)
}
