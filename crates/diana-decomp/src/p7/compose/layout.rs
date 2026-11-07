//! Sequencing, scope and continuation choices over the selected layout.

use super::super::*;
use super::*;

/// A jump to the statement printed immediately after it is ordinary sequencing.
/// Only containers that print their children in order and add nothing after them
/// may pass the search upward; a loop body does not fall through its own test.
pub(super) fn collapse_fallthrough_gotos(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut parent: Vec<Option<ControlId>> = filled(code.controls.len(), None)?;
    let mut buffer = filled(code.controls.len().max(3), ControlId(0))?;
    for control in &code.controls {
        for &child in &control.children {
            let slot = parent
                .get_mut(child.0 as usize)
                .ok_or(bug("control names an unknown child"))?;
            if slot.replace(control.id).is_some() {
                return Err(bug("control has two containment owners"));
            }
        }
    }
    for transfer in 0..code.emission.transfers.len() {
        let TransferSpelling::Goto(label) = code.emission.transfers[transfer].spelling else {
            continue;
        };
        let from = code.transfers[transfer].from;
        let Some(owner) = code.labels.get(from.0 as usize).map(|label| label.owner) else {
            continue;
        };
        let Some(next) = printed_successor(code, &parent, owner, &mut buffer) else {
            continue;
        };
        if printed_entry_is(code, next, label) {
            code.emission.transfers[transfer].spelling = TransferSpelling::Fallthrough;
        }
    }
    Ok(())
}

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

/// A control's planned continuation repeats the jump its last printed block
/// already performs. Two adjacent jumps to one label are redundant, so the
/// control stops emitting the second one.
pub(super) fn drop_duplicate_continuation(code: &mut MethodCode) -> Result<(), ExpressionError> {
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
    // Every block whose printed end is a jump to one label.
    let mut jumps = filled(code.blocks.len(), None)?;
    for (node, block) in code.emission.blocks.iter().enumerate() {
        let transfer = match &block.end {
            BlockEnd::Terminal(terminal) => match code.terminals[terminal.0 as usize].kind {
                TerminalKind::Goto(transfer) => Some(transfer),
                _ => None,
            },
            BlockEnd::Transfers(transfers) => transfers.last().copied(),
            BlockEnd::None => None,
        };
        let Some(transfer) = transfer else {
            continue;
        };
        if let TransferSpelling::Goto(label) = code.emission.transfers[transfer.0 as usize].spelling
        {
            jumps[node] = Some(label);
        }
    }
    for control in 0..code.emission.controls.len() {
        let Some(ControlCompletion::Goto(continuation)) =
            code.emission.controls[control].continuation
        else {
            continue;
        };
        for (node, jump) in jumps.iter().enumerate() {
            if *jump != Some(continuation) {
                continue;
            }
            let Some(mut current) = owner.get(node).copied().flatten() else {
                continue;
            };
            let mut last = false;
            for _ in 0..=code.controls.len() {
                if current.0 as usize == control {
                    last = true;
                    break;
                }
                let Some(up) = parent.get(current.0 as usize).copied().flatten() else {
                    break;
                };
                let Some(parent_control) = code.controls.get(up.0 as usize) else {
                    break;
                };
                let emitted = &code.emission.controls[up.0 as usize];
                if code.emission.controls[current.0 as usize]
                    .continuation
                    .is_some()
                    || !matches!(emitted.layout, EmissionLayout::Sequence)
                    || emitted.children(parent_control).last() != Some(&current)
                {
                    break;
                }
                current = up;
            }
            if last {
                code.emission.controls[control].continuation = None;
                break;
            }
        }
    }
    Ok(())
}

pub(super) fn collapse_planned_continuations(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut parent = filled(code.controls.len(), None)?;
    let mut buffer = filled(code.controls.len().max(3), ControlId(0))?;
    for control in &code.controls {
        for &child in &control.children {
            *parent
                .get_mut(child.0 as usize)
                .ok_or(bug("control names an unknown child"))? = Some(control.id);
        }
    }
    for index in 0..code.emission.controls.len() {
        let Some(ControlCompletion::Goto(label)) = code.emission.controls[index].continuation
        else {
            continue;
        };
        let control = ControlId(super::build::id(index)?);
        let Some(next) = printed_successor(code, &parent, control, &mut buffer) else {
            continue;
        };
        if printed_entry_is(code, next, label) {
            code.emission.controls[index].continuation = None;
        }
    }
    Ok(())
}

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
            if let Some(no) = no {
                buffer[2] = *no;
            }
            return &buffer[..if no.is_some() { 3 } else { 2 }];
        }
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
