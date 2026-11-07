//! Composes an original `check-cast` into the existing display of its value's
//! first read, or omits it when P4 established it cannot fail there.
//!
//! Nothing is reordered and nothing is created: the check statement, its
//! operation, its exception site and the checked local all keep their original
//! facts, and the read that prints the check keeps its own expression. Only the
//! check's separate display action is dropped, so the surviving text still names
//! one original check.
//!
//! Two carriers exist, and both must be the next event after the check:
//!
//! 1. The next statement action of the same block, when the checked value is the
//!    first value that statement evaluates in its printed form. The carrier is
//!    that statement plus the exact read occurrence, so a second read of the same
//!    storage in the same statement keeps its own text.
//! 2. The first step of the single unconditional transfer that ends this block,
//!    when that existing copy assignment reads the checked storage. A check that
//!    is the block's last action has no other printed event before the transfer,
//!    so the assignment's right-hand side is its first read.
//!
//! A check whose pre-check disposition P4 published as redundant never composes:
//! the cast states nothing there, so its own statement is omitted and the
//! surviving carrier prints its original text unchanged.
//!
//! Anything else keeps the standalone check statement: a phase, a route
//! annotation, a value write, a selection, a Save or a gap report between the
//! check and the read would move the check past an event it now precedes.

use super::super::build::{bug, copy, push};
use super::super::*;
use crate::p2::OpId;
use crate::p4::CheckDisposition;
use crate::p5::{CopyLocation, Materialization, TransferSite};

pub(super) fn construct(
    code: &mut MethodCode,
    types: &crate::p4::TypeFacts,
    key: crate::MethodKey,
) -> Result<(), ExpressionError> {
    let omitted = redundant(code, types, key)?;
    let mut found: Vec<(StatementId, InlineCheck)> = Vec::new();
    statement_uses(code, &mut found, key)?;
    copy_uses(code, &mut found, key)?;
    terminal_uses(code, &mut found, key)?;
    // A redundant check has no text to carry, so its value's read keeps its own
    // spelling and the check's statement only contributes its origins.
    found.retain(|(statement, _)| {
        omitted
            .get(statement.0 as usize)
            .is_none_or(Option::is_none)
    });
    if !found.is_empty() {
        for (_, check) in &found {
            let index = u32::try_from(code.emission.inline_checks.len())
                .map_err(|_| crate::RequestError::PlatformCapacity)?;
            push(&mut code.emission.inline_checks, *check)?;
            let uses = match check.carrier {
                CheckCarrier::Statement(statement) => {
                    &mut code.emission.check_uses[statement.0 as usize]
                }
                CheckCarrier::Copy { bundle, .. } => {
                    &mut code.emission.copy_check_uses[bundle.0 as usize]
                }
                CheckCarrier::Terminal(terminal) => {
                    &mut code.emission.terminal_check_uses[terminal.0 as usize]
                }
            };
            push(uses, index)?;
        }
        // The check statement prints nothing of its own; its provenance moves to
        // the use. Removing the collected statements keeps every other action in
        // place.
        for block in &mut code.emission.blocks {
            block.actions.retain(|action| match action {
                BodyAction::Statement { statement, .. } | BodyAction::Release { statement, .. } => {
                    !found.iter().any(|(check, _)| check == statement)
                }
                BodyAction::Receive { .. } => true,
            });
        }
    }
    code.emission.omitted_checks = omitted;
    Ok(())
}

/// One original `check-cast` per statement whose own display adds no behavior.
/// Only a P4 `Redundant` disposition qualifies, and only for a statement whose
/// text states nothing else: a phase, a printed route or an output definition
/// keeps the statement, because its removal could not keep those obligations.
fn redundant(
    code: &MethodCode,
    types: &crate::p4::TypeFacts,
    key: crate::MethodKey,
) -> Result<Vec<Option<OmittedCheck>>, ExpressionError> {
    let mut omitted: Vec<Option<OmittedCheck>> =
        super::super::build::filled(code.statements.len(), None)?;
    for check in types.checks() {
        let CheckDisposition::Redundant(evidence) = check.disposition() else {
            continue;
        };
        let Some(operation) = code.operations.get(check.operation().0 as usize) else {
            continue;
        };
        let Some(statement) = operation.statement else {
            continue;
        };
        let Some((value, reference)) = eligible_check(code, check.operation(), statement, key)
        else {
            continue;
        };
        let at = omitted
            .get_mut(statement.0 as usize)
            .ok_or(bug("check-cast names an unknown statement"))?;
        *at = Some(OmittedCheck {
            check: statement,
            operation: check.operation(),
            pc: operation.pc,
            value,
            reference,
            sources: copy(evidence.sources())?,
        });
    }
    Ok(omitted)
}

