//! Composes the established emission layout without reconstructing control flow.
//! The input is the same method's closed P7 product; no predecessor is reopened.

mod branches;
mod casts;
mod expressions;
mod loops;

use super::build::{bug, filled};
use super::*;
use crate::p2::BlockId;
use crate::p5::ThrowSite;

/// Retains only textually referenced labels and selects an ordinary loop test
/// when its selected predicate node has no work other than its condition.
pub(super) fn construct(
    code: &mut MethodCode,
    types: &crate::p4::TypeFacts,
) -> Result<(), ExpressionError> {
    // Select complete condition alternatives before generic layout and labels.
    expressions::conditions(code)?;
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
    // A catch clause prints as its `try` clause, so a chain line is only needed
    // when a printed site names it. A chain nothing points at would be a second,
    // unreferenced statement of the same routes.
    code.emission.site_annotations = site_annotations(code)?;
    for site in &code.sites {
        if !code.emission.site_annotations[site.id.0 as usize] {
            continue;
        }
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
    code.emission.show_chains = chains;
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
        // Only a printed chain line names its handler targets; otherwise the
        // catch clause that prints the route carries no label text.
        if !code.emission.show_chains[chain.id.0 as usize] {
            continue;
        }
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

    expressions::construct(code)?;
    loops::select_compact(code)?;
    natural_stores(code)?;
    casts::construct(code, types)?;
    branches::construct(code)?;
    omit_trailing_void_return(code)?;
    array_literals(code)?;
    transparent_labelled_scopes(code)?;
    Ok(())
}

/// Marks a label-free `Labelled` scope transparent when its only printed child
/// is one unnamed control. Such a scope contributes braces but no fact: the
/// original control, its labels and every transfer target stay in the product.
fn transparent_labelled_scopes(code: &mut MethodCode) -> Result<(), ExpressionError> {
    for index in 0..code.emission.controls.len() {
        let control = code
            .controls
            .get(index)
            .ok_or(bug("emission control has no source control"))?;
        let plan = code
            .emission
            .controls
            .get(index)
            .ok_or(bug("emission control is missing"))?;
        let transparent = matches!(plan.layout, EmissionLayout::Labelled)
            && !plan.label_required
            && plan.continuation.is_none()
            && !code.emission.deferred_control(control.id)
            && match control.children.as_slice() {
                [child] => code
                    .emission
                    .controls
                    .get(child.0 as usize)
                    .is_some_and(|child| !child.omitted && !child.label_required),
                _ => false,
            };
        let plan = code
            .emission
            .controls
            .get_mut(index)
            .ok_or(bug("emission control is missing"))?;
        plan.transparent = transparent;
    }
    Ok(())
}

/// By local: its read count, capped at two, and whether a copy or publication
/// touches it.
type LocalAccess = (Vec<u8>, Vec<bool>);

/// Reads and copy/publication involvement of every local in the closed method.
/// `None` when one copy assignment carries an unavailable dependency, so no
/// decision can establish which local that assignment affects.
fn local_access(code: &MethodCode) -> Result<Option<LocalAccess>, ExpressionError> {
    let mut reads = filled(code.locals.len(), 0u8)?;
    let mut protected = filled(code.locals.len(), false)?;
    for expression in &code.expressions {
        let storage = match expression.kind {
            ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } => Some(storage),
            _ => None,
        };
        if let Some(storage) = storage {
            let count = reads
                .get_mut(storage.0 as usize)
                .ok_or(bug("expression names an unknown local"))?;
            *count = count.saturating_add(1).min(2);
        }
    }
    for copy in &code.copies {
        for assignment in &copy.assignments {
            // An unavailable dependency cannot establish which local it affects.
            if assignment.issue().is_some() {
                return Ok(None);
            }
            if let Ok(destination) = assignment.destination() {
                mark(&mut protected, StorageId(destination.0))?;
            }
            if let Ok(source) = assignment.source()
                && let Some(source) = source.local()
            {
                mark(&mut protected, StorageId(source.0))?;
            }
        }
        for step in &copy.steps {
            match *step {
                CopyStatement::Save {
                    destination,
                    source,
                } => {
                    mark(&mut protected, destination)?;
                    mark(&mut protected, source)?;
                }
                CopyStatement::Assign { destination, .. } => mark(&mut protected, destination)?,
            }
        }
    }
    for group in &code.constructions {
        for publication in &group.publications {
            mark(&mut protected, publication.destination)?;
        }
    }
    Ok(Some((reads, protected)))
}

fn mark(protected: &mut [bool], id: StorageId) -> Result<(), ExpressionError> {
    *protected
        .get_mut(id.0 as usize)
        .ok_or(bug("copy names an unknown local"))? = true;
    Ok(())
}

