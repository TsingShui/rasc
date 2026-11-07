//! An allocation publishes its type and local where it happens.

use super::super::*;
use super::*;

/// A new-instance statement that publishes its own storage states the allocation
/// at that position: the object exists and its constructor has not run. Printing
/// it as `T local` keeps the original allocation point; the constructor call keeps
/// its own statement and position. Only a directly published allocation
/// qualifies, so a discarded or aliased allocation keeps its expression form.
pub(super) fn allocation_declarations(code: &mut MethodCode) -> Result<(), ExpressionError> {
    for index in 0..code.statements.len() {
        let statement = &code.statements[index];
        let StatementKind::Evaluate { expression } = statement.kind else {
            continue;
        };
        if !matches!(
            code.expressions.get(expression.0 as usize).map(|e| &e.kind),
            Some(ExprKind::AllocateInstance { .. })
        ) {
            continue;
        }
        let [output] = statement.outputs.as_slice() else {
            continue;
        };
        let local = match output.materialization() {
            crate::p5::Materialization::Store(local)
            | crate::p5::Materialization::AllocationCarrier(local) => local,
            _ => continue,
        };
        *code
            .emission
            .allocation_declarations
            .get_mut(index)
            .ok_or(bug("allocation names an unknown statement"))? = true;
        *code
            .emission
            .allocation_locals
            .get_mut(local.0 as usize)
            .ok_or(bug("allocation names an unknown local"))? = true;
    }
    Ok(())
}
