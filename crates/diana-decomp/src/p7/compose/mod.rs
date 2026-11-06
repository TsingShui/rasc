//! Composes the established emission layout without reconstructing control flow.
//! The input is the same method's closed P7 product; no predecessor is reopened.

mod expressions;

use super::build::{bug, filled};
use super::*;
use crate::p2::BlockId;

/// Retains only textually referenced labels and selects an ordinary loop test
/// when its selected predicate node has no work other than its condition.
pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut labels = filled(code.labels.len(), false)?;
    let mut controls = filled(code.controls.len(), false)?;
    let mut chains = filled(code.chains.len(), false)?;
    for chain in &code.chains {
        if chain
            .clauses
            .iter()
            .any(|clause| clause.catch.is_some() || clause.target.is_some())
        {
            *chains
                .get_mut(chain.id.0 as usize)
                .ok_or(bug("exception chain lost its identity"))? = true;
        }
    }
    for site in &code.sites {
        if !matches!(site.syntax, ExceptionSyntax::Propagate) {
            *chains
                .get_mut(site.chain.0 as usize)
                .ok_or(bug("site names an unknown exception chain"))? = true;
            for route in &site.routes {
                if let Delivery::Catch(control) = route.delivery {
                    *controls
                        .get_mut(control.0 as usize)
                        .ok_or(bug("exception route names an unknown control"))? = true;
                }
            }
        }
    }
    code.emission.show_chains = chains;
    for transfer in &code.emission.transfers {
        match transfer.spelling {
            TransferSpelling::Goto(label) => mark_label(&mut labels, label)?,
            TransferSpelling::Break(control) | TransferSpelling::Continue(control) => {
                *controls
                    .get_mut(control.0 as usize)
                    .ok_or(bug("transfer names an unknown control scope"))? = true;
            }
            TransferSpelling::Fallthrough | TransferSpelling::Selected => {}
        }
    }
    for control in &code.controls {
        if let ControlKind::Graph { entries, .. } = &control.kind {
            for &entry in entries {
                mark_label(&mut labels, entry)?;
            }
            if let Some(entry) = control.entry {
                mark_label(&mut labels, entry)?;
            }
        }
        if let ControlKind::Switch { arms, .. } = &control.kind {
            for &edge in arms {
                mark_selected_target(code, &mut labels, edge)?;
            }
        }
    }
    for chain in &code.chains {
        for clause in &chain.clauses {
            if let Some(target) = clause.target {
                mark_label(&mut labels, target)?;
            }
        }
    }
    for control in &code.emission.controls {
        if let Some(label) = control.continuation {
            mark_label(&mut labels, label)?;
        }
    }
    for block in &code.emission.blocks {
        match &block.end {
            BlockEnd::ConstructionFollow { target, .. } => mark_label(&mut labels, *target)?,
            BlockEnd::Terminal(block) => mark_terminal_targets(code, &mut labels, *block)?,
            BlockEnd::None | BlockEnd::Transfers(_) => {}
        }
    }
    for (index, block) in code.emission.blocks.iter_mut().enumerate() {
        block.show_label = *labels.get(index).ok_or(bug("block lost its label"))?;
    }
    for (index, control) in code.emission.controls.iter_mut().enumerate() {
        control.label_required = controls[index];
    }

    // Determine shape before mutating controls, so each decision observes the
    // same completed schedules, not partial results of earlier decisions.
    let mut loop_compact = filled(code.emission.controls.len(), false)?;
    for (index, control) in code.emission.controls.iter().enumerate() {
        let (predicate, test) = match &control.layout {
            EmissionLayout::Loop {
                placement:
                    LoopPlacement::While {
                        predicate, test, ..
                    }
                    | LoopPlacement::DoWhile {
                        predicate, test, ..
                    },
                ..
            } => (*predicate, *test),
            _ => continue,
        };
        let predicate = code
            .emission
            .controls
            .get(predicate.0 as usize)
            .ok_or(bug("loop predicate lost its control"))?;
        let EmissionLayout::Block(node) = predicate.layout else {
            continue;
        };
        let block = code
            .emission
            .blocks
            .get(node.0 as usize)
            .ok_or(bug("loop predicate lost its block"))?;
        let phases = code
            .emission
            .terminals
            .get(test.terminal.0 as usize)
            .ok_or(bug("loop predicate lost its terminal phases"))?;
        loop_compact[index] = !block.show_label
            && block.actions.is_empty()
            && matches!(block.end, BlockEnd::None)
            && predicate.continuation.is_none()
            && phases.before.is_empty()
            && phases.snapshots.is_empty()
            && code.emission.transfers[test.when_true.0 as usize]
                .copies
                .is_empty()
            && code.emission.transfers[test.when_false.0 as usize]
                .copies
                .is_empty();
    }
    for (index, control) in code.emission.controls.iter_mut().enumerate() {
        let test = match &mut control.layout {
            EmissionLayout::If { test, .. } => Some(test),
            EmissionLayout::Loop {
                placement:
                    LoopPlacement::While { test, compact, .. }
                    | LoopPlacement::DoWhile { test, compact, .. },
                ..
            } => {
                *compact = loop_compact[index];
                Some(test)
            }
            _ => None,
        };
        if let Some(test) = test {
            test.edge_copies = !code.emission.transfers[test.when_true.0 as usize]
                .copies
                .is_empty()
                || !code.emission.transfers[test.when_false.0 as usize]
                    .copies
                    .is_empty();
        }
    }
    expressions::construct(code)?;
    Ok(())
}
fn mark_label(labels: &mut [bool], label: LabelId) -> Result<(), ExpressionError> {
    *labels
        .get_mut(label.0 as usize)
        .ok_or(bug("emission names an unknown label"))? = true;
    Ok(())
}
fn mark_selected_target(
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
fn mark_terminal_targets(
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