/// An allocation immediately followed by the fill that states its whole content
/// prints that payload as its own array literal. Every other allocation keeps
/// the separate `new Element[count]` and `fill_array` statements: a display flag
/// is only ever set from facts that already decide each condition.
fn array_literals(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let Some((_, protected)) = local_access(code)? else {
        return Ok(());
    };
    let mut literals: Vec<Option<ArrayLiteral>> = filled(code.statements.len(), None)?;
    let mut folded = filled(code.statements.len(), false)?;
    for node in 0..code.emission.blocks.len() {
        let actions = &code.emission.blocks[node].actions;
        for index in 0..actions.len().saturating_sub(1) {
            let (
                BodyAction::Statement {
                    operation: allocation_operation,
                    statement: allocation,
                },
                BodyAction::Statement {
                    operation: fill_operation,
                    statement: fill,
                },
            ) = (actions[index], actions[index + 1])
            else {
                continue;
            };
            let Some(literal) = array_literal(
                code,
                &protected,
                allocation_operation,
                allocation,
                fill_operation,
                fill,
            ) else {
                continue;
            };
            *literals
                .get_mut(allocation.0 as usize)
                .ok_or(bug("statement names an unknown statement"))? = Some(literal);
            *folded
                .get_mut(fill.0 as usize)
                .ok_or(bug("statement names an unknown statement"))? = true;
        }
    }
    if !folded.contains(&true) {
        return Ok(());
    }
    code.emission.array_literals = literals;
    // The literal states the fill's whole effect, so the fill prints no text.
    // Every other action stays at its own position.
    for block in &mut code.emission.blocks {
        block.actions.retain(|action| match action {
            BodyAction::Statement { statement, .. } | BodyAction::Release { statement, .. } => {
                !folded.get(statement.0 as usize).copied().unwrap_or(false)
            }
            BodyAction::Receive { .. } => true,
        });
    }
    Ok(())
}

fn array_literal(
    code: &MethodCode,
    protected: &[bool],
    allocation_operation: crate::p2::OpId,
    allocation: StatementId,
    fill_operation: crate::p2::OpId,
    fill: StatementId,
) -> Option<ArrayLiteral> {
    let operation = code.operations.get(allocation_operation.0 as usize)?;
    if !matches!(operation.emission, Emission::Block { .. })
        || code.emission.composed_operation(allocation_operation)
    {
        return None;
    }
    let allocation = code.statements.get(allocation.0 as usize)?;
    let StatementKind::Evaluate { expression } = allocation.kind else {
        return None;
    };
    let ExprKind::NewArray { reference, length } =
        code.expressions.get(expression.0 as usize)?.kind
    else {
        return None;
    };
    // The array local is the allocation's own committed result, not a copy, an
    // alias, a phi or a snapshot of it.
    let [definition] = allocation.outputs.as_slice() else {
        return None;
    };
    let crate::p5::Materialization::Store(destination) = definition.materialization() else {
        return None;
    };
    if definition.commit() != crate::p2::CommitRule::AtOperation {
        return None;
    }
    let storage = StorageId(destination.0);
    let declared = code.locals.get(storage.0 as usize)?;
    // One SSA value: a parameter, an uninitialized carrier, a saved snapshot or a
    // copy temporary would make the local something other than this allocation.
    // No copy or publication may observe it either, because such a step prints
    // after the allocation and before the fill.
    if declared.gap.is_some()
        || !matches!(
            declared.original.kind(),
            crate::p5::LocalKind::Value(_) | crate::p5::LocalKind::ReferenceCarrier(_)
        )
        || protected[storage.0 as usize]
    {
        return None;
    }
    // The fill writes only that array and prints nothing else: no definition, no
    // printed exception marker and no copy phase of its own. A site that prints no
    // text states a route already visible in the enclosing chains and catches.
    let fill_statement = code.statements.get(fill.0 as usize)?;
    let StatementKind::FillArray {
        array,
        payload: payload_id,
    } = fill_statement.kind
    else {
        return None;
    };
    let phases = code.emission.operations.get(fill_operation.0 as usize)?;
    if !fill_statement.outputs.is_empty()
        || fill_statement
            .sites
            .iter()
            .any(|&site| code.emission.site_annotation(site))
        || !phases.before.is_empty()
        || !phases.snapshots.is_empty()
        || !phases.after.is_empty()
    {
        return None;
    }
    // The fill's only read of that local is its array operand, and it prints the
    // local itself rather than a composed producer.
    if !matches!(
        code.expressions.get(array.0 as usize)?.kind,
        ExprKind::Read { storage: read, .. } if read == storage
    ) || code.emission.composed_use(array).is_some()
    {
        return None;
    }
    let payload = code.payloads.get(payload_id.0 as usize)?;
    // One dimension of the payload's own element type, allocated with the
    // payload's exact element count.
    let Ok(ReferenceDeclaration::Type { type_use }) =
        &code.references.get(reference.0 as usize)?.declaration
    else {
        return None;
    };
    let Ok(element) = payload.element else {
        return None;
    };
    let units = [u16::from(b'['), u16::from(element_descriptor(element))];
    if code.types.get(type_use.0 as usize)?.units != units {
        return None;
    }
    let ExprKind::Literal { bits, .. } = code.expressions.get(length.0 as usize)?.kind else {
        return None;
    };
    if bits.width != crate::SlotWidth::One
        || bits.value != u64::from(payload.count)
        || usize::try_from(payload.count).ok()? != payload.bits.len()
        || !exact_elements(element, &payload.bits)
    {
        return None;
    }
    Some(ArrayLiteral {
        payload: payload_id,
        fill,
        operation: fill_operation,
        pc: code.operations.get(fill_operation.0 as usize)?.pc,
    })
}

