use crate::p2_flow::BlockId;
use crate::p7_java::dispatcher::{Case, Dispatcher, Transfer};
use crate::p7_java::source_control::{Label, Plan, Step};
use crate::substrate::owned::TextBuffer;
use crate::{RequestError, RunContext};

fn case(storage: &Dispatcher, id: BlockId) -> Result<&Case, RequestError> {
    storage
        .cases
        .get(usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?)
        .filter(|case| case.key == id)
        .ok_or(RequestError::InternalInvariant("source control lost block"))
}
pub(super) fn write(
    storage: &Dispatcher,
    plan: &Plan,
    names: &crate::p7_java::types::TypeNames,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    super::dispatcher::locals(storage, names, output, budget)?;
    for step in plan.steps() {
        budget.checkpoint()?;
        match step {
            Step::Statements(id) => {
                super::dispatcher::statements(case(storage, *id)?, output, budget)?
            }
            Step::Copies { source, edge } => {
                let transfer = &case(storage, *source)?.transfer;
                let original = match transfer {
                    Transfer::Jump(e) if e.id == *edge => e,
                    Transfer::Branch { yes, .. } if yes.id == *edge => yes,
                    Transfer::Branch { no, .. } if no.id == *edge => no,
                    _ => {
                        return Err(RequestError::InternalInvariant(
                            "source control lost CopyPlan",
                        ));
                    }
                };
                super::java::assignments(&original.copies, output, budget)?;
            }
            Step::If(id) => {
                super::dispatcher::open_if(&case(storage, *id)?.transfer, output, budget)?
            }
            Step::Else => output.push("} else {\n", budget)?,
            Step::End => output.push("}\n", budget)?,
            Step::Loop(Label::Loop0) => output.push("loop0: while (true) {\n", budget)?,
            Step::Loop(Label::Nearest) => output.push("while (true) {\n", budget)?,
            Step::Break(Label::Loop0) => output.push("break loop0;\n", budget)?,
            Step::Break(Label::Nearest) => output.push("break;\n", budget)?,
            Step::Continue(Label::Loop0) => output.push("continue loop0;\n", budget)?,
            Step::Continue(Label::Nearest) => output.push("continue;\n", budget)?,
            Step::Return(value) => super::java::returned(*value, output, budget)?,
            Step::ReturnVoid => output.push("return;\n", budget)?,
            Step::InitializerExit => output.push("break initExit;\n", budget)?,
            Step::Throw(value) => {
                output.push("throw ", budget)?;
                super::java::value_use(*value, output, budget)?;
                output.push(";\n", budget)?;
            }
            Step::Rethrow(handler) => {
                output.push("throw caught", budget)?;
                output.unsigned(u64::from(handler.0), budget)?;
                output.push(";\n", budget)?;
            }
            Step::Synchronized(lock) => {
                output.push("synchronized (", budget)?;
                super::java::value_use(*lock, output, budget)?;
                output.push(") {\n", budget)?;
            }
            Step::Try(_) => output.push("try {\n", budget)?,
            Step::CloseTryBody => output.push("}", budget)?,
            Step::EndTry => {}
            Step::Catch {
                alternatives,
                handler,
            } => {
                output.push(" catch (", budget)?;
                for (index, name) in alternatives.iter().enumerate() {
                    budget.checkpoint()?;
                    if index != 0 {
                        output.push(" | ", budget)?;
                    }
                    output.push(name, budget)?;
                }
                output.push(" caught", budget)?;
                output.unsigned(u64::from(handler.0), budget)?;
                output.push(") {\n", budget)?;
            }
        }
    }
    Ok(())
}