/// The checked storage and declared target of a `check-cast` that prints no
/// separate work of its own. A phase or printed route would be an event after
/// the check, and a result definition would be text only the statement carries.
fn eligible_check(
    code: &MethodCode,
    operation: OpId,
    statement: StatementId,
    key: crate::MethodKey,
) -> Option<(StorageId, ReferenceId)> {
    let refuse = |why: &'static str| super::trace::cast_refused(key, statement, why);
    let s = code.statements.get(statement.0 as usize)?;
    // Only a real check-cast can be refused; every other statement simply has no
    // carrier question, so it is silent.
    let StatementKind::CheckCast { value, reference } = s.kind else {
        return None;
    };
    if !s.outputs.is_empty() {
        refuse("check_has_output");
        return None;
    }
    let ExprKind::Read { storage, .. } = code.expressions.get(value.0 as usize)?.kind else {
        refuse("value_not_a_read");
        return None;
    };
    let Some(phases) = code.emission.operations.get(operation.0 as usize) else {
        refuse("no_phase_record");
        return None;
    };
    if !phases.before.is_empty() || !phases.snapshots.is_empty() || !phases.after.is_empty() {
        refuse("check_has_copy_phase");
        return None;
    }
    if s.sites
        .iter()
        .any(|&site| code.emission.site_annotation(site))
    {
        refuse("check_site_is_printed");
        return None;
    }
    Some((storage, reference))
}

fn carried_check(
    code: &MethodCode,
    operation: OpId,
    statement: StatementId,
    value: StorageId,
    reference: ReferenceId,
    carrier: CheckCarrier,
    occurrence: ExprId,
) -> Result<(StatementId, InlineCheck), ExpressionError> {
    let pc = code
        .operations
        .get(operation.0 as usize)
        .ok_or(bug("check-cast lost its operation"))?
        .pc;
    Ok((
        statement,
        InlineCheck {
            check: statement,
            operation,
            pc,
            value,
            reference,
            carrier,
            occurrence,
        },
    ))
}

/// Only one check may name one printed occurrence. A second one keeps its own
/// statement: the exact carrier is what makes the composition a single-use
/// display, so it must never resolve to two omitted checks.
fn unclaimed_occurrence(found: &[(StatementId, InlineCheck)], check: &InlineCheck) -> bool {
    !found.iter().any(|(_, existing)| {
        existing.carrier == check.carrier && existing.occurrence == check.occurrence
    })
}

/// The checked value read by the very next statement action, at that statement's
/// first evaluated value. Nothing is reordered: the check still happens before
/// every effect of that statement, and the checked local is not written.
fn statement_uses(
    code: &MethodCode,
    found: &mut Vec<(StatementId, InlineCheck)>,
    key: crate::MethodKey,
) -> Result<(), ExpressionError> {
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
            let Some((storage, reference)) = eligible_check(code, operation, statement, key) else {
                continue;
            };
            let refuse = |why: &'static str| super::trace::cast_refused(key, statement, why);
            let BodyAction::Statement {
                statement: next, ..
            } = actions[index + 1]
            else {
                refuse("next_action_is_not_a_statement");
                continue;
            };
            if code.statements[next.0 as usize]
                .outputs
                .iter()
                .any(|output| {
                    matches!(
                        output.materialization(),
                        Materialization::Store(local) if StorageId(local.0) == storage
                    )
                })
            {
                refuse("next_statement_writes_the_value");
                continue;
            }
            let Some(occurrence) = first_leaf(code, next) else {
                refuse("next_statement_has_no_first_leaf");
                continue;
            };
            if !reads(code, occurrence, storage) {
                refuse("first_leaf_is_another_value");
                continue;
            }
            if !rendered_once(code, next, occurrence)? {
                refuse("first_leaf_renders_twice");
                continue;
            }
            let carried = carried_check(
                code,
                operation,
                statement,
                storage,
                reference,
                CheckCarrier::Statement(next),
                occurrence,
            )?;
            if unclaimed_occurrence(found, &carried.1) {
                push(found, carried)?;
            }
        }
    }
    Ok(())
}

