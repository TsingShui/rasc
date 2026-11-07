//! How the selected layout prints one control after another.

use super::super::*;

/// Follow only layouts that print their established entry before any control
/// action. Entering a loop, protected scope or graph can have different effects
/// from jumping to a block inside it, so those boundaries remain explicit.
pub(super) fn printed_entry_is(code: &MethodCode, mut current: ControlId, target: LabelId) -> bool {
    for _ in 0..code.controls.len() {
        let Some(control) = code.controls.get(current.0 as usize) else {
            return false;
        };
        let Some(emission) = code.emission.controls.get(current.0 as usize) else {
            return false;
        };
        if let EmissionLayout::Block(node) = emission.layout {
            return node.0 == target.0;
        }
        if emission.omitted || control.entry.is_none_or(|entry| entry.0 != target.0) {
            return false;
        }
        current = match &emission.layout {
            EmissionLayout::Sequence => {
                let Some(&first) = emission.children(control).first() else {
                    return false;
                };
                first
            }
            EmissionLayout::If { header, .. }
            | EmissionLayout::Condition { header, .. }
            | EmissionLayout::Switch { header, .. } => *header,
            EmissionLayout::Block(_)
            | EmissionLayout::Labelled
            | EmissionLayout::Graph
            | EmissionLayout::When { .. }
            | EmissionLayout::Loop { .. }
            | EmissionLayout::Try { .. }
            | EmissionLayout::Synchronized { .. } => return false,
        };
    }
    false
}

/// The caller reserves space for all controls (at least three slots). Layout
/// roles, not containment order, determine which children P9 prints.
pub(super) fn printed_parts<'a>(
    code: &'a MethodCode,
    control: ControlId,
    buffer: &'a mut [ControlId],
) -> &'a [ControlId] {
    let parts: &[ControlId] = match &code.emission.controls[control.0 as usize].layout {
        EmissionLayout::Block(_) => return &[],
        EmissionLayout::Sequence | EmissionLayout::Labelled | EmissionLayout::Graph => {
            return code.emission.controls[control.0 as usize]
                .children(&code.controls[control.0 as usize]);
        }
        EmissionLayout::If {
            header, yes, no, ..
        } => {
            buffer[..3].copy_from_slice(&[*header, *yes, *no]);
            return &buffer[..if code.emission.omitted(*no) { 2 } else { 3 }];
        }
        EmissionLayout::Condition {
            header, yes, no, ..
        } => {
            buffer[..2].copy_from_slice(&[*header, *yes]);
            if let ConditionElse::Body(no) = no {
                buffer[2] = *no;
                return &buffer[..3];
            }
            return &buffer[..2];
        }
        EmissionLayout::When { bodies } => bodies,
        EmissionLayout::Loop { body, .. } => std::slice::from_ref(body),
        EmissionLayout::Switch { header, bodies, .. } => {
            buffer[0] = *header;
            buffer[1..=bodies.len()].copy_from_slice(bodies);
            return &buffer[..=bodies.len()];
        }
        EmissionLayout::Try { body, catches } => {
            buffer[0] = *body;
            buffer[1..=catches.len()].copy_from_slice(catches);
            return &buffer[..=catches.len()];
        }
        EmissionLayout::Synchronized { entry, body, .. } => &[*entry, *body],
    };
    buffer[..parts.len()].copy_from_slice(parts);
    &buffer[..parts.len()]
}

pub(super) fn printed_successor(
    code: &MethodCode,
    parent: &[Option<ControlId>],
    start: ControlId,
    buffer: &mut [ControlId],
) -> Option<ControlId> {
    let mut current = start;
    for _ in 0..=code.controls.len() {
        let up = parent.get(current.0 as usize).copied().flatten()?;
        let layout = &code.emission.controls.get(up.0 as usize)?.layout;
        if !matches!(
            layout,
            EmissionLayout::Sequence
                | EmissionLayout::Labelled
                | EmissionLayout::Graph
                | EmissionLayout::If { .. }
        ) {
            return None;
        }
        let parts = printed_parts(code, up, buffer);
        let index = parts.iter().position(|&part| part == current)?;
        if index + 1 < parts.len() {
            // Branches are alternatives, not sequential successors. Only the
            // last printed branch may fall through the enclosing If's end.
            return if matches!(layout, EmissionLayout::If { .. }) {
                None
            } else {
                Some(parts[index + 1])
            };
        }
        if code.emission.controls[up.0 as usize].continuation.is_some() {
            return None;
        }
        current = up;
    }
    None
}

pub(super) fn contains(parent: &[Option<ControlId>], start: ControlId, target: ControlId) -> bool {
    let mut current = start;
    for _ in 0..=parent.len() {
        if current == target {
            return true;
        }
        match parent.get(current.0 as usize).copied().flatten() {
            Some(next) => current = next,
            None => return false,
        }
    }
    false
}
