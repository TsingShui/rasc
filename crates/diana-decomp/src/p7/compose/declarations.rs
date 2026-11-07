//! Declaration placement over the selected display layout. Initializers retain
//! their original statement, evaluation, exception and commit positions.

use super::super::build::{bug, copy, filled, push};
use super::super::*;
use crate::p2::OpId;
use crate::p3::UseSite;
use crate::p5::{CopyLocation, LocalKind, Materialization, ThrowSite, TransferSite};
use crate::p6::NodeId;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct ScopeId(usize);
struct Scope {
    end: usize,
}
#[derive(Clone, Copy)]
struct Place {
    scope: ScopeId,
    order: usize,
    declarations: bool,
}
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Step {
    Operation(OpId, bool),
    Terminal,
}
#[derive(Clone, Copy)]
struct Point {
    place: Place,
    step: Step,
}
struct Layout {
    scopes: Vec<Scope>,
    nodes: Vec<Option<Place>>,
    terminals: Vec<Option<Point>>,
}
enum Event {
    Control(ControlId, ScopeId, bool),
    Nested(ControlId, bool),
    Predicate(NodeId, ScopeId),
    End(ScopeId),
}

impl Layout {
    fn construct(code: &MethodCode) -> Result<Self, ExpressionError> {
        let mut layout = Self {
            scopes: Vec::new(),
            nodes: filled(code.blocks.len(), None)?,
            terminals: filled(code.terminals.len(), None)?,
        };
        push(&mut layout.scopes, Scope { end: 0 })?;
        let mut work = Vec::new();
        let mut order = 0usize;
        if let Some(root) = code.root {
            push(&mut work, Event::Control(root, ScopeId(0), true))?;
        }
        while let Some(event) = work.pop() {
            let (id, scope, declarations) = match event {
                Event::End(scope) => {
                    layout.scopes[scope.0].end = layout.scopes.len();
                    continue;
                }
                Event::Predicate(node, scope) => {
                    layout.node(node, scope, false, &mut order)?;
                    continue;
                }
                Event::Nested(id, declarations) => {
                    let scope = ScopeId(layout.scopes.len());
                    push(&mut layout.scopes, Scope { end: 0 })?;
                    push(&mut work, Event::End(scope))?;
                    (id, scope, declarations)
                }
                Event::Control(id, scope, declarations) => (id, scope, declarations),
            };
            let control = &code.controls[id.0 as usize];
            let emission = &code.emission.controls[id.0 as usize];
            if emission.omitted {
                continue;
            }
            match &emission.layout {
                EmissionLayout::Block(node) => {
                    layout.node(*node, scope, declarations, &mut order)?
                }
                EmissionLayout::Graph => {
                    // Explicit entries and jumps do not establish lexical first assignment.
                }
                EmissionLayout::Sequence | EmissionLayout::Labelled => {
                    let nested = emission.label_required
                        || matches!(emission.layout, EmissionLayout::Labelled)
                            && !emission.transparent;
                    if nested {
                        let nested_scope = ScopeId(layout.scopes.len());
                        push(&mut layout.scopes, Scope { end: 0 })?;
                        push(&mut work, Event::End(nested_scope))?;
                        for &child in emission.children(control).iter().rev() {
                            push(&mut work, Event::Control(child, nested_scope, declarations))?;
                        }
                    } else {
                        for &child in emission.children(control).iter().rev() {
                            push(&mut work, Event::Control(child, scope, declarations))?;
                        }
                    }
                }
                EmissionLayout::If {
                    header, yes, no, ..
                } => {
                    push(&mut work, Event::Nested(*no, declarations))?;
                    push(&mut work, Event::Nested(*yes, declarations))?;
                    push(&mut work, Event::Control(*header, scope, declarations))?;
                }
                EmissionLayout::Condition {
                    header, yes, no, ..
                } => {
                    if let ConditionElse::Body(no) = no {
                        push(&mut work, Event::Nested(*no, declarations))?;
                    }
                    push(&mut work, Event::Nested(*yes, declarations))?;
                    if let ControlKind::Condition { tests, .. } = &control.kind {
                        for test in tests.iter().rev().take(tests.len().saturating_sub(1)) {
                            push(&mut work, Event::Predicate(test.predicate, scope))?;
                        }
                    }
                    push(&mut work, Event::Control(*header, scope, declarations))?;
                }
                EmissionLayout::When { bodies } => {
                    for &body in bodies.iter().rev() {
                        push(&mut work, Event::Nested(body, declarations))?;
                    }
                    if let Some(decision) = &control.decision {
                        for arm in decision.arms.iter().rev() {
                            push(&mut work, Event::Predicate(arm.predicate, scope))?;
                        }
                    }
                }
                EmissionLayout::Loop { body, placement } => match placement {
                    LoopPlacement::Infinite => push(&mut work, Event::Nested(*body, declarations))?,
                    LoopPlacement::While { predicate, .. } => {
                        push(&mut work, Event::Nested(*body, declarations))?;
                        push(&mut work, Event::Nested(*predicate, false))?;
                    }
                    LoopPlacement::DoWhile { predicate, .. } => {
                        push(&mut work, Event::Nested(*predicate, false))?;
                        push(&mut work, Event::Nested(*body, declarations))?;
                    }
                },
                EmissionLayout::Switch { header, bodies, .. } => {
                    for &body in bodies.iter().rev() {
                        // Separate case scopes are conservative even for shared case blocks.
                        push(&mut work, Event::Nested(body, declarations))?;
                    }
                    push(&mut work, Event::Control(*header, scope, declarations))?;
                }
                EmissionLayout::Try { body, catches } => {
                    for &catch in catches.iter().rev() {
                        push(&mut work, Event::Nested(catch, declarations))?;
                    }
                    push(&mut work, Event::Nested(*body, declarations))?;
                }
                EmissionLayout::Synchronized { entry, body, .. } => {
                    push(&mut work, Event::Nested(*body, declarations))?;
                    push(&mut work, Event::Control(*entry, scope, declarations))?;
                }
            }
        }
        layout.scopes[0].end = layout.scopes.len();
        for block in &code.blocks {
            if let Some(place) = layout.nodes[block.node.0 as usize] {
                layout.terminals[block.original_block.0 as usize] = Some(Point {
                    place,
                    step: Step::Terminal,
                });
            }
        }
        Ok(layout)
    }

