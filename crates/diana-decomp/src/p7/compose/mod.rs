//! Composes the established emission layout without reconstructing control flow.
//! The input is the same method's closed P7 product; no predecessor is reopened.

mod expressions;
mod loops;

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
    unlabel_inner_scopes(code)?;
    drop_trailing_continues(code)?;
    drop_duplicate_continuation(code)?;
    collapse_planned_continuations(code)?;
    loops::construct(code)?;
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
    code.emission.field_qualifications = field_qualifications(code)?;
    for transfer in &code.emission.transfers {
        match transfer.spelling {
            TransferSpelling::Goto(label) => mark_label(&mut labels, label)?,
            TransferSpelling::Break(control) | TransferSpelling::Continue(control) => {
                *controls
                    .get_mut(control.0 as usize)
                    .ok_or(bug("transfer names an unknown control scope"))? = true;
            }
            TransferSpelling::BreakHere(_) | TransferSpelling::ContinueHere(_) => {}
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
            && phases.snapshots.is_empty();
    }
    for (index, control) in code.emission.controls.iter_mut().enumerate() {
        if let EmissionLayout::Loop {
            placement: LoopPlacement::While { compact, .. } | LoopPlacement::DoWhile { compact, .. },
            ..
        } = &mut control.layout
        {
            *compact = loop_compact[index];
        }
    }
    expressions::construct(code)?;
    natural_stores(code)?;
    inline_check_casts(code)?;
    omit_empty_branches(code)?;
    Ok(())
}

/// Java evaluates the array reference, then its index, then the value. A DEX
/// store only reads registers, so printing the value last states an order the
/// original instruction did not. The array, index and receiver slots are never
/// composed, so they stay plain local reads or literals; reading them before the
/// value cannot be observed, and the store's null/bounds check still follows the
/// value in both forms. Only a non-plain array, index or receiver — an inlined
/// expression or an unavailable operand — keeps the explicit store form.
fn natural_stores(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut natural = Vec::new();
    crate::substrate::owned::reserve(&mut natural, code.statements.len())?;
    natural.resize(code.statements.len(), false);
    for statement in &code.statements {
        let plain = |operand: ExprId| {
            code.emission.composed_use(operand).is_none()
                && matches!(
                    code.expressions.get(operand.0 as usize).map(|e| &e.kind),
                    Some(
                        ExprKind::Read { .. }
                            | ExprKind::Snapshot { .. }
                            | ExprKind::Literal { .. }
                    )
                )
        };
        natural[statement.id.0 as usize] = match &statement.kind {
            StatementKind::FieldStore { receiver, .. } => receiver.is_none_or(plain),
            StatementKind::ArrayStore { array, index, .. } => plain(*array) && plain(*index),
            _ => continue,
        };
    }
    code.emission.natural_stores = natural;
    Ok(())
}

