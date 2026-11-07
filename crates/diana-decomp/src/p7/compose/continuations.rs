//! Planned continuations that the layout already performs print nothing.

use super::super::*;
use super::*;

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
        let Some(next) = super::sequence::printed_successor(code, &parent, control, &mut buffer)
        else {
            continue;
        };
        if super::sequence::printed_entry_is(code, next, label) {
            code.emission.controls[index].continuation = None;
        }
    }
    Ok(())
}
