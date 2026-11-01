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
    if let Some(entry) = body.prologue.first() {
        output.push(
            match entry.kind {
                crate::p5_types::initialization::ChainKind::This => "this(",
                crate::p5_types::initialization::ChainKind::Super => "super(",
            },
            budget,
        )?;
        for (index, argument) in entry.arguments.iter().enumerate() {
            budget.charge_work(1)?;
            if index != 0 {
                output.push(", ", budget)?;
            }
            super::java::chain_argument(argument, output, budget)?;
        }
        output.push(");\n", budget)?;
    }
    for declaration in &body.locals {
        budget.charge_work(1)?;
        output.push(type_names.name(declaration.kind, budget)?, budget)?;
        output.push(" ", budget)?;
        local(declaration.id, output, budget)?;
        if declaration.initializer == crate::p6_variables::normal::Initializer::ThisAfterChain {
            output.push(" = this", budget)?;
        } else if let crate::p6_variables::normal::Initializer::Parameter(parameter) =
            declaration.initializer
        {
            match parameter {
                crate::p6_variables::normal::Parameter::This => output.push(" = this", budget)?,
                crate::p6_variables::normal::Parameter::Explicit(index) => {
                    output.push(" = p", budget)?;
                    output.unsigned(u64::from(index), budget)?;
                }
            }
        } else if declaration.initializer == crate::p6_variables::normal::Initializer::Default {
            // The SSA/edge definition certificate establishes assignment before
            // every original read; these DA-only values are never observed.
            output.push(
                match declaration.kind {
                    Kind::Int => " = 0",
                    Kind::Boolean => " = false",
                    Kind::Object
                    | Kind::Throwable
                    | Kind::Class
                    | Kind::String
                    | Kind::PrimitiveArray(_)
                    | Kind::ObjectArray
                    | Kind::Nominal(_)
                    | Kind::ReferenceArray(_)
                    | Kind::ProjectedNominal(_) => " = null",
                    Kind::Float => " = 0.0f",
                    Kind::Long => " = 0L",
                    Kind::Double => " = 0.0d",
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
            crate::p8_java::calls::Statement::Snapshot(snapshot) => {
                local(snapshot.copy.target, output, budget)?;
                output.push(" = ", budget)?;
                local(snapshot.copy.source, output, budget)?;
                output.push(";\n", budget)?;
            }
            crate::p8_java::calls::Statement::ExceptionBind { handler, target } => {
                local(*target, output, budget)?;
                output.push(" = caught", budget)?;
                output.unsigned(u64::from(handler.0), budget)?;
                output.push(";\n", budget)?;
            }
            crate::p8_java::calls::Statement::Monitor { .. } => {}
            crate::p8_java::calls::Statement::Class { target, source, .. } => {
                local(*target, output, budget)?;
                output.push(" = ", budget)?;
                output.push(source, budget)?;
                output.push(".class;\n", budget)?;
            }
            crate::p8_java::calls::Statement::Deferred(_)
            | crate::p8_java::calls::Statement::ConstructorChain(_) => {}
            crate::p8_java::calls::Statement::Initialize(init) => {
                local(init.root, output, budget)?;
                output.push(" = new ", budget)?;
                output.push(&init.source_type, budget)?;
                output.push("(", budget)?;
                for (index, &argument) in init.arguments.iter().enumerate() {
                    budget.charge_work(1)?;
                    if index != 0 {
                        output.push(", ", budget)?;
                    }
                    super::java::value_use(argument, output, budget)?;
                }
                output.push(");\n", budget)?;
                for &alias in &init.aliases {
                    budget.charge_work(1)?;
                    if alias != init.root {
                        local(alias, output, budget)?;
                        output.push(" = ", budget)?;
                        local(init.root, output, budget)?;
                        output.push(";\n", budget)?;
                    }
                }
            }
            crate::p8_java::calls::Statement::NewArray(array) => {
                local(array.result, output, budget)?;
                output.push(" = new ", budget)?;
                output.push(&array.base_type, budget)?;
                match &array.contents {
                    crate::p8_java::fresh_fill::Contents::Length(length) => {
                        output.push("[", budget)?;
                        super::java::value_use(*length, output, budget)?;
                        output.push("]", budget)?;
                        for _ in 0..array.trailing_dimensions {
                            output.push("[]", budget)?;
                        }
                        output.push(";\n", budget)?;
                    }
                    crate::p8_java::fresh_fill::Contents::Elements(values) => {
                        output.push("[]", budget)?;
                        for _ in 0..array.trailing_dimensions {
                            output.push("[]", budget)?;
                        }
                        output.push(" {", budget)?;
                        for (index, &value) in values.iter().enumerate() {
                            budget.charge_work(1)?;
                            if index != 0 {
                                output.push(", ", budget)?;
                            }
                            super::java::value_use(value, output, budget)?;
                        }
                        output.push("};\n", budget)?;
                    }
                }
            }
            crate::p8_java::calls::Statement::FreshFill(fill) => {
                for (index, element) in fill.elements.iter().enumerate() {
                    budget.charge_work(1)?;
                    local(fill.array, output, budget)?;
                    output.push("[", budget)?;
                    output.unsigned(
                        u64::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                        budget,
                    )?;
                    output.push("] = ", budget)?;
                    super::array_literals::element(element, output, budget)?;
                    output.push(";\n", budget)?;
                }
            }
            crate::p8_java::calls::Statement::EmptyFill { array, result, .. } => {
                local(*result, output, budget)?;
                output.push(" = ", budget)?;
                local(*array, output, budget)?;
                output.push(".length;\n", budget)?;
            }
            crate::p8_java::calls::Statement::TypeCheck(check) => {
                local(check.target, output, budget)?;
                output.push(" = ", budget)?;
                match check.operation {
                    crate::p8_java::cast_ops::CheckKind::Cast => {
                        output.push("(", budget)?;
                        output.push(&check.type_name, budget)?;
                        output.push(") ", budget)?;
                        super::java::value_use(check.source, output, budget)?;
                    }
                    crate::p8_java::cast_ops::CheckKind::InstanceOf => {
                        super::java::value_use(check.source, output, budget)?;
                        output.push(" instanceof ", budget)?;
                        output.push(&check.type_name, budget)?;
                    }
                }
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
    use crate::p8_java::conditions::{Comparison, Expression, Right};
    let Transfer::Branch { condition, .. } = transfer else {
        return Err(RequestError::InternalInvariant(
            "structured condition lost source expression",
        ));
    };
    output.push("if (", budget)?;
    match condition.expression() {
        Expression::Boolean { value, negated } => {
            if negated {
                output.push("!", budget)?;
            }
            local(value, output, budget)?;
        }
        Expression::Compare {
            left,
            operator,
            right,
        } => {
            local(left, output, budget)?;
            output.push(
                match operator {
                    Comparison::Equal => " == ",
                    Comparison::NotEqual => " != ",
                    Comparison::Less => " < ",
                    Comparison::GreaterEqual => " >= ",
                    Comparison::Greater => " > ",
                    Comparison::LessEqual => " <= ",
                },
                budget,
            )?;
            match right {
                Right::Local(value) => local(value, output, budget)?,
                Right::Null => output.push("null", budget)?,
                Right::Zero => output.push("0", budget)?,
            }
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
            Transfer::InitializerExit => output.push("break initExit;\n", budget)?,
            Transfer::Throw(_) | Transfer::Rethrow { .. } => {
                super::java::thrown(&case.transfer, output, budget)?
            }
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
