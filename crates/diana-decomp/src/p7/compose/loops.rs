//! Moves a do-while repetition-edge copy set into the loop body when the
//! delivered facts prove the two schedules equivalent.
//!
//! The move is admitted only under conditions that hold for the printed shape,
//! not for a particular input:
//!
//! 1. The loop is a tail-test form, so the predicate is reached only after the
//!    body. A head-test predicate is also reached on entry with different state.
//! 2. The predicate block has exactly one normal incoming transfer and it comes
//!    from inside the body. Another arrival (mid-body `continue`, explicit jump)
//!    would skip the moved copies.
//! 3. No destination written by the moved copies is read by the condition. The
//!    condition is evaluated after the original edge but before the moved copy.
//! 4. No destination is read or written by the exiting-edge copies, which run on
//!    the exit path where the moved copies now also run.
//! 5. No destination is read outside the loop subtree, by another copy, by a
//!    handler receive or by a construction. Copies are plain assignments, so a
//!    later read would observe the extra execution.
//!
//! Each condition is a conservative fact over the owned arenas. When any of them
//! cannot be established, the copies stay on their original edge.

use super::super::build::{bug, filled};
use super::super::*;
use crate::p3::UseSite;

pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut parent = vec_of(code.controls.len(), None)?;
    for control in &code.controls {
        for &child in &control.children {
            *parent
                .get_mut(child.0 as usize)
                .ok_or(bug("control names an unknown child"))? = Some(control.id);
        }
    }
    let mut owner = vec_of(code.labels.len(), None)?;
    for label in &code.labels {
        *owner
            .get_mut(label.node.0 as usize)
            .ok_or(bug("label names an unknown node"))? = Some(label.owner);
    }
    let mut block_owner = vec_of(code.terminals.len(), None)?;
    for (node, block) in code.emission.blocks.iter().enumerate() {
        if let BlockEnd::Terminal(terminal) = block.end
            && let Some(label) = code.labels.get(node)
        {
            *block_owner
                .get_mut(terminal.0 as usize)
                .ok_or(bug("block end names an unknown terminal"))? = Some(label.owner);
        }
    }

    for loop_id in 0..code.controls.len() {
        let ControlKind::Loop {
            syntax:
                LoopSyntax::DoWhile {
                    terminal,
                    repeat,
                    exit,
                },
            ..
        } = &code.controls[loop_id].kind
        else {
            continue;
        };
        let (terminal, repeat, exit) = (*terminal, *repeat, *exit);
        if code.emission.transfers[repeat.0 as usize].copies.is_empty() {
            continue;
        }
        // 1. Tail-test placement.
        if !matches!(
            code.emission.controls[loop_id].layout,
            EmissionLayout::Loop {
                placement: LoopPlacement::DoWhile { .. },
                ..
            }
        ) {
            continue;
        }
        let (Some(&body), Some(&predicate)) = (
            code.controls[loop_id].children.first(),
            code.controls[loop_id].children.get(1),
        ) else {
            continue;
        };
        let EmissionLayout::Block(predicate_node) =
            code.emission.controls[predicate.0 as usize].layout
        else {
            continue;
        };
        // 2. Exactly one arrival, from inside the body.
        let arrivals: Vec<usize> = (0..code.transfers.len())
            .filter(|&index| code.transfers[index].destination.0 == predicate_node.0)
            .collect();
        let [tail] = arrivals.as_slice() else {
            continue;
        };
        let tail_owner = code
            .labels
            .get(code.transfers[*tail].from.0 as usize)
            .map(|label| label.owner);
        if !in_subtree(code, body, tail_owner) {
            continue;
        }
        let moved = code.emission.transfers[repeat.0 as usize].copies.clone();
        let mut destinations = Vec::new();
        for &copy in &moved {
            collect_destinations(code, copy, &mut destinations)?;
        }
        if destinations.is_empty() {
            continue;
        }
        // 3. The condition does not observe the moved writes.
        let TerminalKind::If { condition, .. } = &code.terminals[terminal.0 as usize].kind else {
            continue;
        };
        let mut reads = Vec::new();
        for operand in condition_operands(condition)? {
            collect_reads(code, operand, &mut reads)?;
        }
        if intersects(&reads, &destinations) {
            continue;
        }
        // 4. The exiting edge copies neither read nor write them.
        let mut exit_locals = Vec::new();
        for &copy in &code.emission.transfers[exit.0 as usize].copies {
            collect_locals(code, copy, &mut exit_locals)?;
        }
        if intersects(&exit_locals, &destinations) {
            continue;
        }
        // 5. Nothing outside the loop observes the destinations.
        if observed_outside(code, body, &owner, &block_owner, &destinations, &moved)? {
            continue;
        }
        for copy in moved {
            crate::substrate::owned::push(&mut code.emission.transfers[*tail].copies, copy)?;
        }
        code.emission.transfers[repeat.0 as usize].copies.clear();
    }
    Ok(())
}