/// The checked value read by the first executed step of this block's single
/// unconditional transfer copy set. A check that is the block's last action and
/// whose transfer carries no earlier step is followed by that read immediately;
/// a terminal, a second transfer, a Save or another bundle between them keeps
/// the standalone statement.
fn copy_uses(
    code: &MethodCode,
    found: &mut Vec<(StatementId, InlineCheck)>,
    key: crate::MethodKey,
) -> Result<(), ExpressionError> {
    for node in 0..code.emission.blocks.len() {
        let block = &code.emission.blocks[node];
        let Some((last, _)) = block.actions.split_last() else {
            continue;
        };
        let BodyAction::Statement {
            operation,
            statement,
        } = *last
        else {
            continue;
        };
        let Some((storage, reference)) = eligible_check(code, operation, statement, key) else {
            continue;
        };
        let refuse = |why: &'static str| super::trace::cast_refused(key, statement, why);
        let BlockEnd::Transfers(transfers) = &block.end else {
            // A terminal evaluates its own operands and selects between its
            // original edges, so the check still precedes both outcomes.
            refuse("block_ends_in_a_terminal");
            continue;
        };
        let [transfer] = transfers.as_slice() else {
            refuse("block_has_two_transfers");
            continue;
        };
        let Some(planned) = code.emission.transfers.get(transfer.0 as usize) else {
            continue;
        };
        if matches!(planned.spelling, TransferSpelling::Selected) {
            // A selected arm prints this bundle for one outcome only.
            refuse("transfer_is_a_selected_arm");
            continue;
        }
        let [bundle] = planned.copies.as_slice() else {
            refuse("transfer_has_not_exactly_one_bundle");
            continue;
        };
        let Some(bundle) = code.copies.get(bundle.0 as usize) else {
            continue;
        };
        let CopyLocation::Transfer {
            site: TransferSite::NormalEdge(edge),
            ..
        } = bundle.location
        else {
            refuse("bundle_is_not_a_normal_edge_copy");
            continue;
        };
        // The bundle must be this transfer's own normal edge, not a copy another
        // composition step placed here from a different origin.
        if code.transfers.get(transfer.0 as usize).and_then(|t| t.edge) != Some(edge) {
            refuse("bundle_edge_is_not_this_transfer");
            continue;
        }
        if !matches!(bundle.emission, CopyEmission::Original) {
            refuse("bundle_was_relocated");
            continue;
        }
        let Some(CopyStatement::Assign {
            destination,
            expression,
            ..
        }) = bundle.steps.first()
        else {
            refuse("first_copy_step_is_not_an_assignment");
            continue;
        };
        if *destination == storage {
            // The surviving assignment would look like the check wrote the local.
            refuse("copy_writes_the_checked_local");
            continue;
        }
        if !reads(code, *expression, storage) {
            refuse("copy_rhs_is_another_value");
            continue;
        }
        let carried = carried_check(
            code,
            operation,
            statement,
            storage,
            reference,
            // The carrier is the bundle's first executed step.
            CheckCarrier::Copy {
                bundle: bundle.id,
                step: 0,
            },
            *expression,
        )?;
        if unclaimed_occurrence(found, &carried.1) {
            push(found, carried)?;
        }
    }
    Ok(())
}

/// The checked value read by the first evaluated operand of this block's own
/// terminal. The check is the block's last action, so the terminal follows it
/// with nothing printed in between; a before-terminal copy, snapshot or selected
/// arm would move the check past that event and keeps the standalone statement.
fn terminal_uses(
    code: &MethodCode,
    found: &mut Vec<(StatementId, InlineCheck)>,
    key: crate::MethodKey,
) -> Result<(), ExpressionError> {
    for node in 0..code.emission.blocks.len() {
        let block = &code.emission.blocks[node];
        let Some((last, _)) = block.actions.split_last() else {
            continue;
        };
        let BodyAction::Statement {
            operation,
            statement,
        } = *last
        else {
            continue;
        };
        let Some((storage, reference)) = eligible_check(code, operation, statement, key) else {
            continue;
        };
        // The terminal may sit in a following portion of the same original
        // block: a plain within-block link with no copy prints nothing between
        // the check and the terminal operand.
        let refuse = |why: &'static str| super::trace::cast_refused(key, statement, why);
        let Some(terminal) = terminal_after(code, node)? else {
            refuse("no_terminal_follows");
            continue;
        };
        let phases = &code.emission.terminals[terminal.0 as usize];
        if !phases.before.is_empty() {
            refuse("terminal_has_before_copies");
            continue;
        }
        let target = &code.terminals[terminal.0 as usize];
        let operand = match &target.kind {
            TerminalKind::Return(Some(value)) | TerminalKind::Throw(value) => *value,
            TerminalKind::If { condition, .. } => match condition {
                Condition::Pair { left, .. } => *left,
                Condition::Zero { value, .. } => *value,
            },
            TerminalKind::Switch { selector, .. } => *selector,
            TerminalKind::Return(None) | TerminalKind::None | TerminalKind::Goto(_) => {
                refuse("terminal_has_no_operand");
                continue;
            }
        };
        let Some(occurrence) = leftmost(code, operand) else {
            refuse("terminal_operand_has_no_first_leaf");
            continue;
        };
        if !reads(code, occurrence, storage) {
            refuse("terminal_first_leaf_is_another_value");
            continue;
        }
        if !once_in(code, operand, occurrence)? {
            refuse("terminal_operand_renders_twice");
            continue;
        }
        let carried = carried_check(
            code,
            operation,
            statement,
            storage,
            reference,
            CheckCarrier::Terminal(terminal),
            occurrence,
        )?;
        if unclaimed_occurrence(found, &carried.1) {
            push(found, carried)?;
        }
    }
    Ok(())
}

