//! A scope that adds braces but no fact prints without a name.

use super::super::*;
use super::*;

/// Marks a label-free `Labelled` scope transparent when its only printed child
/// is one unnamed control. Such a scope contributes braces but no fact: the
/// original control, its labels and every transfer target stay in the product.
pub(super) fn transparent_labelled_scopes(code: &mut MethodCode) -> Result<(), ExpressionError> {
    for index in 0..code.emission.controls.len() {
        let control = code
            .controls
            .get(index)
            .ok_or(bug("emission control has no source control"))?;
        let plan = code
            .emission
            .controls
            .get(index)
            .ok_or(bug("emission control is missing"))?;
        let transparent = matches!(plan.layout, EmissionLayout::Labelled)
            && !plan.label_required
            && plan.continuation.is_none()
            && !code.emission.deferred_control(control.id)
            && match control.children.as_slice() {
                [child] => code
                    .emission
                    .controls
                    .get(child.0 as usize)
                    .is_some_and(|child| !child.omitted && !child.label_required),
                _ => false,
            };
        let plan = code
            .emission
            .controls
            .get_mut(index)
            .ok_or(bug("emission control is missing"))?;
        plan.transparent = transparent;
    }
    Ok(())
}