/// The descriptor letter of one primitive array element type.
fn element_descriptor(element: crate::p4::ArrayElementType) -> u8 {
    match element {
        crate::p4::ArrayElementType::Boolean => b'Z',
        crate::p4::ArrayElementType::Byte => b'B',
        crate::p4::ArrayElementType::Char => b'C',
        crate::p4::ArrayElementType::Short => b'S',
        crate::p4::ArrayElementType::Int => b'I',
        crate::p4::ArrayElementType::Long => b'J',
        crate::p4::ArrayElementType::Float => b'F',
        crate::p4::ArrayElementType::Double => b'D',
    }
}

/// Whether every element has an exact Java literal spelling. P9 prints these
/// elements with the bit-preserving forms the ordinary literal printer uses: the
/// shortest decimal for a finite float already round-trips to the same bits,
/// while NaN and either infinity have no finite decimal spelling at all.
fn exact_elements(element: crate::p4::ArrayElementType, bits: &[u64]) -> bool {
    use crate::p4::ArrayElementType::*;
    match element {
        Boolean => bits.iter().all(|&value| value <= 1),
        Float => bits
            .iter()
            .all(|&value| f32::from_bits(value as u32).is_finite()),
        Double => bits.iter().all(|&value| f64::from_bits(value).is_finite()),
        Byte | Char | Short | Int | Long => true,
    }
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
        if printed_entry_is(code, next, label) {
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
        if printed_entry_is(code, next, label) {
            code.emission.controls[index].continuation = None;
        }
    }
    Ok(())
}

/// Follow only layouts that print their established entry before any control
/// action. Entering a loop, protected scope or graph can have different effects
/// from jumping to a block inside it, so those boundaries remain explicit.
fn printed_entry_is(code: &MethodCode, mut current: ControlId, target: LabelId) -> bool {
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
fn printed_parts<'a>(
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

/// A void method's closing brace performs the return a bare `return;` states:
/// nothing after it runs and no value is produced. Only the last printed action
/// of the whole method body qualifies; a return inside an arm, a loop or any
/// other scope that still prints after it stays explicit.
fn omit_trailing_void_return(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let Some(terminal) = trailing_void_return(code)? else {
        return Ok(());
    };
    let slot = code
        .emission
        .omitted_returns
        .get_mut(terminal.0 as usize)
        .ok_or(bug("trailing return names an unknown terminal"))?;
    *slot = true;
    Ok(())
}

/// The terminal that prints last in the method body, following only containers
/// that add nothing around their children.
fn trailing_void_return(code: &MethodCode) -> Result<Option<BlockId>, ExpressionError> {
    let Some(root) = code.root else {
        return Ok(None);
    };
    let mut current = root;
    for _ in 0..=code.emission.controls.len() {
        let control = code
            .emission
            .controls
            .get(current.0 as usize)
            .ok_or(bug("trailing return names an unknown control"))?;
        // A planned continuation prints its `goto` after this control's children.
        if control.continuation.is_some() {
            return Ok(None);
        }
        let block = match &control.layout {
            // An unlabeled Sequence prints nothing around its children.
            EmissionLayout::Sequence if !control.label_required => {
                let source = code
                    .controls
                    .get(current.0 as usize)
                    .ok_or(bug("trailing return names an unknown control"))?;
                let children = control.children(source);
                let Some(last) = children
                    .iter()
                    .rev()
                    .find(|&&child| !code.emission.omitted(child))
                else {
                    return Ok(None);
                };
                current = *last;
                continue;
            }
            EmissionLayout::Block(node) => code
                .emission
                .blocks
                .get(node.0 as usize)
                .ok_or(bug("trailing return names an unknown block"))?,
            // Every other scope prints a closing brace, a predicate or a catch
            // clause after its children, so nothing inside it ends the body.
            _ => return Ok(None),
        };
        return trailing_return_of(code, block);
    }
    Ok(None)
}

/// The return printing a block's end, when it is a bare void return that prints
/// nothing besides the `return;` text and that no label is waiting on.
fn trailing_return_of(
    code: &MethodCode,
    block: &EmissionBlock,
) -> Result<Option<BlockId>, ExpressionError> {
    if block.show_label {
        return Ok(None);
    }
    let BlockEnd::Terminal(terminal) = block.end else {
        return Ok(None);
    };
    let source = code
        .terminals
        .get(terminal.0 as usize)
        .ok_or(bug("block end names an unknown terminal"))?;
    if !matches!(source.kind, TerminalKind::Return(None)) || source.site.is_some() {
        return Ok(None);
    }
    let phases = code
        .emission
        .terminals
        .get(terminal.0 as usize)
        .ok_or(bug("trailing return lost its terminal phases"))?;
    if !phases.before.is_empty() || !phases.snapshots.is_empty() || !phases.after.is_empty() {
        return Ok(None);
    }
    Ok(Some(terminal))
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