/// A jump to the statement printed immediately after it is ordinary sequencing.
/// Only containers that print their children in order and add nothing after them
/// may pass the search upward; a loop body does not fall through its own test.
fn collapse_fallthrough_gotos(code: &mut MethodCode) -> Result<(), ExpressionError> {
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
        if matches!(
            code.emission.controls[next.0 as usize].layout,
            EmissionLayout::Block(node) if node.0 == label.0
        ) {
            code.emission.transfers[transfer].spelling = TransferSpelling::Fallthrough;
        }
    }
    Ok(())
}
/// A break or continue whose target is already the innermost enclosing scope
/// needs no label. The printed nesting then matches the original target.
fn unlabel_inner_scopes(code: &mut MethodCode) -> Result<(), ExpressionError> {
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
        let mut current = start;
        for _ in 0..=code.controls.len() {
            if current == target {
                code.emission.transfers[transfer].spelling = spelling;
                break;
            }
            let Some(next) = parent.get(current.0 as usize).copied().flatten() else {
                break;
            };
            // Only the innermost breakable/continuing scope may take the short form.
            if matches!(
                code.controls.get(current.0 as usize).map(|c| &c.kind),
                Some(ControlKind::Loop { .. })
            ) || (!looping
                && matches!(
                    code.controls.get(current.0 as usize).map(|c| &c.kind),
                    Some(ControlKind::Switch { .. })
                ))
            {
                break;
            }
            current = next;
        }
    }
    Ok(())
}
/// A continue that ends the loop body means the same as reaching the body end:
/// the predicate runs next. Copies attached to the transfer keep it explicit.
fn drop_trailing_continues(code: &mut MethodCode) -> Result<(), ExpressionError> {
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
            if !matches!(control.kind, ControlKind::Sequence)
                || control.children.last() != Some(&current)
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
fn drop_duplicate_continuation(code: &mut MethodCode) -> Result<(), ExpressionError> {
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
        let Some(continuation) = code.emission.controls[control].continuation else {
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
                if !matches!(parent_control.kind, ControlKind::Sequence)
                    || parent_control.children.last() != Some(&current)
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
fn collapse_planned_continuations(code: &mut MethodCode) -> Result<(), ExpressionError> {
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
        let Some(label) = code.emission.controls[index].continuation else {
            continue;
        };
        let control = ControlId(super::build::id(index)?);
        let Some(next) = printed_successor(code, &parent, control, &mut buffer) else {
            continue;
        };
        if matches!(
            code.emission.controls[next.0 as usize].layout,
            EmissionLayout::Block(node) if node.0 == label.0
        ) {
            code.emission.controls[index].continuation = None;
        }
    }
    Ok(())
}

/// The caller reserves space for all controls (at least three slots). Layout
/// roles, not containment order, determine which children P9 prints.
fn printed_parts<'a>(
    code: &'a MethodCode,
    control: ControlId,
    buffer: &'a mut [ControlId],
) -> &'a [ControlId] {
    let parts: &[ControlId] = match &code.emission.controls[control.0 as usize].layout {
        EmissionLayout::Block(_) => return &[],
        EmissionLayout::Sequence | EmissionLayout::Labelled | EmissionLayout::Graph => {
            return &code.controls[control.0 as usize].children;
        }
        EmissionLayout::If {
            header, yes, no, ..
        } => {
            buffer[..3].copy_from_slice(&[*header, *yes, *no]);
            return &buffer[..if code.emission.omitted(*no) { 2 } else { 3 }];
        }
        EmissionLayout::Loop { body, .. } => std::slice::from_ref(body),
        EmissionLayout::Switch { header, body, .. } => &[*header, *body],
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

fn printed_successor(
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

fn omit_empty_branches(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut empty = filled(code.controls.len(), false)?;
    let mut work = Vec::new();
    if let Some(root) = code.root {
        super::build::push(&mut work, (root, false))?;
    }
    while let Some((id, visited)) = work.pop() {
        let control = &code.emission.controls[id.0 as usize];
        if !visited {
            super::build::push(&mut work, (id, true))?;
            for &child in &code.controls[id.0 as usize].children {
                super::build::push(&mut work, (child, false))?;
            }
            continue;
        }
        if control.label_required
            || control.continuation.is_some()
            || code.emission.deferred_control(id)
        {
            continue;
        }
        empty[id.0 as usize] = match &control.layout {
            EmissionLayout::Block(node) => {
                let block = &code.emission.blocks[node.0 as usize];
                block.actions.is_empty() && !block.show_label && display_empty_end(code, &block.end)
            }
            EmissionLayout::Sequence => code.controls[id.0 as usize]
                .children
                .iter()
                .all(|child| empty[child.0 as usize]),
            _ => false,
        };
    }
    for index in 0..code.emission.controls.len() {
        if let EmissionLayout::If { no, test, .. } = code.emission.controls[index].layout
            && empty[no.0 as usize]
            && arm_prints_nothing(code, test.when_false, true)
        {
            code.emission.controls[no.0 as usize].omitted = true;
        }
    }
    for index in 0..code.terminals.len() {
        if let TerminalKind::If { when_false, .. } = code.terminals[index].kind {
            let phases = &code.emission.terminals[index];
            code.emission.empty_else[index] = phases.before.is_empty()
                && phases.snapshots.is_empty()
                && phases.after.is_empty()
                && arm_prints_nothing(code, when_false, false);
        }
    }
    Ok(())
}

/// A syntax-owned If arm prints only its copies; an ordinary terminal arm
/// additionally prints its transfer spelling.
fn arm_prints_nothing(code: &MethodCode, id: TransferId, syntax_owned: bool) -> bool {
    let transfer = &code.emission.transfers[id.0 as usize];
    transfer.copies.is_empty()
        && (syntax_owned || matches!(transfer.spelling, TransferSpelling::Fallthrough))
}

fn display_empty_end(code: &MethodCode, end: &BlockEnd) -> bool {
    match end {
        BlockEnd::None => true,
        BlockEnd::Transfers(transfers) => transfers
            .iter()
            .all(|&id| arm_prints_nothing(code, id, false)),
        BlockEnd::Terminal(id) => {
            let phases = &code.emission.terminals[id.0 as usize];
            phases.before.is_empty()
                && phases.snapshots.is_empty()
                && phases.after.is_empty()
                && matches!(code.terminals[id.0 as usize].kind,
                    TerminalKind::Goto(transfer) if arm_prints_nothing(code, transfer, false))
        }
    }
}
/// A field reference is owner, name and type. The type is printed only when two
/// delivered references share owner and name but not type, or when the original
/// name form or a use issue already requires an explicit reference.
fn field_qualifications(code: &MethodCode) -> Result<Vec<bool>, ExpressionError> {
    let mut explicit = filled(code.references.len(), false)?;
    for (index, reference) in code.references.iter().enumerate() {
        let Ok(ReferenceDeclaration::Member(MemberDeclaration::Field {
            owner,
            name,
            field_type,
        })) = &reference.declaration
        else {
            continue;
        };
        let owner_units = &code.types[owner.0 as usize].units;
        let field_units = &code.types[field_type.0 as usize].units;
        explicit[index] = name.form == NameForm::Original || reference.use_issue.is_some();
        if explicit[index] {
            continue;
        }
        for (other_index, other) in code.references.iter().enumerate() {
            if other_index == index {
                continue;
            }
            let Ok(ReferenceDeclaration::Member(MemberDeclaration::Field {
                owner: other_owner,
                name: other_name,
                field_type: other_type,
            })) = &other.declaration
            else {
                continue;
            };
            if other_name.units == name.units
                && code.types[other_owner.0 as usize].units == *owner_units
                && code.types[other_type.0 as usize].units != *field_units
            {
                explicit[index] = true;
                break;
            }
        }
    }
    Ok(explicit)
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

/// A `check-cast` whose value is read by the very next statement, at that
/// statement's first evaluated leaf, prints inside that use. Nothing is
/// reordered: the check still happens before every effect of that statement, and
/// the checked local is not written.
fn inline_check_casts(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut found: Vec<(StatementId, InlineCheck)> = Vec::new();
    for node in 0..code.emission.blocks.len() {
        let actions = &code.emission.blocks[node].actions;
        for index in 0..actions.len().saturating_sub(1) {
            let BodyAction::Statement {
                operation,
                statement,
            } = actions[index]
            else {
                continue;
            };
            let StatementKind::CheckCast { value, reference } =
                code.statements[statement.0 as usize].kind
            else {
                continue;
            };
            let ExprKind::Read { storage, .. } = code.expressions[value.0 as usize].kind else {
                continue;
            };
            let phases = &code.emission.operations[operation.0 as usize];
            if !phases.before.is_empty()
                || !phases.snapshots.is_empty()
                || !phases.after.is_empty()
                || code.statements[statement.0 as usize]
                    .sites
                    .iter()
                    .any(|&site| code.emission.site_annotation(site))
            {
                continue;
            }
            let BodyAction::Statement {
                statement: next, ..
            } = actions[index + 1]
            else {
                continue;
            };
            if code.statements[next.0 as usize]
                .outputs
                .iter()
                .any(|output| {
                    matches!(
                        output.materialization(),
                        crate::p5::Materialization::Store(local) if StorageId(local.0) == storage
                    )
                })
            {
                continue;
            }
            if first_leaf(code, next).is_none_or(|leaf| !reads(code, leaf, storage)) {
                continue;
            }
            super::build::push(
                &mut found,
                (
                    statement,
                    InlineCheck {
                        check: statement,
                        operation,
                        pc: code.operations[operation.0 as usize].pc,
                        value: storage,
                        reference,
                        target: next,
                    },
                ),
            )?;
        }
    }
    if found.is_empty() {
        return Ok(());
    }
    for (_, check) in &found {
        let index = u32::try_from(code.emission.inline_checks.len())
            .map_err(|_| crate::RequestError::PlatformCapacity)?;
        super::build::push(&mut code.emission.inline_checks, *check)?;
        super::build::push(
            &mut code.emission.check_uses[check.target.0 as usize],
            index,
        )?;
    }
    // The check statement prints nothing of its own; its provenance moves to the
    // use. Removing the collected statements keeps every other action in place.
    for block in &mut code.emission.blocks {
        block.actions.retain(|action| match action {
            BodyAction::Statement { statement, .. } | BodyAction::Release { statement, .. } => {
                !found.iter().any(|(check, _)| check == statement)
            }
            BodyAction::Receive { .. } => true,
        });
    }
    Ok(())
}

/// The leftmost value this statement evaluates in its printed form.
fn first_leaf(code: &MethodCode, statement: StatementId) -> Option<ExprId> {
    let statement = code.statements.get(statement.0 as usize)?;
    match &statement.kind {
        StatementKind::Move { expression, .. } | StatementKind::Evaluate { expression } => {
            leftmost(code, *expression)
        }
        // A natural store prints its receiver or array first; the explicit form
        // prints the value first.
        StatementKind::FieldStore {
            receiver, value, ..
        } => {
            if code.emission.natural_store(statement.id) {
                match receiver {
                    Some(receiver) => leftmost(code, *receiver),
                    None => None,
                }
            } else {
                leftmost(code, *value)
            }
        }
        StatementKind::ArrayStore { array, value, .. } => {
            if code.emission.natural_store(statement.id) {
                leftmost(code, *array)
            } else {
                leftmost(code, *value)
            }
        }
        _ => None,
    }
}

fn leftmost(code: &MethodCode, expression: ExprId) -> Option<ExprId> {
    let expression = code.expressions.get(expression.0 as usize)?;
    if let Some(composed) = code.emission.composed_use(expression.id) {
        return leftmost(code, composed.expression);
    }
    match &expression.kind {
        ExprKind::Read { .. }
        | ExprKind::Snapshot { .. }
        | ExprKind::Literal { .. }
        | ExprKind::Unavailable { .. } => Some(expression.id),
        ExprKind::Unary { value, .. } => leftmost(code, *value),
        ExprKind::Binary { left, .. } | ExprKind::Compare { left, .. } => leftmost(code, *left),
        ExprKind::Array { array, .. } => leftmost(code, *array),
        ExprKind::Field { receiver, .. } => receiver.and_then(|r| leftmost(code, r)),
        ExprKind::ArrayLength(a) => leftmost(code, *a),
        ExprKind::InstanceOf { value, .. } => leftmost(code, *value),
        ExprKind::NewArray { length, .. } => leftmost(code, *length),
        ExprKind::Call(call) => match call.receiver {
            Some(receiver) => leftmost(code, receiver),
            None => match &call.arguments {
                Arguments::Known(args) => args.first().and_then(|a| leftmost(code, *a)),
                Arguments::Unavailable { .. } => None,
            },
        },
        ExprKind::New { arguments, .. } => arguments.first().and_then(|a| leftmost(code, *a)),
        _ => None,
    }
}

fn reads(code: &MethodCode, expression: ExprId, storage: StorageId) -> bool {
    matches!(
        code.expressions.get(expression.0 as usize).map(|e| &e.kind),
        Some(ExprKind::Read { storage: read, .. }) if *read == storage
    )
}
