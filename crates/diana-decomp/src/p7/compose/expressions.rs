//! Compose a definition with a unique, adjacent use in the established layout.
//!
//! No control, storage or type solver runs here. A candidate that cannot retain
//! operand, commit, exception and copy order remains an ordinary statement.

use super::super::build::{bug, filled, push};
use super::super::*;
use crate::p2::{BranchOp, CommitRule, Effects, OpId};
use crate::p3::{StateId, UseSite, ValueId};
use crate::p4::{Family, Initialization, Interpretation};
use crate::p5::{LocalId, LocalKind, Materialization, StorageRead, StorageType};

pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let Some((reads, protected)) = super::access::local_access(code)? else {
        return Ok(());
    };

    let mut last: Vec<Option<OpId>> = filled(code.terminals.len(), None)?;
    let mut next: Vec<Option<OpId>> = filled(code.operations.len(), None)?;
    let mut incoming = filled(code.blocks.len(), 0u8)?;
    let mut sibling = filled(code.controls.len(), None)?;
    // A store may print a plain receiver before its value. Its existing pure
    // producer can move there, but further nesting could move an effect across
    // that receiver, so that composition is a boundary for earlier producers.
    let mut nested = filled(code.operations.len(), true)?;
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
        let emitted = &code.emission.controls[control.id.0 as usize];
        if matches!(
            emitted.layout,
            EmissionLayout::Sequence | EmissionLayout::Graph
        ) {
            for pair in emitted.children(control).windows(2) {
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
        let candidate = terminal_use(code, node, &last, &incoming, &sibling, &reads, &protected);
        if let Some((producer, use_id, control)) = candidate {
            install(code, producer, use_id, Some(control))?;
        }
        // Select the use nearest the end first. An earlier producer can then
        // compose into the already selected expression without moving its use.
        let count = code.emission.blocks[node].actions.len();
        for index in (0..count.saturating_sub(1)).rev() {
            let candidate = operation_use(code, node, index, &next, &reads, &protected, &nested);
            if let Some((producer, use_id, allows_nested)) = candidate {
                *nested
                    .get_mut(producer.operation.0 as usize)
                    .ok_or(bug("composed operation lost its nesting boundary"))? = allows_nested;
                install(code, producer, use_id, None)?;
            }
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
    let (expression, moving) = match statement_ref.kind {
        StatementKind::Evaluate { expression } => (expression, false),
        StatementKind::Move { expression, .. } => (expression, true),
        _ => return None,
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
    let value = match declared.original.kind() {
        LocalKind::Value(value) | LocalKind::ReferenceCarrier(value) => value,
        _ => return None,
    };
    let StorageType::Family { family, .. } = declared.original.storage_type() else {
        return None;
    };
    let boolean = match &expr.kind {
        ExprKind::Read { storage, view } if moving => {
            let origin = expr.origin.as_ref()?;
            if !compatible(*view, family)
                || !initialized(origin.initialization, family)
                || origin.site
                    != (UseSite::Operation {
                        operation,
                        argument: 0,
                    })
                || !matches!(origin.read, Ok(StorageRead::Value { local, view: read_view, .. })
                    if local.0 == storage.0 && read_view == *view)
            {
                return None;
            }
            boolean_type(code, declared, family)
                && code
                    .locals
                    .get(storage.0 as usize)
                    .is_some_and(|source| boolean_type(code, source, family))
        }
        ExprKind::Unary { value, .. }
            if !moving && family != Family::Reference && leaf(code, *value) =>
        {
            false
        }
        ExprKind::Binary { left, right, .. }
            if family != Family::Reference && leaf(code, *left) && leaf(code, *right) =>
        {
            false
        }
        // A field read keeps its original receiver evaluation and, when it may
        // throw, its original exception position. The consumer checks below
        // decide whether that position is preserved.
        ExprKind::Field { receiver, .. } if receiver.is_none_or(|r| leaf(code, r)) => false,
        ExprKind::InstanceOf { value, reference }
            if family == Family::Integral && leaf(code, *value) =>
        {
            let reference = code.references.get(reference.0 as usize)?;
            if !matches!(reference.declaration, Ok(ReferenceDeclaration::Type { .. }))
                || reference.use_issue.is_some()
            {
                return None;
            }
            true
        }
        ExprKind::Call(call)
            if !moving
                && call.receiver.is_none_or(|receiver| leaf(code, receiver))
                && matches!(&call.arguments, Arguments::Known(args) if args.iter().all(|&arg| leaf(code, arg))) =>
        {
            // Boolean shorthand still requires complete, nonempty `Z` sources;
            // an `I` call keeps its numeric comparison, and any other family
            // keeps its own interpretation. The ordered condition producer
            // decides whether that numeric value may compose as a test operand.
            boolean_type(code, declared, family)
        }
        _ => return None,
    };
    if boolean && !moving {
        // The adjacent terminal is the first consumer. The original check or
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
        // An effectful producer stays at the same execution position: one
        // delivered site with nothing saved, so the original throw route and
        // evaluation count stay unchanged.
        source.exceptional?;
        let [site] = statement_ref.sites.as_slice() else {
            return None;
        };
        let site = code.sites.get(site.0 as usize)?;
        if !matches!(site.syntax, ExceptionSyntax::Propagate) || !site.source_snapshots.is_empty() {
            return None;
        }
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
    nested: &[bool],
) -> Option<(Producer, ExprId, bool)> {
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
    if next.get(operation.0 as usize).copied().flatten()? != consumer
        || (code.emission.composed_operation(consumer)
            && !nested.get(consumer.0 as usize).copied()?)
    {
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
    // The producer preceded every original capture. It may occupy that same
    // position only when the first delivered capture is also the first operand
    // in the selected spelling. A store can print its plain receiver before its
    // value; only the existing pure-producer path can cross that local read.
    let &use_id = inputs.first()?;
    let store = match &consumer_statement.kind {
        StatementKind::FieldStore {
            receiver, value, ..
        } => *value == use_id || *receiver == Some(use_id),
        StatementKind::ArrayStore {
            array,
            index,
            value,
            ..
        } => [*array, *index, *value].contains(&use_id),
        _ => false,
    };
    if store {
        let source = code.operations.get(operation.0 as usize)?;
        if source.effects != Effects::default()
            || source.exceptional.is_some()
            || !code.statements.get(statement.0 as usize)?.sites.is_empty()
        {
            return None;
        }
    } else if first_operand(code, consumer_statement)? != use_id {
        return None;
    }
    matches_use(
        code,
        producer,
        use_id,
        UseSite::Operation {
            operation: consumer,
            argument: 0,
        },
    )
    .then_some((producer, use_id, !store))
}

fn first_operand(code: &MethodCode, statement: &Statement) -> Option<ExprId> {
    match &statement.kind {
        StatementKind::Move { expression, .. } => Some(*expression),
        StatementKind::Evaluate { expression } => {
            match &code.expressions.get(expression.0 as usize)?.kind {
                ExprKind::Unary { value, .. }
                | ExprKind::ArrayLength(value)
                | ExprKind::InstanceOf { value, .. } => Some(*value),
                ExprKind::Binary { left, .. } | ExprKind::Compare { left, .. } => Some(*left),
                ExprKind::Field { receiver, .. } => *receiver,
                ExprKind::Array { array, .. } => Some(*array),
                ExprKind::NewArray { length, .. } => Some(*length),
                ExprKind::Call(call) => call.receiver.or_else(|| match &call.arguments {
                    Arguments::Known(args) => args.first().copied(),
                    Arguments::Unavailable { .. } => None,
                }),
                _ => None,
            }
        }
        _ => None,
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
    // The producer's only normal delivery reaches this empty, single-entry
    // header in the same original block. The selected layout must print that
    // header immediately after its source; no operation or copy moves across it.
    let terminal = match following_control.layout {
        EmissionLayout::If { header, test, .. } => {
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
            test.terminal
        }
        EmissionLayout::Block(target) if target.0 as usize == next_node => {
            match code.blocks.get(next_node)?.completion {
                BlockCompletion::Syntax {
                    terminal,
                    owner: TerminalEmission::Loop(control),
                } => {
                    let EmissionLayout::Loop { placement, .. } =
                        &code.emission.controls.get(control.0 as usize)?.layout
                    else {
                        return None;
                    };
                    let test = match placement {
                        LoopPlacement::While { test, .. } | LoopPlacement::DoWhile { test, .. } => {
                            test
                        }
                        LoopPlacement::Infinite => return None,
                    };
                    if test.terminal != terminal
                        || following_control.continuation.is_some()
                        || !matches!(next.end, BlockEnd::None)
                    {
                        return None;
                    }
                    terminal
                }
                _ => {
                    let BlockEnd::Terminal(terminal) = next.end else {
                        return None;
                    };
                    if !matches!(
                        code.terminals.get(terminal.0 as usize)?.emission,
                        TerminalEmission::Block
                    ) {
                        return None;
                    }
                    terminal
                }
            }
        }
        _ => return None,
    };
    let target = code.terminals.get(terminal.0 as usize)?;
    let use_id = match &target.kind {
        TerminalKind::If {
            condition,
            when_true,
            when_false,
        } => {
            let yes = code.transfers.get(when_true.0 as usize)?;
            let no = code.transfers.get(when_false.0 as usize)?;
            if yes.destination == no.destination
                && code
                    .emission
                    .transfers
                    .get(when_true.0 as usize)?
                    .copies
                    .is_empty()
                && code
                    .emission
                    .transfers
                    .get(when_false.0 as usize)?
                    .copies
                    .is_empty()
            {
                // A predicate with no distinct delivery can disappear. Keep
                // its producer as a statement instead of creating an empty If.
                return None;
            }
            if !supported_role(code, terminal, producer.boolean, 0) {
                return None;
            }
            match condition {
                Condition::Pair { left, .. } => *left,
                Condition::Zero { value, .. } => *value,
            }
        }
        TerminalKind::Return(Some(value)) => *value,
        _ => return None,
    };
    let target = code.terminals.get(terminal.0 as usize)?;
    let phases = code.emission.terminals.get(terminal.0 as usize)?;
    if !phases.before.is_empty()
        || !phases.snapshots.is_empty()
        || !phases.after.is_empty()
        || target.site.is_some()
    {
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

fn boolean_type(code: &MethodCode, local: &LocalDeclaration, family: Family) -> bool {
    family == Family::Integral
        && !local.type_source_unknown
        && !local.declared_types.is_empty()
        && local.declared_types.iter().all(|id| {
            code.types
                .get(id.0 as usize)
                .is_some_and(|ty| ty.units == [u16::from(b'Z')])
        })
}

fn compatible(view: Interpretation, family: Family) -> bool {
    matches!(view, Interpretation::Direct(actual) if actual == family)
        || matches!(view, Interpretation::IntegralView { .. } if family == Family::Integral)
}

fn initialized(initialization: Option<Initialization>, family: Family) -> bool {
    if family == Family::Reference {
        matches!(initialization, Some(Initialization::Initialized))
    } else {
        initialization.is_none()
    }
}

fn matches_use(code: &MethodCode, producer: Producer, id: ExprId, site: UseSite) -> bool {
    if code.emission.composed_use(id).is_some() {
        return false;
    }
    let Some(expression) = code.expressions.get(id.0 as usize) else {
        return false;
    };
    let ExprKind::Read { storage, view } = expression.kind else {
        return false;
    };
    let Some(origin) = &expression.origin else {
        return false;
    };
    storage == producer.local
        && compatible(view, producer.family)
        && initialized(origin.initialization, producer.family)
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

/// Composes every delivered short-circuit test at its original evaluation
/// position. The P6 plan already fixes the test order, both original links and
/// the shared arm; this pass only decides whether each test's scheduled prelude
/// can be stated as the condition operand. A test whose prelude cannot keep its
/// facts prints the established explicit schedule instead.
pub(super) fn conditions(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let access = super::access::local_access(code)?;
    let mut layouts = Vec::new();
    for index in 0..code.emission.controls.len() {
        if matches!(
            code.emission.controls[index].layout,
            EmissionLayout::Condition { .. }
        ) {
            push(&mut layouts, index)?;
        }
    }
    for index in layouts {
        let composed = match &access {
            Some((reads, protected)) => compose(code, index, reads, protected)?,
            // An unavailable copy dependency cannot establish which locals the
            // scheduled statements define. Every dependent condition keeps its
            // complete explicit schedule; independent facts remain available.
            None => false,
        };
        if !composed {
            explicit(code, index)?;
        }
    }
    Ok(())
}

/// One test's prelude states its operands only as an ordered suffix of the run's
/// statements. The suffix order must equal the terminal's operand order, because
/// P9 evaluates a pair's left operand first. A later run prints no header of its
/// own, so every statement in it must participate in that suffix.
fn compose(
    code: &mut MethodCode,
    index: usize,
    reads: &[u8],
    protected: &[bool],
) -> Result<bool, ExpressionError> {
    let count = match &code.emission.controls[index].layout {
        EmissionLayout::Condition { tests, .. } => tests.len(),
        _ => return Ok(false),
    };
    let mut work = Vec::new();
    for position in 0..count {
        let test = {
            let EmissionLayout::Condition { tests, .. } = &code.emission.controls[index].layout
            else {
                return Ok(false);
            };
            tests[position]
        };
        let Some(&control) = code.controls[index].children.get(position) else {
            return Ok(false);
        };
        let Some(nodes) = run_nodes(code, control)? else {
            return Ok(false);
        };
        let Some(predicate) = nodes.last().copied() else {
            return Ok(false);
        };
        if code.blocks[predicate.0 as usize].original_block != test.terminal
            || !matches!(
                code.emission.blocks[predicate.0 as usize].end,
                BlockEnd::None
            )
            || {
                let transfers = &code.blocks[predicate.0 as usize].transfers;
                transfers.len() != 2
                    || !transfers.contains(&test.when_true)
                    || !transfers.contains(&test.when_false)
            }
        {
            return Ok(false);
        }
        let TerminalKind::If { condition, .. } = &code.terminals[test.terminal.0 as usize].kind
        else {
            return Ok(false);
        };
        let phases = &code.emission.terminals[test.terminal.0 as usize];
        if !phases.before.is_empty()
            || !phases.snapshots.is_empty()
            || !phases.after.is_empty()
            || code.terminals[test.terminal.0 as usize].site.is_some()
            || !code.emission.transfers[test.when_true.0 as usize]
                .copies
                .is_empty()
            || !code.emission.transfers[test.when_false.0 as usize]
                .copies
                .is_empty()
        {
            return Ok(false);
        }
        let mut operands = Vec::new();
        match condition {
            Condition::Pair { left, right, .. } => {
                push(&mut operands, (*left, 0u32))?;
                push(&mut operands, (*right, 1u32))?;
            }
            Condition::Zero { value, .. } => push(&mut operands, (*value, 0u32))?,
        }
        let Some((scheduled, all)) = scheduled(
            code,
            &nodes,
            test.terminal,
            &operands,
            reads,
            protected,
            position > 0,
        )?
        else {
            return Ok(false);
        };
        if position != 0 && !all {
            return Ok(false);
        }
        for item in scheduled {
            push(&mut work, item)?;
        }
    }
    for (producer, operand) in work {
        install(code, producer, operand, None)?;
    }
    if let EmissionLayout::Condition { tests, .. } = &code.emission.controls[index].layout {
        for test in tests {
            code.emission.transfers[test.when_true.0 as usize].spelling =
                TransferSpelling::Fallthrough;
            code.emission.transfers[test.when_false.0 as usize].spelling =
                TransferSpelling::Fallthrough;
        }
    }
    Ok(true)
}

/// A producer composes into a branch only under the established branch-role
/// policy. A boolean value is a zero test with an integral-zero meaning; it is
/// not a pair operand or a different comparison operator.
fn supported_role(
    code: &MethodCode,
    terminal: crate::p2::BlockId,
    boolean: bool,
    argument: u32,
) -> bool {
    if !boolean {
        return true;
    }
    argument == 0
        && matches!(
            code.terminals[terminal.0 as usize].kind,
            TerminalKind::If {
                condition: Condition::Zero {
                    op: BranchOp::Eqz | BranchOp::Nez,
                    meaning: ZeroMeaning::IntegralZero,
                    ..
                },
                ..
            }
        )
}

/// The block nodes of one straight test run, in original order.
fn run_nodes(
    code: &MethodCode,
    control: ControlId,
) -> Result<Option<Vec<crate::p6::NodeId>>, ExpressionError> {
    let Some(plan) = code.emission.controls.get(control.0 as usize) else {
        return Ok(None);
    };
    if plan.continuation.is_some() {
        return Ok(None);
    }
    let mut out = Vec::new();
    match plan.layout {
        EmissionLayout::Block(node) => push(&mut out, node)?,
        EmissionLayout::Sequence => {
            let Some(control) = code.controls.get(control.0 as usize) else {
                return Ok(None);
            };
            for &child in plan.children(control) {
                let Some(child) = code.emission.controls.get(child.0 as usize) else {
                    return Ok(None);
                };
                let EmissionLayout::Block(node) = child.layout else {
                    return Ok(None);
                };
                if child.continuation.is_some() {
                    return Ok(None);
                }
                push(&mut out, node)?;
            }
        }
        _ => return Ok(None),
    }
    Ok(Some(out))
}

/// The ordered producer suffix of one run and whether it covers the whole run.
type Scheduled = (Vec<(Producer, ExprId)>, bool);

/// The ordered producer suffix of one run. `all` is true when every statement
/// is a producer. Each producer must name one distinct terminal operand and the
/// matched operand positions must strictly increase, so the composed spelling
/// evaluates the original effects in their original order.
fn scheduled(
    code: &MethodCode,
    nodes: &[crate::p6::NodeId],
    terminal: crate::p2::BlockId,
    operands: &[(ExprId, u32)],
    reads: &[u8],
    protected: &[bool],
    skipped: bool,
) -> Result<Option<Scheduled>, ExpressionError> {
    let mut actions = Vec::new();
    for &node in nodes {
        let Some(block) = code.emission.blocks.get(node.0 as usize) else {
            return Ok(None);
        };
        for action in &block.actions {
            let BodyAction::Statement {
                operation,
                statement,
            } = *action
            else {
                return Ok(None);
            };
            push(&mut actions, (operation, statement))?;
        }
    }
    if !straight(code, nodes, skipped)? {
        return Ok(None);
    }
    let mut producers = Vec::new();
    for &(operation, statement) in &actions {
        let mut matched = None;
        if let Some(producer) = producer(code, operation, statement, reads, protected) {
            for &(operand, argument) in operands {
                if matches_use(
                    code,
                    producer,
                    operand,
                    UseSite::Terminal {
                        block: terminal,
                        argument,
                    },
                ) && supported_role(code, terminal, producer.boolean, argument)
                {
                    if matched.is_some() {
                        return Ok(None);
                    }
                    matched = Some((producer, operand, argument));
                }
            }
        }
        // A producer for some other use remains in this run's printed prefix;
        // it must not prevent composing an independently matched suffix.
        push(&mut producers, matched)?;
    }
    let mut start = 0;
    for (at, producer) in producers.iter().enumerate() {
        if producer.is_none() {
            start = at + 1;
        }
    }
    let all = start == 0;
    let mut last = None;
    let mut out = Vec::new();
    for producer in producers.iter().skip(start) {
        let Some((producer, operand, argument)) = *producer else {
            return Ok(None);
        };
        if last.is_some_and(|previous| argument <= previous) {
            return Ok(None);
        }
        last = Some(argument);
        push(&mut out, (producer, operand))?;
    }
    Ok(Some((out, all)))
}

/// A test run is one straight fallthrough chain: every node except the last
/// transfers only to the next listed node, and the last node owns the predicate
/// terminal. A skipped run may not carry copies whose steps would disappear.
fn straight(
    code: &MethodCode,
    nodes: &[crate::p6::NodeId],
    skipped: bool,
) -> Result<bool, ExpressionError> {
    if nodes.is_empty() {
        return Ok(false);
    }
    for window in nodes.windows(2) {
        let BlockEnd::Transfers(links) = &code.emission.blocks[window[0].0 as usize].end else {
            return Ok(false);
        };
        let [link] = links.as_slice() else {
            return Ok(false);
        };
        let Some(transfer) = code.transfers.get(link.0 as usize) else {
            return Ok(false);
        };
        if code.blocks[window[0].0 as usize].transfers.as_slice() != [*link]
            || code.labels[transfer.destination.0 as usize].node != window[1]
        {
            return Ok(false);
        }
        if skipped && !code.emission.transfers[link.0 as usize].copies.is_empty() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Selects P6's complete explicit alternative. Predicate portions, child order
/// and child completions were established before P6 publication; this consumer
/// neither walks subtrees nor changes retained source roles or continuations.
fn explicit(code: &mut MethodCode, index: usize) -> Result<(), ExpressionError> {
    let ControlKind::Condition {
        tests, explicit, ..
    } = &code.controls[index].kind
    else {
        return Err(bug("explicit condition lost its delivered plan"));
    };
    for test in tests {
        let block = code
            .blocks
            .get(test.predicate.0 as usize)
            .ok_or(bug("explicit condition lost its predicate portion"))?;
        code.emission.blocks[test.predicate.0 as usize].end =
            BlockEnd::Terminal(block.original_block);
    }
    for &link in &explicit.gotos {
        let transfer = code
            .transfers
            .get(link.0 as usize)
            .ok_or(bug("explicit condition lost an original branch edge"))?;
        code.emission.transfers[link.0 as usize].spelling =
            TransferSpelling::Goto(transfer.destination);
    }
    for completion in &explicit.completions {
        code.emission.controls[completion.child.0 as usize].continuation =
            match &completion.continuation {
                Continuation::Goto(label) => Some(ControlCompletion::Goto(*label)),
                Continuation::Break(scope) => Some(ControlCompletion::Break(*scope)),
                Continuation::Continue(scope) => Some(ControlCompletion::Continue(*scope)),
                Continuation::None | Continuation::Parent | Continuation::Next(_) => None,
            };
    }
    let children = super::super::build::copy(&explicit.children)?;
    code.emission.controls[index].explicit_children = Some(children);
    code.emission.controls[index].layout = EmissionLayout::Sequence;
    Ok(())
}
