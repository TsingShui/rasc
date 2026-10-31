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
pub(super) fn write(
    body: &Dispatcher,
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
                Kind::Float => "float ",
            },
            budget,
        )?;
        local(declaration.id, output, budget)?;
        if let Some(parameter) = declaration.parameter {
            output.push(" = p", budget)?;
            output.unsigned(u64::from(parameter), budget)?;
        } else {
            // The SSA/edge definition certificate establishes assignment before
            // every original read; these DA-only values are never observed.
            output.push(
                match declaration.kind {
                    Kind::Int => " = 0",
                    Kind::Boolean => " = false",
                    Kind::Object => " = null",
                    Kind::Float => " = 0.0f",
                },
                budget,
            )?;
        }
        output.push(";\n", budget)?;
    }
    assignments(&body.entry, output, budget)?;
    output.push("int pc = 0;\nwhile (true) {\nswitch (pc) {\n", budget)?;
    for case in &body.cases {
        budget.charge_work(1)?;
        output.push("case ", budget)?;
        output.unsigned(u64::from(case.key.0), budget)?;
        output.push(": {\n", budget)?;
        assignments(&case.statements, output, budget)?;
        match &case.transfer {
            Transfer::Switch {
                value,
                cases,
                default,
            } => {
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
            Transfer::Return(value) => super::java::returned(*value, output, budget)?,
            Transfer::Jump(target) => edge(target, output, budget)?,
            Transfer::Branch {
                left,
                right,
                kind,
                opcode,
                yes,
                no,
            } => {
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
                        output.push(if *kind == Kind::Object { "null" } else { "0" }, budget)?;
                    }
                }
                output.push(") {\n", budget)?;
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