/// The terminal this block reaches without printing anything in between: its own
/// end, or a chain of empty blocks joined by single copy-free within-block links.
fn terminal_after(
    code: &MethodCode,
    mut node: usize,
) -> Result<Option<crate::p2::BlockId>, ExpressionError> {
    for _ in 0..code.emission.blocks.len() {
        let block = &code.emission.blocks[node];
        match &block.end {
            BlockEnd::Terminal(terminal) => return Ok(Some(*terminal)),
            BlockEnd::None => {
                // A syntax-owned predicate portion: the If or loop control prints
                // this terminal's operand, and nothing else stands between.
                if let BlockCompletion::Syntax {
                    terminal,
                    owner: TerminalEmission::If(_) | TerminalEmission::Loop(_),
                } = code.blocks[node].completion
                {
                    return Ok(Some(terminal));
                }
                return Ok(None);
            }
            BlockEnd::Transfers(transfers) => {
                let [only] = transfers.as_slice() else {
                    return Ok(None);
                };
                let transfer = code
                    .emission
                    .transfers
                    .get(only.0 as usize)
                    .ok_or(bug("check-cast carrier names an unknown transfer"))?;
                if !transfer.copies.is_empty()
                    || !matches!(transfer.spelling, TransferSpelling::Fallthrough)
                {
                    return Ok(None);
                }
                let from = code
                    .transfers
                    .get(only.0 as usize)
                    .ok_or(bug("check-cast carrier names an unknown transfer plan"))?;
                if from.edge.is_some() {
                    return Ok(None);
                }
                let next = from.destination.0 as usize;
                if code
                    .emission
                    .blocks
                    .get(next)
                    .is_none_or(|next| !next.actions.is_empty())
                {
                    return Ok(None);
                }
                node = next;
            }
        }
    }
    Err(bug("check-cast carrier walk did not reach a terminal"))
}

/// Whether this expression tree renders the occurrence exactly once. A composed
/// subtree prints under another carrier, so it is not a position of this one.
fn once_in(code: &MethodCode, root: ExprId, occurrence: ExprId) -> Result<bool, ExpressionError> {
    let mut work = Vec::new();
    push(&mut work, root)?;
    let mut seen = 0u8;
    while let Some(id) = work.pop() {
        if id == occurrence {
            seen = seen.saturating_add(1);
            if seen > 1 {
                return Ok(false);
            }
        }
        let expression = code
            .expressions
            .get(id.0 as usize)
            .ok_or(bug("terminal expression is outside its arena"))?;
        if code.emission.composed_use(id).is_some() {
            continue;
        }
        match &expression.kind {
            ExprKind::Unary { value, .. } => push(&mut work, *value)?,
            ExprKind::Binary { left, right, .. } | ExprKind::Compare { left, right, .. } => {
                push(&mut work, *left)?;
                push(&mut work, *right)?;
            }
            ExprKind::Array { array, index, .. } => {
                push(&mut work, *array)?;
                push(&mut work, *index)?;
            }
            ExprKind::Field {
                receiver: Some(receiver),
                ..
            } => push(&mut work, *receiver)?,
            ExprKind::ArrayLength(array) => push(&mut work, *array)?,
            ExprKind::InstanceOf { value, .. } => push(&mut work, *value)?,
            ExprKind::NewArray { length, .. } => push(&mut work, *length)?,
            ExprKind::Call(call) => {
                if let Some(receiver) = call.receiver {
                    push(&mut work, receiver)?;
                }
                if let Arguments::Known(args) = &call.arguments {
                    for &argument in args {
                        push(&mut work, argument)?;
                    }
                }
            }
            ExprKind::New { arguments, .. } => {
                for &argument in arguments {
                    push(&mut work, argument)?;
                }
            }
            ExprKind::FilledArray {
                elements: Arguments::Known(elements),
                ..
            } => {
                for &element in elements {
                    push(&mut work, element)?;
                }
            }
            _ => {}
        }
    }
    Ok(seen == 1)
}