    fn node(
        &mut self,
        node: NodeId,
        scope: ScopeId,
        declarations: bool,
        order: &mut usize,
    ) -> Result<(), ExpressionError> {
        let place = self
            .nodes
            .get_mut(node.0 as usize)
            .ok_or(bug("declaration layout lost its block"))?;
        if place.is_none() {
            *place = Some(Place {
                scope,
                order: *order,
                declarations,
            });
            *order = order
                .checked_add(1)
                .ok_or(crate::RequestError::PlatformCapacity)?;
        }
        Ok(())
    }

    fn operation(&self, code: &MethodCode, operation: OpId, after: bool) -> Option<Point> {
        let node = code.operations[operation.0 as usize].node;
        self.nodes[node.0 as usize].map(|place| Point {
            place,
            step: Step::Operation(operation, after),
        })
    }

    fn covers(&self, declaration: Point, read: Point) -> bool {
        let scope = declaration.place.scope.0;
        scope <= read.place.scope.0
            && read.place.scope.0 < self.scopes[scope].end
            && (declaration.place.order, declaration.step) <= (read.place.order, read.step)
    }
}

#[derive(Clone, Copy)]
enum Resolution {
    Pending,
    Active,
    Done(Option<Point>),
}

// Follow only the already selected composition links, with memoized locations.
fn operation_locations(
    code: &MethodCode,
    layout: &Layout,
) -> Result<Vec<Resolution>, ExpressionError> {
    let mut moved = filled(code.operations.len(), None)?;
    for (index, composed) in code.emission.composed_uses.iter().enumerate() {
        if let Some(composed) = composed {
            moved[composed.operation.0 as usize] = code.expressions[index]
                .origin
                .as_ref()
                .map(|origin| origin.site);
        }
    }
    for group in &code.constructions {
        for prepared in &group.prepared_arguments {
            moved[prepared.operation.0 as usize] = Some(UseSite::Operation {
                operation: group.initialization,
                argument: 0,
            });
        }
    }
    let mut locations = Vec::new();
    for operation in &code.operations {
        push(
            &mut locations,
            if code.emission.composed_operation(operation.source) {
                Resolution::Pending
            } else {
                Resolution::Done(layout.operation(code, operation.source, false))
            },
        )?;
    }
    let mut path = Vec::new();
    for index in 0..locations.len() {
        let mut current = index;
        let point = loop {
            match locations[current] {
                Resolution::Done(point) => break point,
                Resolution::Active => {
                    return Err(bug("declaration location has a cyclic composition"));
                }
                Resolution::Pending => {
                    locations[current] = Resolution::Active;
                    push(&mut path, current)?;
                    match moved[current] {
                        Some(UseSite::Operation { operation, .. }) => {
                            current = operation.0 as usize
                        }
                        Some(UseSite::Terminal { block, .. }) => {
                            break layout.terminals[block.0 as usize];
                        }
                        None => break None,
                    }
                }
            }
        };
        for index in path.drain(..) {
            locations[index] = Resolution::Done(point);
        }
    }
    Ok(locations)
}

