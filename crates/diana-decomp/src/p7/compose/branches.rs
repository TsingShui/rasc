//! Display shapes over completed child schedules. Original arm ownership and
//! predicate-edge identities remain unchanged, including for an inverted If.

use super::super::build::{filled, push};
use super::super::*;

pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let mut empty = filled(code.controls.len(), false)?;
    let mut single_if = filled(code.controls.len(), false)?;
    let mut abrupt = filled(code.controls.len(), false)?;
    let mut work = Vec::new();
    if let Some(root) = code.root {
        push(&mut work, (root, false))?;
    }
    while let Some((id, visited)) = work.pop() {
        if !visited {
            push(&mut work, (id, true))?;
            for &child in
                code.emission.controls[id.0 as usize].children(&code.controls[id.0 as usize])
            {
                push(&mut work, (child, false))?;
            }
            continue;
        }
        // Inner display choices must be complete before the containing arm is
        // classified: an empty If can itself empty a surrounding Sequence.
        if let EmissionLayout::If { yes, no, test, .. } =
            code.emission.controls[id.0 as usize].layout
        {
            let yes_empty = empty[yes.0 as usize] && copies_print_nothing(code, test.when_true);
            let no_empty = empty[no.0 as usize] && arm_prints_nothing(code, test.when_false, true);
            if no_empty {
                code.emission.controls[no.0 as usize].omitted = true;
            }
            let both_empty =
                yes_empty && empty[no.0 as usize] && copies_print_nothing(code, test.when_false);
            let control = &code.emission.controls[id.0 as usize];
            let shape = if both_empty
                && control.continuation.is_none()
                && !code.emission.deferred_control(id)
                && discardable_predicate(code, test)
            {
                IfShape::Empty
            } else if yes_empty && !no_empty && !displayed_predicate_site(code, test) {
                IfShape::Inverted
            } else if !yes_empty && !no_empty && !single_if[id.0 as usize] && abrupt[yes.0 as usize]
            {
                IfShape::NoElse
            } else if !no_empty
                && arm_prints_nothing(code, test.when_false, true)
                && single_if[no.0 as usize]
            {
                IfShape::ElseIf
            } else {
                IfShape::IfElse
            };
            if let EmissionLayout::If {
                shape: selected,
                test,
                ..
            } = &mut code.emission.controls[id.0 as usize].layout
            {
                *selected = shape;
                if matches!(shape, IfShape::Inverted) {
                    test.negate = !test.negate;
                }
            }
        }
        let control = &code.emission.controls[id.0 as usize];
        if control.label_required
            || control.continuation.is_some()
            || code.emission.deferred_control(id)
        {
            continue;
        }
        match &control.layout {
            EmissionLayout::Block(node) => {
                let block = &code.emission.blocks[node.0 as usize];
                if let BlockEnd::Terminal(terminal) = block.end
                    && let TerminalKind::If {
                        when_true,
                        when_false,
                        ..
                    } = code.terminals[terminal.0 as usize].kind
                {
                    code.emission.empty_if[terminal.0 as usize] =
                        [when_true, when_false].iter().all(|&edge| {
                            matches!(
                                code.emission.transfers[edge.0 as usize].spelling,
                                TransferSpelling::Fallthrough
                            ) && copies_print_nothing(code, edge)
                        }) && discardable_predicate(
                            code,
                            PredicateEmission {
                                terminal,
                                when_true,
                                when_false,
                                negate: false,
                            },
                        );
                }
                empty[id.0 as usize] = block.actions.is_empty()
                    && !block.show_label
                    && display_empty_end(code, &block.end);
                abrupt[id.0 as usize] = display_abrupt_end(code, &block.end);
            }
            EmissionLayout::Sequence => {
                empty[id.0 as usize] = true;
                for &child in
                    code.emission.controls[id.0 as usize].children(&code.controls[id.0 as usize])
                {
                    if empty[child.0 as usize] {
                        continue;
                    }
                    if !empty[id.0 as usize] {
                        single_if[id.0 as usize] = false;
                        break;
                    }
                    empty[id.0 as usize] = false;
                    single_if[id.0 as usize] = single_if[child.0 as usize];
                    abrupt[id.0 as usize] = abrupt[child.0 as usize];
                }
            }
            EmissionLayout::If {
                header,
                test,
                shape,
                no,
                ..
            } => {
                abrupt[id.0 as usize] = !matches!(shape, IfShape::Empty) && abrupt[no.0 as usize];
                if matches!(shape, IfShape::Empty) {
                    empty[id.0 as usize] = empty[header.0 as usize];
                } else if empty[header.0 as usize]
                    && code.emission.terminals[test.terminal.0 as usize]
                        .before
                        .is_empty()
                {
                    // Nothing may print between `else ` and the inner `if`.
                    single_if[id.0 as usize] = true;
                }
            }
            EmissionLayout::Graph | EmissionLayout::Labelled => {
                // An explicit container prints its children in order, so only
                // the last printed child decides whether control leaves. Its own
                // transfers keep jumping to the original targets.
                abrupt[id.0 as usize] = code.emission.controls[id.0 as usize]
                    .children(&code.controls[id.0 as usize])
                    .last()
                    .is_some_and(|last| abrupt[last.0 as usize]);
            }
            _ => {}
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

fn displayed_predicate_site(code: &MethodCode, test: PredicateEmission) -> bool {
    let terminal = &code.terminals[test.terminal.0 as usize];
    if terminal
        .site
        .is_some_and(|site| code.emission.site_annotation(site))
    {
        return true;
    }
    let displayed_operand_site = |operand| {
        code.emission
            .composed_use(operand)
            .and_then(|value| value.site)
            .is_some_and(|site| code.emission.site_annotation(site))
    };
    match terminal.kind {
        TerminalKind::If {
            condition: Condition::Pair { left, right, .. },
            ..
        } => displayed_operand_site(left) || displayed_operand_site(right),
        TerminalKind::If {
            condition: Condition::Zero { value, .. },
            ..
        } => displayed_operand_site(value),
        _ => true,
    }
}

fn discardable_predicate(code: &MethodCode, test: PredicateEmission) -> bool {
    let phases = &code.emission.terminals[test.terminal.0 as usize];
    if !phases.before.is_empty()
        || !phases.snapshots.is_empty()
        || !phases.after.is_empty()
        || displayed_predicate_site(code, test)
    {
        return false;
    }
    // A composed operand can own an original evaluation/exception site or a
    // deferred source span. Keep that evaluation even when both arms are empty.
    match code.terminals[test.terminal.0 as usize].kind {
        TerminalKind::If {
            condition: Condition::Pair { left, right, .. },
            ..
        } => {
            code.emission.composed_use(left).is_none()
                && code.emission.composed_use(right).is_none()
        }
        TerminalKind::If {
            condition: Condition::Zero { value, .. },
            ..
        } => code.emission.composed_use(value).is_none(),
        _ => false,
    }
}

/// A zero-step bundle still owns its original edge and a zero-length source
/// anchor. Unavailable assignments print even when there are no executable steps.
fn copies_print_nothing(code: &MethodCode, edge: TransferId) -> bool {
    code.emission.transfers[edge.0 as usize]
        .copies
        .iter()
        .all(|&id| {
            let bundle = &code.copies[id.0 as usize];
            bundle.steps.is_empty() && bundle.assignments.iter().all(|copy| copy.issue().is_none())
        })
}

/// A syntax-owned If arm prints only its copies; an ordinary terminal arm
/// additionally prints its transfer spelling.
fn arm_prints_nothing(code: &MethodCode, id: TransferId, syntax_owned: bool) -> bool {
    let transfer = &code.emission.transfers[id.0 as usize];
    transfer.copies.is_empty()
        && (syntax_owned || matches!(transfer.spelling, TransferSpelling::Fallthrough))
}

/// Whether control leaves this point unconditionally, so nothing after it can be
/// reached by falling out of the containing statement.
fn display_abrupt_end(code: &MethodCode, end: &BlockEnd) -> bool {
    match end {
        BlockEnd::None => false,
        BlockEnd::Terminal(id) => matches!(
            code.terminals[id.0 as usize].kind,
            TerminalKind::Return(_) | TerminalKind::Throw(..) | TerminalKind::Goto(_)
        ),
        BlockEnd::Transfers(transfers) => match transfers.as_slice() {
            [single] => matches!(
                code.emission.transfers[single.0 as usize].spelling,
                TransferSpelling::Goto(_)
                    | TransferSpelling::Break(_)
                    | TransferSpelling::Continue(_)
            ),
            _ => false,
        },
    }
}

fn display_empty_end(code: &MethodCode, end: &BlockEnd) -> bool {
    match end {
        BlockEnd::None => true,
        BlockEnd::Transfers(transfers) => transfers
            .iter()
            .all(|&id| arm_prints_nothing(code, id, false)),
        BlockEnd::Terminal(id) if code.emission.empty_if[id.0 as usize] => true,
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
