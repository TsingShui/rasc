//! A jump to the statement printed right after it is ordinary sequencing.

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
        let Some(next) = super::sequence::printed_successor(code, &parent, owner, &mut buffer)
        else {
            continue;
        };
        if super::sequence::printed_entry_is(code, next, label) {
            code.emission.transfers[transfer].spelling = TransferSpelling::Fallthrough;
        }
    }
    Ok(())
}
