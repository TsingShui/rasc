//! Composes the established emission layout without reconstructing control flow.
//! The input is the same method's closed P7 product; no predecessor is reopened.

mod expressions;

use super::build::{bug, filled};
use super::*;
use crate::p2::BlockId;
use crate::p5::ThrowSite;

/// Retains only textually referenced labels and selects an ordinary loop test
/// when its selected predicate node has no work other than its condition.
pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    // Sequencing is fixed before labels are selected, so a collapsed jump no
    // longer keeps a display label alive.
    collapse_fallthrough_gotos(code)?;
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
    code.emission.site_annotations = site_annotations(code)?;
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

/// A jump to the statement printed immediately after it is ordinary sequencing.
/// Only containers that print their children in order and add nothing after them
/// may pass the search upward; a loop body does not fall through its own test.
fn collapse_fallthrough_gotos(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut parent: Vec<Option<ControlId>> = filled(code.controls.len(), None)?;
    let mut sibling: Vec<Option<ControlId>> = filled(code.controls.len(), None)?;
    for control in &code.controls {
        for &child in &control.children {
            let slot = parent
                .get_mut(child.0 as usize)
                .ok_or(bug("control names an unknown child"))?;
            if slot.replace(control.id).is_some() {
                return Err(bug("control has two containment owners"));
            }
        }
        if matches!(control.kind, ControlKind::Sequence) {
            for pair in control.children.windows(2) {
                let slot = sibling
                    .get_mut(pair[0].0 as usize)
                    .ok_or(bug("sequence names an unknown child"))?;
                if slot.replace(pair[1]).is_some() {
                    return Err(bug("sequence child has two print successors"));
                }
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
        let Some(next) = printed_successor(code, &parent, &sibling, owner) else {
            continue;
        };
        if matches!(
            code.emission.controls[next.0 as usize].layout,
            EmissionLayout::Block(node) if node.0 == label.0
        ) {
            code.emission.transfers[transfer].spelling = TransferSpelling::Fallthrough;
        }
    }
    Ok(())
}
fn printed_successor(
    code: &MethodCode,
    parent: &[Option<ControlId>],
    sibling: &[Option<ControlId>],
    start: ControlId,
) -> Option<ControlId> {
    let mut current = start;
    for _ in 0..=code.controls.len() {
        if let Some(next) = sibling.get(current.0 as usize).copied().flatten() {
            return Some(next);
        }
        let parent = parent.get(current.0 as usize).copied().flatten()?;
        if !matches!(
            code.controls.get(parent.0 as usize)?.kind,
            ControlKind::Sequence
                | ControlKind::LabelledBlock { .. }
                | ControlKind::Graph { .. }
                | ControlKind::Synchronized { .. }
        ) {
            return None;
        }
        // A container with its own continuation prints that jump before the
        // enclosing successor, so control does not fall into it.
        if code.emission.controls[parent.0 as usize]
            .continuation
            .is_some()
        {
            return None;
        }
        current = parent;
    }
    None
}
/// A natural route already printed as the enclosing try/catch needs no repeated
/// annotation. Explicit protection, several targets and unplaced routes keep one.
fn site_annotations(code: &MethodCode) -> Result<Vec<bool>, ExpressionError> {
    let mut parent: Vec<Option<ControlId>> = filled(code.controls.len(), None)?;
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
    let mut owner: Vec<Option<ControlId>> = filled(code.labels.len(), None)?;
    for label in &code.labels {
        *owner
            .get_mut(label.node.0 as usize)
            .ok_or(bug("label names an unknown node"))? = Some(label.owner);
    }
    let mut node_of_block: Vec<Option<u32>> = filled(code.terminals.len(), None)?;
    for (node, block) in code.emission.blocks.iter().enumerate() {
        if let BlockEnd::Terminal(terminal) = block.end {
            *node_of_block
                .get_mut(terminal.0 as usize)
                .ok_or(bug("block end names an unknown terminal"))? =
                Some(u32::try_from(node).map_err(|_| crate::RequestError::PlatformCapacity)?);
        }
    }
    let mut required = filled(code.sites.len(), true)?;
    for site in &code.sites {
        if matches!(
            site.syntax,
            ExceptionSyntax::Propagate | ExceptionSyntax::ImpossibleRelease
        ) {
            required[site.id.0 as usize] = false;
            continue;
        }
        if !matches!(site.syntax, ExceptionSyntax::Natural) {
            continue;
        }
        let [route] = site.routes.as_slice() else {
            continue;
        };
        let Delivery::Catch(target) = route.delivery else {
            continue;
        };
        let node = match site.original {
            ThrowSite::Operation(op) => code.operations.get(op.0 as usize).map(|o| o.node),
            ThrowSite::Terminal(block) => node_of_block
                .get(block.0 as usize)
                .copied()
                .flatten()
                .map(crate::p6::NodeId),
        };
        let Some(start) = node.and_then(|node| owner.get(node.0 as usize).copied().flatten())
        else {
            continue;
        };
        if contains(&parent, start, target) {
            required[site.id.0 as usize] = false;
        }
    }
    Ok(required)
}
fn contains(parent: &[Option<ControlId>], start: ControlId, target: ControlId) -> bool {
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