fn vec_of<T>(len: usize, value: T) -> Result<Vec<T>, ExpressionError>
where
    T: Clone,
{
    let mut values = Vec::new();
    crate::substrate::owned::reserve(&mut values, len)?;
    values.resize(len, value);
    Ok(values)
}

fn in_subtree(code: &MethodCode, root: ControlId, candidate: Option<ControlId>) -> bool {
    let Some(candidate) = candidate else {
        return false;
    };
    let mut stack = Vec::new();
    if crate::substrate::owned::push(&mut stack, root).is_err() {
        return false;
    }
    while let Some(id) = stack.pop() {
        if id == candidate {
            return true;
        }
        let Some(control) = code.controls.get(id.0 as usize) else {
            return false;
        };
        for &child in &control.children {
            if crate::substrate::owned::push(&mut stack, child).is_err() {
                return false;
            }
        }
        if stack.len() > code.controls.len() {
            return false;
        }
    }
    false
}

fn collect_destinations(
    code: &MethodCode,
    copy: CopyPlanId,
    out: &mut Vec<StorageId>,
) -> Result<(), ExpressionError> {
    let bundle = code
        .copies
        .get(copy.0 as usize)
        .ok_or(bug("repeat edge names an unknown copy"))?;
    for step in &bundle.steps {
        let destination = match step {
            CopyStatement::Assign { destination, .. } | CopyStatement::Save { destination, .. } => {
                *destination
            }
        };
        crate::substrate::owned::push(out, destination)?;
    }
    Ok(())
}

fn collect_locals(
    code: &MethodCode,
    copy: CopyPlanId,
    out: &mut Vec<StorageId>,
) -> Result<(), ExpressionError> {
    let bundle = code
        .copies
        .get(copy.0 as usize)
        .ok_or(bug("exit edge names an unknown copy"))?;
    for step in &bundle.steps {
        match step {
            CopyStatement::Save {
                destination,
                source,
            } => {
                crate::substrate::owned::push(out, *destination)?;
                crate::substrate::owned::push(out, *source)?;
            }
            CopyStatement::Assign {
                destination,
                original,
                ..
            } => {
                if let Some(local) = original.local() {
                    crate::substrate::owned::push(out, StorageId(local.0))?;
                }
                crate::substrate::owned::push(out, *destination)?;
            }
        }
    }
    Ok(())
}

fn condition_operands(condition: &Condition) -> Result<Vec<ExprId>, ExpressionError> {
    let mut out = Vec::new();
    match condition {
        Condition::Pair { left, right, .. } => {
            crate::substrate::owned::push(&mut out, *left)?;
            crate::substrate::owned::push(&mut out, *right)?;
        }
        Condition::Zero { value, .. } => crate::substrate::owned::push(&mut out, *value)?,
    }
    Ok(out)
}

fn collect_reads(
    code: &MethodCode,
    expression: ExprId,
    out: &mut Vec<StorageId>,
) -> Result<(), ExpressionError> {
    let Some(expression) = code.expressions.get(expression.0 as usize) else {
        return Ok(());
    };
    if let Some(composed) = code.emission.composed_use(expression.id) {
        return collect_reads(code, composed.expression, out);
    }
    let mut nested = [None; 2];
    match &expression.kind {
        ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } => {
            crate::substrate::owned::push(out, *storage)?;
        }
        ExprKind::Unary { value, .. } | ExprKind::ArrayLength(value) => nested[0] = Some(*value),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Compare { left, right, .. }
        | ExprKind::Array {
            array: left,
            index: right,
            ..
        } => {
            nested[0] = Some(*left);
            nested[1] = Some(*right);
        }
        ExprKind::InstanceOf { value, .. } => nested[0] = Some(*value),
        ExprKind::NewArray { length, .. } => nested[0] = Some(*length),
        _ => {}
    }
    for value in nested.into_iter().flatten() {
        collect_reads(code, value, out)?;
    }
    Ok(())
}

fn intersects(left: &[StorageId], right: &[StorageId]) -> bool {
    left.iter().any(|value| right.contains(value))
}

fn read_of(step: &CopyStatement) -> Option<StorageId> {
    match step {
        CopyStatement::Save { source, .. } => Some(*source),
        CopyStatement::Assign { original, .. } => original.local().map(|local| StorageId(local.0)),
    }
}

