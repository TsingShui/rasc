//! Compose a definition with a unique, adjacent use in the established layout.
//!
//! No control, storage or type solver runs here. A candidate that cannot retain
//! operand, commit, exception and copy order remains an ordinary statement.

use super::super::build::{bug, filled};
use super::super::*;
use crate::p2::{BranchOp, CommitRule, Effects, OpId};
use crate::p3::{StateId, UseSite, ValueId};
use crate::p4::{Family, Interpretation};
use crate::p5::{LocalId, LocalKind, Materialization, StorageRead, StorageType};

pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let Some((reads, protected)) = super::local_access(code)? else {
        return Ok(());
    };

    let mut last: Vec<Option<OpId>> = filled(code.terminals.len(), None)?;
    let mut next: Vec<Option<OpId>> = filled(code.operations.len(), None)?;
    let mut incoming = filled(code.blocks.len(), 0u8)?;
    let mut sibling = filled(code.controls.len(), None)?;
    for operation in &code.operations {
        let previous = last
            .get_mut(operation.original_block.0 as usize)
            .ok_or(bug("operation names an unknown original block"))?;
        if let Some(previous) = *previous {
            *next
                .get_mut(previous.0 as usize)
                .ok_or(bug("operation lost its original successor"))? = Some(operation.source);
        }
        *previous = Some(operation.source);
    }
    for transfer in &code.transfers {
        let count = incoming
            .get_mut(transfer.destination.0 as usize)
            .ok_or(bug("transfer names an unknown node"))?;
        *count = count.saturating_add(1).min(2);
    }
    for control in &code.controls {
        if matches!(control.kind, ControlKind::Sequence) {
            for pair in control.children.windows(2) {
                let next = sibling
                    .get_mut(pair[0].0 as usize)
                    .ok_or(bug("sequence names an unknown child"))?;
                if next.replace(pair[1]).is_some() {
                    return Err(bug("region child has multiple sequence owners"));
                }
            }
        }
    }
    for node in 0..code.emission.blocks.len() {
        let count = code.emission.blocks[node].actions.len();
        for index in 0..count.saturating_sub(1) {
            let candidate = operation_use(code, node, index, &next, &reads, &protected);
            if let Some((producer, use_id)) = candidate {
                install(code, producer, use_id, None)?;
            }
        }
        let candidate = terminal_use(code, node, &last, &incoming, &sibling, &reads, &protected);
        if let Some((producer, use_id, control)) = candidate {
            install(code, producer, use_id, Some(control))?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Producer {
    operation: OpId,
    statement: StatementId,
    expression: ExprId,
    local: StorageId,
    destination: LocalId,
    value: ValueId,
    state: StateId,
    family: Family,
    boolean: bool,
    site: Option<SiteId>,
}

fn producer(
    code: &MethodCode,
    operation: OpId,
    statement: StatementId,
    reads: &[u8],
    protected: &[bool],
) -> Option<Producer> {
    let source = code.operations.get(operation.0 as usize)?;
    if code.emission.composed_operation(operation)
        || !matches!(source.emission, Emission::Block { .. })
    {
        return None;
    }
    let phase = code.emission.operations.get(operation.0 as usize)?;
    if !phase.before.is_empty() || !phase.snapshots.is_empty() || !phase.after.is_empty() {
        return None;
    }
    let statement_ref = code.statements.get(statement.0 as usize)?;
    let StatementKind::Evaluate { expression } = statement_ref.kind else {
        return None;
    };
    if statement_ref.outputs.len() != 1 {
        return None;
    }
    let definition = &statement_ref.outputs[0];
    let Materialization::Store(destination) = definition.materialization() else {
        return None;
    };
    let expr = code.expressions.get(expression.0 as usize)?;
    // Calls commit at their delivered move-result; adjacency below keeps that
    // commit before the branch, and install retains its result_pc origin.
    let commit = if matches!(expr.kind, ExprKind::Call(_)) {
        CommitRule::AfterResult
    } else {
        CommitRule::AtOperation
    };
    if definition.commit() != commit {
        return None;
    }
    let local = StorageId(destination.0);
    let declared = code.locals.get(local.0 as usize)?;
    if declared.gap.is_some() || protected[local.0 as usize] || reads[local.0 as usize] != 1 {
        return None;
    }
    let LocalKind::Value(value) = declared.original.kind() else {
        return None;
    };
    let StorageType::Family { family, .. } = declared.original.storage_type() else {
        return None;
    };
    if matches!(family, Family::Reference) {
        return None;
    }
    let boolean = match &expr.kind {
        ExprKind::Unary { value, .. } if leaf(code, *value) => false,
        ExprKind::Binary { left, right, .. } if leaf(code, *left) && leaf(code, *right) => false,
        ExprKind::InstanceOf { value, reference } if leaf(code, *value) => {
            let reference = code.references.get(reference.0 as usize)?;
            if !matches!(reference.declaration, Ok(ReferenceDeclaration::Type { .. }))
                || reference.use_issue.is_some()
            {
                return None;
            }
            true
        }
        ExprKind::Call(call)
            if call.receiver.is_none_or(|receiver| leaf(code, receiver))
                && matches!(&call.arguments, Arguments::Known(args) if args.iter().all(|&arg| leaf(code, arg)))
                && family == Family::Integral
                && !declared.type_source_unknown
                && !declared.declared_types.is_empty()
                && declared.declared_types.iter().all(|id| {
                    code.types
                        .get(id.0 as usize)
                        .is_some_and(|ty| ty.units == [u16::from(b'Z')])
                }) =>
        {
            true
        }
        _ => return None,
    };
    if boolean {
        // The adjacent branch is the first consumer. The original check or
        // call can throw, so only an uncaught route without saved state is eligible.
        if !matches!(
            source.original,
            crate::p2::OpKind::InstanceOf { .. }
                | crate::p2::OpKind::Invoke { .. }
                | crate::p2::OpKind::UnresolvedInvoke { .. }
        ) || !source.effects.may_throw()
            || source.exceptional.is_none()
            || statement_ref.sites.len() != 1
        {
            return None;
        }
        let site = code.sites.get(statement_ref.sites[0].0 as usize)?;
        if !matches!(site.syntax, ExceptionSyntax::Propagate) || !site.source_snapshots.is_empty() {
            return None;
        }
    } else if source.effects != Effects::default()
        || source.exceptional.is_some()
        || !statement_ref.sites.is_empty()
    {
        return None;
    }
    Some(Producer {
        operation,
        statement,
        expression,
        local,
        destination,
        value,
        state: definition.state(),
        family,
        boolean,
        site: statement_ref.sites.first().copied(),
    })
}

fn operation_use(
    code: &MethodCode,
    node: usize,
    index: usize,
    next: &[Option<OpId>],
    reads: &[u8],
    protected: &[bool],
) -> Option<(Producer, ExprId)> {
    let block = code.emission.blocks.get(node)?;
    let BodyAction::Statement {
        operation,
        statement,
    } = *block.actions.get(index)?
    else {
        return None;
    };
    let BodyAction::Statement {
        operation: consumer,
        statement: consumer_statement,
    } = *block.actions.get(index + 1)?
    else {
        return None;
    };
    if next.get(operation.0 as usize).copied().flatten()? != consumer {
        return None;
    }
    let producer = producer(code, operation, statement, reads, protected)?;
    if producer.boolean
        || !code
            .emission
            .operations
            .get(consumer.0 as usize)?
            .before
            .is_empty()
    {
        return None;
    }
    let consumer_statement = code.statements.get(consumer_statement.0 as usize)?;
    let Arguments::Known(inputs) = &consumer_statement.inputs else {
        return None;
    };
    // The producer's original reads precede every consumer operand capture.
    // Substituting a later argument would reverse that original read order.
    for (argument, &use_id) in inputs.iter().enumerate() {
        let argument = u32::try_from(argument).ok()?;
        if argument == 0
            && rendered_operand(code, consumer_statement, use_id)
            && matches_use(
                code,
                producer,
                use_id,
                UseSite::Operation {
                    operation: consumer,
                    argument,
                },
            )
        {
            return Some((producer, use_id));
        }
    }
    None
}

fn rendered_operand(code: &MethodCode, statement: &Statement, id: ExprId) -> bool {
    match &statement.kind {
        StatementKind::Move { expression, .. } => *expression == id,
        StatementKind::ArrayStore {
            array,
            index,
            value,
            ..
        } => [*value, *array, *index].contains(&id),
        StatementKind::FieldStore {
            receiver, value, ..
        } => *value == id || *receiver == Some(id),
        StatementKind::Evaluate { expression } => {
            let Some(expr) = code.expressions.get(expression.0 as usize) else {
                return false;
            };
            match &expr.kind {
                ExprKind::Unary { value, .. } | ExprKind::ArrayLength(value) => *value == id,
                ExprKind::Binary { left, right, .. } | ExprKind::Compare { left, right, .. } => {
                    *left == id || *right == id
                }
                ExprKind::Array { array, index, .. } => *array == id || *index == id,
                ExprKind::NewArray { length, .. } => *length == id,
                ExprKind::Call(call) => {
                    call.receiver == Some(id)
                        || matches!(&call.arguments, Arguments::Known(args) if args.contains(&id))
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn terminal_use(
    code: &MethodCode,
    node: usize,
    last: &[Option<OpId>],
    incoming: &[u8],
    sibling: &[Option<ControlId>],
    reads: &[u8],
    protected: &[bool],
) -> Option<(Producer, ExprId, ControlId)> {
    let block = code.emission.blocks.get(node)?;
    let BodyAction::Statement {
        operation,
        statement,
    } = *block.actions.last()?
    else {
        return None;
    };
    let source = code.operations.get(operation.0 as usize)?;
    if last
        .get(source.original_block.0 as usize)
        .copied()
        .flatten()?
        != operation
    {
        return None;
    }
    let producer = producer(code, operation, statement, reads, protected)?;
    let BlockEnd::Transfers(links) = &block.end else {
        return None;
    };
    let [link] = links.as_slice() else {
        return None;
    };
    let transfer = code.transfers.get(link.0 as usize)?;
    let emitted = code.emission.transfers.get(link.0 as usize)?;
    if transfer.edge.is_some()
        || !matches!(emitted.spelling, TransferSpelling::Fallthrough)
        || !emitted.copies.is_empty()
    {
        return None;
    }
    let next_node = transfer.destination.0 as usize;
    let next = code.emission.blocks.get(next_node)?;
    if next.show_label
        || !next.actions.is_empty()
        || incoming.get(next_node).copied()? != 1
        || code.blocks.get(next_node)?.original_block != source.original_block
    {
        return None;
    }
    let owner = code.labels.get(node)?.owner;
    let source_control = code.emission.controls.get(owner.0 as usize)?;
    if !matches!(source_control.layout, EmissionLayout::Block(n) if n.0 as usize == node)
        || source_control.continuation.is_some()
    {
        return None;
    }
    let following = sibling.get(owner.0 as usize).copied().flatten()?;
    let following_control = code.emission.controls.get(following.0 as usize)?;
    let (terminal, use_id) = if !producer.boolean {
        let EmissionLayout::Block(target) = following_control.layout else {
            return None;
        };
        if target.0 as usize != next_node {
            return None;
        }
        let BlockEnd::Terminal(terminal) = next.end else {
            return None;
        };
        let target = code.terminals.get(terminal.0 as usize)?;
        if !matches!(target.emission, TerminalEmission::Block) {
            return None;
        }
        let TerminalKind::Return(Some(use_id)) = target.kind else {
            return None;
        };
        (terminal, use_id)
    } else {
        // The producer is the last original operation; its sole fallthrough
        // reaches this empty, single-entry header with no copies or phases.
        // Thus even an effectful call stays at the same evaluation position.
        let EmissionLayout::If { header, test, .. } = following_control.layout else {
            return None;
        };
        let header = code.emission.controls.get(header.0 as usize)?;
        if !matches!(header.layout, EmissionLayout::Block(target) if target.0 as usize == next_node)
            || header.continuation.is_some()
            || !matches!(next.end, BlockEnd::None)
        {
            return None;
        }
        let target = code.terminals.get(test.terminal.0 as usize)?;
        if !matches!(target.emission, TerminalEmission::If(control) if control == following) {
            return None;
        }
        let TerminalKind::If {
            condition:
                Condition::Zero {
                    op: BranchOp::Eqz | BranchOp::Nez,
                    value: use_id,
                    meaning: ZeroMeaning::IntegralZero,
                },
            ..
        } = target.kind
        else {
            return None;
        };
        (test.terminal, use_id)
    };
    let target = code.terminals.get(terminal.0 as usize)?;
    let phases = code.emission.terminals.get(terminal.0 as usize)?;
    if !phases.before.is_empty() || !phases.snapshots.is_empty() || target.site.is_some() {
        return None;
    }
    matches_use(
        code,
        producer,
        use_id,
        UseSite::Terminal {
            block: terminal,
            argument: 0,
        },
    )
    .then_some((producer, use_id, owner))
}

fn matches_use(code: &MethodCode, producer: Producer, id: ExprId, site: UseSite) -> bool {
    let Some(expression) = code.expressions.get(id.0 as usize) else {
        return false;
    };
    let ExprKind::Read { storage, view } = expression.kind else {
        return false;
    };
    let compatible = matches!(view, Interpretation::Direct(actual) if actual == producer.family)
        || matches!(view, Interpretation::IntegralView { .. } if producer.family == Family::Integral);
    let Some(origin) = &expression.origin else {
        return false;
    };
    storage == producer.local
        && compatible
        && origin.initialization.is_none()
        && origin.site == site
        && matches!(origin.read, Ok(StorageRead::Value { local, value, state, .. })
            if local == producer.destination && value == producer.value && state == producer.state)
}

fn install(
    code: &mut MethodCode,
    producer: Producer,
    use_id: ExprId,
    source_control: Option<ControlId>,
) -> Result<(), ExpressionError> {
    let source = code
        .operations
        .get(producer.operation.0 as usize)
        .ok_or(bug("composed operation disappeared"))?;
    code.emission.composed_uses[use_id.0 as usize] = Some(ComposedExpression {
        expression: producer.expression,
        operation: producer.operation,
        statement: producer.statement,
        pc: source.pc,
        result_pc: source.result_pc,
        site: producer.site,
        boolean: producer.boolean,
        source_control,
    });
    if let Some(control) = source_control {
        let deferred = code
            .emission
            .deferred_controls
            .get_mut(control.0 as usize)
            .ok_or(bug("composed operation lost its control"))?;
        if *deferred {
            return Err(bug("control has multiple deferred compositions"));
        }
        *deferred = true;
    }
    code.emission.composed_operations[producer.operation.0 as usize] = true;
    code.emission.hidden_locals[producer.local.0 as usize] = true;
    Ok(())
}

fn leaf(code: &MethodCode, id: ExprId) -> bool {
    code.emission.composed_use(id).is_none()
        && matches!(
            code.expressions.get(id.0 as usize).map(|v| &v.kind),
            Some(ExprKind::Read { .. } | ExprKind::Literal { .. })
        )
}
