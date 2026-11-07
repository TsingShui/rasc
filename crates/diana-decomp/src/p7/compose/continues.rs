//! A trailing continue states the loop would do anyway.

use super::super::*;
use super::*;

/// A continue that ends the loop body means the same as reaching the body end:
/// the predicate runs next. Copies attached to the transfer keep it explicit.
pub(super) fn drop_trailing_continues(code: &mut MethodCode) -> Result<(), ExpressionError> {
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
    let mut bodies: Vec<Option<ControlId>> = filled(code.controls.len(), None)?;
    for control in &code.controls {
        if let ControlKind::Loop { .. } = control.kind {
            bodies[control.id.0 as usize] = control.children.first().copied();
        }
    }
    for transfer in 0..code.transfers.len() {
        let TransferSpelling::ContinueHere(target) = code.emission.transfers[transfer].spelling
        else {
            continue;
        };
        let Some(body) = bodies.get(target.0 as usize).copied().flatten() else {
            continue;
        };
        let from = code.transfers[transfer].from;
        let Some(start) = owner.get(from.0 as usize).copied().flatten() else {
            continue;
        };
        let mut current = start;
        let mut tail = false;
        for _ in 0..=code.controls.len() {
            if current == body {
                tail = true;
                break;
            }
            let Some(up) = parent.get(current.0 as usize).copied().flatten() else {
                break;
            };
            let Some(control) = code.controls.get(up.0 as usize) else {
                break;
            };
            let emitted = &code.emission.controls[up.0 as usize];
            if code.emission.controls[current.0 as usize]
                .continuation
                .is_some()
                || !matches!(emitted.layout, EmissionLayout::Sequence)
                || emitted.children(control).last() != Some(&current)
            {
                break;
            }
            current = up;
        }
        if tail {
            code.emission.transfers[transfer].spelling = TransferSpelling::Fallthrough;
        }
    }
    Ok(())
}