fn observed_outside(
    code: &MethodCode,
    body: ControlId,
    owner: &[Option<ControlId>],
    block_owner: &[Option<ControlId>],
    destinations: &[StorageId],
    moved: &[CopyPlanId],
) -> Result<bool, ExpressionError> {
    for copy in &code.copies {
        if moved.contains(&copy.id) {
            continue;
        }
        for step in &copy.steps {
            if read_of(step).is_some_and(|read| destinations.contains(&read)) {
                return Ok(true);
            }
        }
    }
    for handler in &code.handlers {
        for copy in &handler.receives {
            let Some(bundle) = code.copies.get(copy.0 as usize) else {
                return Ok(true);
            };
            for step in &bundle.steps {
                if read_of(step).is_some_and(|read| destinations.contains(&read)) {
                    return Ok(true);
                }
            }
        }
    }
    for group in &code.constructions {
        for read in &group.arguments {
            if let Some(local) = read.local()
                && destinations.contains(&StorageId(local.0))
            {
                return Ok(true);
            }
        }
        if group
            .publications
            .iter()
            .any(|publication| destinations.contains(&publication.destination))
        {
            return Ok(true);
        }
    }
    for expression in &code.expressions {
        let read = match expression.kind {
            ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } => storage,
            _ => continue,
        };
        if !destinations.contains(&read) {
            continue;
        }
        // An unresolvable owner could be outside the loop.
        let Some(origin) = expression.origin.as_ref() else {
            return Ok(true);
        };
        let control = match origin.site {
            UseSite::Operation { operation, .. } => code
                .operations
                .get(operation.0 as usize)
                .and_then(|operation| owner.get(operation.node.0 as usize).copied().flatten()),
            UseSite::Terminal { block, .. } => block_owner.get(block.0 as usize).copied().flatten(),
        };
        if !in_subtree(code, body, control) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Compactness consumes the delivered ordered evaluation subtree. Only
/// suppressed composed producers and copy-free original sequencing may precede
/// its syntax-owned terminal; all other prelude work keeps explicit test phases.
pub(super) fn select_compact(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut compact = filled(code.controls.len(), false)?;
    for (index, control) in code.emission.controls.iter().enumerate() {
        let (predicate, test) = match control.layout {
            EmissionLayout::Loop {
                placement:
                    LoopPlacement::While {
                        predicate, test, ..
                    }
                    | LoopPlacement::DoWhile {
                        predicate, test, ..
                    },
                ..
            } => (predicate, test),
            _ => continue,
        };
        let phases = &code.emission.terminals[test.terminal.0 as usize];
        if !phases.before.is_empty() || !phases.snapshots.is_empty() || !phases.after.is_empty() {
            continue;
        }
        let mut stack = Vec::new();
        super::super::build::push(&mut stack, predicate)?;
        let mut empty = true;
        while let Some(id) = stack.pop() {
            let control = &code.emission.controls[id.0 as usize];
            if control.label_required || control.continuation.is_some() {
                empty = false;
                break;
            }
            match &control.layout {
                EmissionLayout::Sequence => {
                    for &child in control.children(&code.controls[id.0 as usize]) {
                        super::super::build::push(&mut stack, child)?;
                    }
                }
                EmissionLayout::Block(node) => {
                    let block = &code.emission.blocks[node.0 as usize];
                    if block.show_label
                        || block.actions.iter().any(|action| {
                            !matches!(action, BodyAction::Statement { operation, .. }
                            if code.emission.composed_operation(*operation))
                        })
                    {
                        empty = false;
                        break;
                    }
                    let end_empty = match &block.end {
                        BlockEnd::None => true,
                        BlockEnd::Transfers(transfers) => transfers.iter().all(|transfer| {
                            let transfer = &code.emission.transfers[transfer.0 as usize];
                            matches!(transfer.spelling, TransferSpelling::Fallthrough)
                                && transfer.copies.is_empty()
                        }),
                        BlockEnd::Terminal(_) => false,
                    };
                    if !end_empty {
                        empty = false;
                        break;
                    }
                }
                _ => {
                    empty = false;
                    break;
                }
            }
        }
        compact[index] = empty;
    }
    for (index, control) in code.emission.controls.iter_mut().enumerate() {
        if let EmissionLayout::Loop {
            placement:
                LoopPlacement::While {
                    compact: selected, ..
                }
                | LoopPlacement::DoWhile {
                    compact: selected, ..
                },
            ..
        } = &mut control.layout
        {
            *selected = compact[index];
        }
    }
    Ok(())
}