/// The leftmost value this statement evaluates in its printed form. This is the
/// exact expression occurrence the check can print at.
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
                // A static store names its owner without evaluating a receiver,
                // so the value is the first evaluated operand it prints.
                match receiver {
                    Some(receiver) => leftmost(code, *receiver),
                    None => leftmost(code, *value),
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

/// Whether this statement's printed expression trees render the occurrence
/// exactly once. P7 builds one expression per operand use, but the carrier must
/// not depend on that construction rule: a shared occurrence would print the
/// check twice. A composed subtree prints under another statement's carrier, so
/// it is not a position of this carrier.
fn rendered_once(
    code: &MethodCode,
    statement: StatementId,
    occurrence: ExprId,
) -> Result<bool, ExpressionError> {
    let statement = code
        .statements
        .get(statement.0 as usize)
        .ok_or(bug("check-cast carrier names an unknown statement"))?;
    let mut roots = [None; 3];
    match &statement.kind {
        StatementKind::Move { expression, .. } | StatementKind::Evaluate { expression } => {
            roots[0] = Some(*expression);
        }
        StatementKind::FieldStore {
            receiver, value, ..
        } => {
            roots[0] = *receiver;
            roots[1] = Some(*value);
        }
        StatementKind::ArrayStore {
            array,
            index,
            value,
            ..
        } => {
            roots[0] = Some(*array);
            roots[1] = Some(*index);
            roots[2] = Some(*value);
        }
        _ => return Ok(false),
    }
    let mut work = Vec::new();
    for root in roots.into_iter().flatten() {
        push(&mut work, root)?;
    }
    let mut seen = 0u8;
    while let Some(id) = work.pop() {
        if id == occurrence {
            seen = seen.saturating_add(1);
            if seen > 1 {
                return Ok(false);
            }
        }
        let expression = code
            .expressions
            .get(id.0 as usize)
            .ok_or(bug("statement expression is outside its arena"))?;
        if code.emission.composed_use(id).is_some() {
            continue;
        }
        match &expression.kind {
            ExprKind::Unary { value, .. } => push(&mut work, *value)?,
            ExprKind::Binary { left, right, .. } | ExprKind::Compare { left, right, .. } => {
                push(&mut work, *left)?;
                push(&mut work, *right)?;
            }
            ExprKind::Array { array, index, .. } => {
                push(&mut work, *array)?;
                push(&mut work, *index)?;
            }
            ExprKind::Field {
                receiver: Some(receiver),
                ..
            } => push(&mut work, *receiver)?,
            ExprKind::ArrayLength(array) => push(&mut work, *array)?,
            ExprKind::InstanceOf { value, .. } => push(&mut work, *value)?,
            ExprKind::NewArray { length, .. } => push(&mut work, *length)?,
            ExprKind::Call(call) => {
                if let Some(receiver) = call.receiver {
                    push(&mut work, receiver)?;
                }
                if let Arguments::Known(args) = &call.arguments {
                    for &argument in args {
                        push(&mut work, argument)?;
                    }
                }
            }
            ExprKind::New { arguments, .. } => {
                for &argument in arguments {
                    push(&mut work, argument)?;
                }
            }
            ExprKind::FilledArray {
                elements: Arguments::Known(elements),
                ..
            } => {
                for &element in elements {
                    push(&mut work, element)?;
                }
            }
            _ => {}
        }
    }
    Ok(seen == 1)
}

fn leftmost(code: &MethodCode, expression: ExprId) -> Option<ExprId> {
    let expression = code.expressions.get(expression.0 as usize)?;
    if code.emission.composed_use(expression.id).is_some() {
        // A composed use prints another statement's expression under that
        // statement's carrier, so this read cannot carry the check here.
        return None;
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
