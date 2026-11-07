//! A scope prints a name only when a transfer still needs it.

use super::super::*;
use super::*;

/// A break or continue whose target is already the innermost enclosing scope
/// needs no label. The printed nesting then matches the original target.
pub(super) fn unlabel_inner_scopes(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut parent: Vec<Option<ControlId>> = filled(code.controls.len(), None)?;
    for control in &code.controls {
        for &child in &control.children {
            *parent
                .get_mut(child.0 as usize)
                .ok_or(bug("control names an unknown child"))? = Some(control.id);
        }
    }
    let mut owner: Vec<Option<ControlId>> = filled(code.labels.len(), None)?;
    for label in &code.labels {
        *owner
            .get_mut(label.node.0 as usize)
            .ok_or(bug("label names an unknown node"))? = Some(label.owner);
    }
    for transfer in 0..code.transfers.len() {
        let (target, looping, spelling) = match code.emission.transfers[transfer].spelling {
            TransferSpelling::Break(target) => (target, false, TransferSpelling::BreakHere(target)),
            TransferSpelling::Continue(target) => {
                (target, true, TransferSpelling::ContinueHere(target))
            }
            _ => continue,
        };
        let from = code.transfers[transfer].from;
        let Some(start) = owner.get(from.0 as usize).copied().flatten() else {
            continue;
        };
        if innermost_scope(code, &parent, start, target, looping) {
            code.emission.transfers[transfer].spelling = spelling;
        }
    }
    for index in 0..code.emission.controls.len() {
        let (target, looping, spelling) = match code.emission.controls[index].continuation {
            Some(ControlCompletion::Break(target)) => {
                (target, false, ControlCompletion::BreakHere(target))
            }
            Some(ControlCompletion::Continue(target)) => {
                (target, true, ControlCompletion::ContinueHere(target))
            }
            _ => continue,
        };
        // Completion executes after this control, in its parent's scope.
        let Some(start) = parent[index] else {
            continue;
        };
        if innermost_scope(code, &parent, start, target, looping) {
            code.emission.controls[index].continuation = Some(spelling);
        }
    }
    Ok(())
}

pub(super) fn innermost_scope(
    code: &MethodCode,
    parent: &[Option<ControlId>],
    mut current: ControlId,
    target: ControlId,
    looping: bool,
) -> bool {
    for _ in 0..=code.controls.len() {
        if current == target {
            return true;
        }
        if matches!(
            code.controls[current.0 as usize].kind,
            ControlKind::Loop { .. }
        ) || (!looping
            && matches!(
                code.controls[current.0 as usize].kind,
                ControlKind::Switch { .. }
            ))
        {
            return false;
        }
        let Some(next) = parent[current.0 as usize] else {
            return false;
        };
        current = next;
    }
    false
}