fn site_point(site: UseSite, operations: &[Resolution], layout: &Layout) -> Option<Point> {
    match site {
        UseSite::Operation { operation, .. } => match operations[operation.0 as usize] {
            Resolution::Done(point) => point,
            _ => None,
        },
        UseSite::Terminal { block, .. } => layout.terminals[block.0 as usize],
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    statement: StatementId,
    point: Point,
}

fn destination(materialization: Materialization) -> Option<StorageId> {
    match materialization {
        Materialization::Store(local)
        | Materialization::AllocationCarrier(local)
        | Materialization::ExceptionCapture(local)
        | Materialization::Deferred { local, .. } => Some(StorageId(local.0)),
        _ => None,
    }
}

fn access(
    local: StorageId,
    point: Option<Point>,
    candidates: &[Option<Candidate>],
    eligible: &mut [bool],
    layout: &Layout,
) {
    if let Some(candidate) = candidates[local.0 as usize]
        && point.is_none_or(|point| !layout.covers(candidate.point, point))
    {
        eligible[local.0 as usize] = false;
    }
}

pub(super) fn construct(code: &mut MethodCode) -> Result<(), ExpressionError> {
    let layout = Layout::construct(code)?;
    let operations = operation_locations(code, &layout)?;
    let mut statement_operations = filled(code.statements.len(), None)?;
    for block in &code.emission.blocks {
        for action in &block.actions {
            if let BodyAction::Statement {
                operation,
                statement,
            } = *action
                && !code.emission.composed_operation(operation)
                && !code.emission.omitted_literal(statement)
            {
                statement_operations[statement.0 as usize] = Some(operation);
            }
        }
    }
    let mut candidates: Vec<Option<Candidate>> = filled(code.locals.len(), None)?;
    let mut eligible = filled(code.locals.len(), true)?;
    for statement in &code.statements {
        let [output] = statement.outputs.as_slice() else {
            continue;
        };
        let local = match output.materialization() {
            Materialization::Store(local) => StorageId(local.0),
            Materialization::Deferred { local, .. } => {
                let StatementKind::Construction(group) = statement.kind else {
                    continue;
                };
                let [publication] = code.constructions[group.0 as usize].publications.as_slice()
                else {
                    continue;
                };
                if publication.destination != StorageId(local.0) {
                    continue;
                }
                StorageId(local.0)
            }
            _ => continue,
        };
        if !matches!(
            statement.kind,
            StatementKind::Move { .. }
                | StatementKind::Evaluate { .. }
                | StatementKind::Construction(_)
        ) || code.emission.allocation_declaration(statement.id)
        {
            continue;
        }
        let Some(operation) = statement_operations[statement.id.0 as usize] else {
            continue;
        };
        let Some(point) = layout.operation(code, operation, true) else {
            continue;
        };
        if point.place.declarations
            && candidates[local.0 as usize].is_none_or(|previous| {
                (point.place.order, point.step) < (previous.point.place.order, previous.point.step)
            })
        {
            candidates[local.0 as usize] = Some(Candidate {
                statement: statement.id,
                point,
            });
        }
    }
    // Later writes must see the declaration too. An earlier or out-of-scope
    // write prevents moving the declaration to a candidate assignment.
    for statement in &code.statements {
        let operation = statement_operations[statement.id.0 as usize]
            .or_else(|| code.emission.statement_operation(statement.id));
        let point = operation.and_then(|operation| {
            if code.emission.composed_operation(operation) {
                site_point(
                    UseSite::Operation {
                        operation,
                        argument: 0,
                    },
                    &operations,
                    &layout,
                )
            } else {
                layout.operation(code, operation, true)
            }
        });
        for output in &statement.outputs {
            if let Some(local) = destination(output.materialization()) {
                access(local, point, &candidates, &mut eligible, &layout);
            }
        }
    }
    let mut handled = filled(code.expressions.len(), false)?;
    let mut edges = HashMap::new();
    edges
        .try_reserve(code.transfers.len())
        .map_err(crate::RequestError::Allocation)?;
    for transfer in &code.transfers {
        if let Some(edge) = transfer.edge {
            edges.insert(
                edge,
                layout.nodes[transfer.from.0 as usize].map(|place| Point {
                    place,
                    step: Step::Terminal,
                }),
            );
        }
    }
    for bundle in &code.copies {
        let point = if matches!(bundle.emission, CopyEmission::Original) {
            match bundle.location {
                CopyLocation::Transfer {
                    site: TransferSite::NormalEdge(edge),
                    ..
                } => edges.get(&edge).copied().flatten(),
                CopyLocation::Transfer {
                    site: TransferSite::AfterCommit(operation),
                    ..
                } => layout.operation(code, operation, true),
                CopyLocation::Transfer {
                    site: TransferSite::BeforeOperation(operation),
                    ..
                }
                | CopyLocation::BeforeThrow(ThrowSite::Operation(operation)) => {
                    layout.operation(code, operation, false)
                }
                CopyLocation::Transfer {
                    site: TransferSite::BeforeTerminal(block),
                    ..
                }
                | CopyLocation::BeforeThrow(ThrowSite::Terminal(block)) => {
                    layout.terminals[block.0 as usize]
                }
                _ => None,
            }
        } else {
            None
        };
        for assignment in &bundle.assignments {
            if let Ok(local) = assignment.destination() {
                eligible[local.0 as usize] = false;
            }
            if let Some(local) = assignment.source().ok().and_then(|source| source.local()) {
                access(
                    StorageId(local.0),
                    point,
                    &candidates,
                    &mut eligible,
                    &layout,
                );
            }
        }
        for step in &bundle.steps {
            match *step {
                CopyStatement::Save {
                    destination,
                    source,
                } => {
                    eligible[destination.0 as usize] = false;
                    access(source, point, &candidates, &mut eligible, &layout);
                }
                CopyStatement::Assign {
                    destination,
                    expression,
                    ..
                } => {
                    eligible[destination.0 as usize] = false;
                    handled[expression.0 as usize] = true;
                    if let ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } =
                        code.expressions[expression.0 as usize].kind
                    {
                        access(storage, point, &candidates, &mut eligible, &layout);
                    }
                }
            }
        }
    }
    for group in &code.constructions {
        let point = site_point(
            UseSite::Operation {
                operation: group.initialization,
                argument: 0,
            },
            &operations,
            &layout,
        );
        if let ExprKind::New { arguments, .. } = &code.expressions[group.expression.0 as usize].kind
        {
            for &argument in arguments {
                handled[argument.0 as usize] = true;
                if code.emission.composed_use(argument).is_none()
                    && let ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } =
                        code.expressions[argument.0 as usize].kind
                {
                    access(storage, point, &candidates, &mut eligible, &layout);
                }
            }
        }
        for publication in &group.publications {
            if candidates[publication.destination.0 as usize]
                .is_none_or(|candidate| candidate.statement != group.statement)
            {
                eligible[publication.destination.0 as usize] = false;
            }
        }
    }
    for expression in &code.expressions {
        if handled[expression.id.0 as usize] || code.emission.composed_use(expression.id).is_some()
        {
            continue;
        }
        if let ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } = expression.kind {
            let point = expression
                .origin
                .as_ref()
                .and_then(|origin| site_point(origin.site, &operations, &layout));
            access(storage, point, &candidates, &mut eligible, &layout);
        }
    }
    let mut declarations = copy(&code.emission.local_initializers)?;
    for local in &code.locals {
        if local.gap.is_some()
            || code.emission.hidden_local(local.id)
            || code.emission.allocation_local(local.id)
            || matches!(
                local.original.kind(),
                LocalKind::Parameter { .. }
                    | LocalKind::Snapshot { .. }
                    | LocalKind::ParallelTemporary { .. }
            )
        {
            continue;
        }
        if eligible[local.id.0 as usize]
            && let Some(candidate) = candidates[local.id.0 as usize]
        {
            declarations[local.id.0 as usize] = Some(candidate.statement);
        }
    }
    code.emission.local_initializers = declarations;
    Ok(())
}
