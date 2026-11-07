//! Which labels and scopes still need a printed name.

use super::super::*;
use super::*;
use crate::p2::BlockId;

pub(super) fn mark_label(labels: &mut [bool], label: LabelId) -> Result<(), ExpressionError> {
    *labels
        .get_mut(label.0 as usize)
        .ok_or(bug("emission names an unknown label"))? = true;
    Ok(())
}

pub(super) fn mark_selected_target(
    code: &MethodCode,
    labels: &mut [bool],
    edge: TransferId,
) -> Result<(), ExpressionError> {
    let original = code
        .transfers
        .get(edge.0 as usize)
        .ok_or(bug("terminal names an unknown transfer"))?;
    let planned = code
        .emission
        .transfers
        .get(edge.0 as usize)
        .ok_or(bug("terminal transfer lost its emission"))?;
    if matches!(planned.spelling, TransferSpelling::Selected) {
        mark_label(labels, original.destination)?;
    }
    Ok(())
}

pub(super) fn mark_terminal_targets(
    code: &MethodCode,
    labels: &mut [bool],
    id: BlockId,
) -> Result<(), ExpressionError> {
    let terminal = code
        .terminals
        .get(id.0 as usize)
        .ok_or(bug("block lost its terminal"))?;
    match &terminal.kind {
        TerminalKind::If {
            when_true,
            when_false,
            ..
        } => {
            mark_selected_target(code, labels, *when_true)?;
            mark_selected_target(code, labels, *when_false)?;
        }
        TerminalKind::Switch { arms, .. } => {
            for &arm in arms {
                mark_selected_target(code, labels, arm)?;
            }
        }
        _ => {}
    }
    Ok(())
}
