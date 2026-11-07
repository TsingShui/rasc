//! Field and array stores print in their natural form only when the order holds.

use super::super::*;

/// Java evaluates the array reference, then its index, then the value. A DEX
/// store only reads registers, so printing the value last states an order the
/// original instruction did not. The array, index and receiver slots are never
/// composed, so they stay plain local reads or literals; reading them before the
/// value cannot be observed, and the store's null/bounds check still follows the
/// value in both forms. Only a non-plain array, index or receiver — an inlined
/// expression or an unavailable operand — keeps the explicit store form.
pub(super) fn natural_stores(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut natural = Vec::new();
    crate::substrate::owned::reserve(&mut natural, code.statements.len())?;
    natural.resize(code.statements.len(), false);
    for statement in &code.statements {
        let plain = |operand: ExprId| {
            code.emission.composed_use(operand).is_none()
                && matches!(
                    code.expressions.get(operand.0 as usize).map(|e| &e.kind),
                    Some(
                        ExprKind::Read { .. }
                            | ExprKind::Snapshot { .. }
                            | ExprKind::Literal { .. }
                    )
                )
        };
        natural[statement.id.0 as usize] = match &statement.kind {
            StatementKind::FieldStore { receiver, .. } => receiver.is_none_or(plain),
            StatementKind::ArrayStore { array, index, .. } => plain(*array) && plain(*index),
            _ => continue,
        };
    }
    code.emission.natural_stores = natural;
    Ok(())
}
