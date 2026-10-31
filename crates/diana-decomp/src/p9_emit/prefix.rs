use crate::p8_java::branch::LocalId;
use crate::p8_java::receiver::PrefixStatement;
use crate::substrate::owned::TextBuffer;
use crate::{CodeUnitOffset, MethodBudget, RequestError};
pub(super) enum Scope {
    Before(CodeUnitOffset),
    After {
        chain: CodeUnitOffset,
        returned: CodeUnitOffset,
    },
}
pub(super) fn write(
    statements: &[PrefixStatement],
    scope: Scope,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let mut previous = None;
    for statement in statements {
        budget.charge_work(1)?;
        let pc = match statement {
            PrefixStatement::IntField {
                contract,
                value,
                name,
                pc,
            } => {
                let mut spelling = name.encode_utf16();
                for &unit in &contract.name {
                    budget.charge_work(1)?;
                    if spelling.next() != Some(unit) {
                        return Err(RequestError::InternalInvariant(
                            "field spelling changed original identity",
                        ));
                    }
                }
                if spelling.next().is_some() {
                    return Err(RequestError::InternalInvariant(
                        "field spelling changed original length",
                    ));
                }
                output.push("this.", budget)?;
                output.push(name, budget)?;
                output.push(" = ", budget)?;
                match value {
                    crate::pipeline::effects::IntValue::Parameter(parameter) => {
                        output.push("p", budget)?;
                        output.unsigned(u64::from(*parameter), budget)?;
                    }
                    crate::pipeline::effects::IntValue::Literal(literal) => {
                        output.arguments(format_args!("{literal}"), budget)?
                    }
                }
                output.push(";\n", budget)?;
                *pc
            }
            PrefixStatement::String { value, units, pc } => {
                output.push("java.lang.String ", budget)?;
                super::java::local(LocalId(value.0), output, budget)?;
                output.push(" = ", budget)?;
                super::literals::string(units, output, budget)?;
                output.push(";\n", budget)?;
                *pc
            }
            PrefixStatement::Effect {
                argument,
                qualifier,
                method,
                pc,
                ..
            } => {
                output.push(qualifier, budget)?;
                output.push(".", budget)?;
                output.push(method, budget)?;
                output.push("(", budget)?;
                super::java::local(LocalId(argument.0), output, budget)?;
                output.push(");\n", budget)?;
                *pc
            }
        };
        let bound = match scope {
            Scope::Before(call) => pc.0 < call.0,
            Scope::After { chain, returned } => chain.0 < pc.0 && pc.0 < returned.0,
        };
        if !bound || previous.is_some_and(|prior| pc.0 <= prior) {
            return Err(RequestError::InternalInvariant(
                "owned effect prefix changed lexical order",
            ));
        }
        previous = Some(pc.0);
    }
    Ok(())
}
