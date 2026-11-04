use crate::input::protection::CatchType;
use crate::p2_flow::{BlockId, Terminal};
use crate::p5_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NodeId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Clause {
    pub kind: CatchType,
    pub handler: BlockId,
}
pub(crate) struct Catch {
    pub clauses: Vec<Clause>,
    pub body: NodeId,
}
pub(crate) enum Node {
    Synchronized {
        scope: super::monitors::ScopeId,
        enter: BlockId,
        body: NodeId,
    },
    Loop {
        header: BlockId,
        body: NodeId,
        body_yes: bool,
        follow: BlockId,
    },
    Block(BlockId),
    Sequence(Vec<NodeId>),
    If {
        block: BlockId,
        yes: NodeId,
        no: NodeId,
    },
    Try {
        body: NodeId,
        catches: Vec<Catch>,
    },
}
pub(crate) struct Protection {
    _sealed: (),
    pub nodes: Vec<Node>,
    pub root: NodeId,
    pub monitors: Option<super::monitors::Monitors>,
}
pub(super) fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn add(
    nodes: &mut Vec<Node>,
    node: Node,
    budget: &mut RunContext<'_>,
) -> Result<NodeId, RequestError> {
    let id = NodeId(u32::try_from(nodes.len()).map_err(|_| RequestError::PlatformCapacity)?);
    push(nodes, node, budget)?;
    Ok(id)
}
pub(super) fn entry(
    nodes: &[Node],
    start: NodeId,
    follow: Option<BlockId>,
    budget: &mut RunContext<'_>,
) -> Result<Option<BlockId>, RequestError> {
    let mut todo = Vec::new();
    push(&mut todo, start, budget)?;
    let mut visits = 0usize;
    while let Some(id) = todo.pop() {
        budget.checkpoint()?;
        if visits >= nodes.len() {
            return Err(RequestError::InternalInvariant("cyclic protection entry"));
        }
        visits += 1;
        match nodes
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant("foreign protection node"))?
        {
            Node::Synchronized { enter, .. } => return Ok(Some(*enter)),
            Node::Loop { header, .. } => return Ok(Some(*header)),
            Node::Block(block) | Node::If { block, .. } => return Ok(Some(*block)),
            Node::Try { body, .. } => push(&mut todo, *body, budget)?,
            Node::Sequence(children) => {
                for &child in children.iter().rev() {
                    budget.checkpoint()?;
                    push(&mut todo, child, budget)?;
                }
            }
        }
    }
    Ok(follow)
}
pub(crate) fn blocks(
    nodes: &[Node],
    roots: &[NodeId],
    budget: &mut RunContext<'_>,
) -> Result<Vec<BlockId>, RequestError> {
    let mut todo = Vec::new();
    let mut result = Vec::new();
    for &root in roots {
        push(&mut todo, root, budget)?;
    }
    while let Some(id) = todo.pop() {
        budget.checkpoint()?;
        match &nodes[ix(id.0)?] {
            Node::Synchronized { enter, body, .. } => {
                push(&mut result, *enter, budget)?;
                push(&mut todo, *body, budget)?;
            }
            Node::Loop { header, body, .. } => {
                push(&mut result, *header, budget)?;
                push(&mut todo, *body, budget)?;
            }
            Node::Block(block) => push(&mut result, *block, budget)?,
            Node::Sequence(children) => {
                for &child in children {
                    budget.checkpoint()?;
                    push(&mut todo, child, budget)?;
                }
            }
            Node::If { block, yes, no } => {
                push(&mut result, *block, budget)?;
                push(&mut todo, *yes, budget)?;
                push(&mut todo, *no, budget)?;
            }
            Node::Try { body, catches } => {
                push(&mut todo, *body, budget)?;
                for catch in catches {
                    budget.checkpoint()?;
                    push(&mut todo, catch.body, budget)?;
                }
            }
        }
    }
    Ok(result)
}

pub(super) fn build(
    storage: &StorageBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Protection>, RequestError> {
    let monitors = super::monitors::build(storage, budget)?;
    let Some((nodes, root)) = super::protection_build::build(storage, monitors.as_ref(), budget)?
    else {
        if let Some(monitors) = &monitors
            && let Some(scope) = monitors.scopes.first()
        {
            return Err(RequestError::MonitorScopeUnproved {
                pc: storage.typed.ssa.operation(scope.enter)?.pc,
            });
        }
        return Ok(None);
    };
    let result = Protection {
        _sealed: (),
        nodes,
        root,
        monitors,
    };
    if !check(storage, &result, budget)? {
        if let Some(monitors) = &result.monitors
            && let Some(scope) = monitors.scopes.first()
        {
            return Err(RequestError::MonitorScopeUnproved {
                pc: storage.typed.ssa.operation(scope.enter)?.pc,
            });
        }
        return Ok(None);
    }
    Ok(Some(result))
}
pub(crate) fn verify(
    storage: &StorageBody<'_>,
    plan: &Protection,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    if !check(storage, plan, budget)? {
        return Err(RequestError::InternalInvariant(
            "protection certificate changed coverage, flow or catch stack",
        ));
    }
    Ok(())
}
fn catch_stack(
    storage: &StorageBody<'_>,
    block: BlockId,
    pc: crate::CodeUnitOffset,
    expected: &[Clause],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let mut rank = 0usize;
    for edge in &storage.typed.ssa.exception_edges {
        budget.checkpoint()?;
        if edge.source != block || edge.pc != pc {
            continue;
        }
        if expected.get(rank)
            != Some(&Clause {
                kind: edge.kind,
                handler: edge.target,
            })
        {
            return Ok(false);
        }
        rank = rank.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(rank == expected.len())
}
fn check(
    storage: &StorageBody<'_>,
    plan: &Protection,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let ssa = &storage.typed.ssa;
    if let Some(monitors) = &plan.monitors {
        super::monitors::verify(storage, monitors, budget)?;
    }
    let mut seen = Vec::new();
    for _ in &ssa.blocks {
        budget.checkpoint()?;
        push(&mut seen, false, budget)?;
    }
    let mut visited = Vec::new();
    for _ in &plan.nodes {
        budget.checkpoint()?;
        push(&mut visited, false, budget)?;
    }
    let mut contexts: Vec<Vec<Clause>> = Vec::new();
    let mut locks: Vec<Vec<crate::p2_flow::OpId>> = Vec::new();
    push(&mut locks, Vec::new(), budget)?;
    let mut depths = Vec::new();
    push(&mut depths, 0u8, budget)?;
    push(&mut contexts, Vec::new(), budget)?;
    let mut todo = Vec::new();
    push(&mut todo, (plan.root, None, 0usize), budget)?;
    if entry(&plan.nodes, plan.root, None, budget)? != Some(BlockId(0)) {
        return Ok(false);
    }
    while let Some((id, follow, context)) = todo.pop() {
        budget.checkpoint()?;
        let Some(mark) = visited.get_mut(ix(id.0)?) else {
            return Ok(false);
        };
        if std::mem::replace(mark, true) {
            return Ok(false);
        }
        let mut original = None;
        match &plan.nodes[ix(id.0)?] {
            Node::Synchronized { scope, enter, body } => {
                let Some(monitors) = &plan.monitors else {
                    return Ok(false);
                };
                let scope = monitors.scope(*scope)?;
                if scope.block != *enter || Some(scope.follow) != follow {
                    return Ok(false);
                }
                let source = &ssa.blocks[ix(enter.0)?];
                let Terminal::Jump(target) = source.terminal else {
                    return Ok(false);
                };
                if entry(&plan.nodes, *body, follow, budget)? != Some(target) {
                    return Ok(false);
                }
                let projected = blocks(&plan.nodes, &[*body], budget)?;
                for &expected in &scope.body {
                    let mut found = false;
                    for &actual in &projected {
                        budget.checkpoint()?;
                        found |= expected == actual;
                    }
                    if !found {
                        return Ok(false);
                    }
                }
                let cleanup = monitors.cleanup(scope.cleanup)?;
                for &block in &cleanup.blocks {
                    budget.checkpoint()?;
                    if std::mem::replace(&mut seen[ix(block.0)?], true) {
                        return Ok(false);
                    }
                    let original = &ssa.blocks[ix(block.0)?];
                    if matches!(original.terminal, Terminal::Throw(_))
                        && !catch_stack(
                            storage,
                            block,
                            original.terminal_pc,
                            &contexts[context],
                            budget,
                        )?
                    {
                        return Ok(false);
                    }
                }
                let mut nested = Vec::new();
                push(
                    &mut nested,
                    Clause {
                        kind: CatchType::All,
                        handler: cleanup.handler,
                    },
                    budget,
                )?;
                let mut held = Vec::new();
                for &lock in &locks[context] {
                    budget.checkpoint()?;
                    push(&mut held, lock, budget)?;
                }
                push(&mut held, scope.enter, budget)?;
                let index = contexts.len();
                let depth = depths[context];
                push(&mut contexts, nested, budget)?;
                push(&mut locks, held, budget)?;
                push(&mut depths, depth, budget)?;
                push(&mut todo, (*body, follow, index), budget)?;
                original = Some(*enter);
            }
            Node::Loop {
                header,
                body,
                body_yes,
                follow: exit,
            } => {
                if Some(*exit) != follow {
                    return Ok(false);
                }
                let Terminal::Test { yes, no, .. } = ssa.blocks[ix(header.0)?].terminal else {
                    return Ok(false);
                };
                if (if *body_yes { no } else { yes }) != *exit
                    || entry(&plan.nodes, *body, Some(*header), budget)?
                        != Some(if *body_yes { yes } else { no })
                {
                    return Ok(false);
                }
                push(&mut todo, (*body, Some(*header), context), budget)?;
                original = Some(*header);
            }
            Node::Sequence(children) => {
                let mut next = follow;
                for &child in children.iter().rev() {
                    budget.checkpoint()?;
                    push(&mut todo, (child, next, context), budget)?;
                    next = entry(&plan.nodes, child, next, budget)?;
                }
            }
            Node::Try { body, catches } => {
                if catches.is_empty() {
                    return Ok(false);
                }
                if depths[context] >= 2 {
                    return Ok(false);
                }
                let mut nested = Vec::new();
                for catch in catches {
                    budget.checkpoint()?;
                    let Some(first) = catch.clauses.first() else {
                        return Ok(false);
                    };
                    if entry(&plan.nodes, catch.body, follow, budget)? != Some(first.handler) {
                        return Ok(false);
                    }
                    for &clause in &catch.clauses {
                        budget.checkpoint()?;
                        if clause.handler != first.handler {
                            return Ok(false);
                        }
                        push(&mut nested, clause, budget)?;
                    }
                    push(&mut todo, (catch.body, follow, context), budget)?;
                }
                for &clause in &contexts[context] {
                    budget.checkpoint()?;
                    if nested
                        .last()
                        .is_some_and(|clause: &Clause| clause.kind == CatchType::All)
                    {
                        break;
                    }
                    push(&mut nested, clause, budget)?;
                }
                let index = contexts.len();
                let depth = depths[context] + 1;
                push(&mut depths, depth, budget)?;
                push(&mut contexts, nested, budget)?;
                let mut held = Vec::new();
                for &lock in &locks[context] {
                    budget.checkpoint()?;
                    push(&mut held, lock, budget)?;
                }
                push(&mut locks, held, budget)?;
                push(&mut todo, (*body, follow, index), budget)?;
            }
            Node::If { block, yes, no } => {
                let Some(source) = ssa.blocks.get(ix(block.0)?) else {
                    return Ok(false);
                };
                let Terminal::Test {
                    yes: target_yes,
                    no: target_no,
                    ..
                } = source.terminal
                else {
                    return Ok(false);
                };
                if entry(&plan.nodes, *yes, follow, budget)? != Some(target_yes)
                    || entry(&plan.nodes, *no, follow, budget)? != Some(target_no)
                {
                    return Ok(false);
                }
                push(&mut todo, (*yes, follow, context), budget)?;
                push(&mut todo, (*no, follow, context), budget)?;
                original = Some(*block);
            }
            Node::Block(block) => {
                let Some(source) = ssa.blocks.get(ix(block.0)?) else {
                    return Ok(false);
                };
                match source.terminal {
                    Terminal::Jump(target) if Some(target) == follow => {}
                    Terminal::Return(_) | Terminal::ReturnVoid | Terminal::Throw(_) => {}
                    _ => return Ok(false),
                }
                original = Some(*block);
            }
        }
        if let Some(block) = original {
            if let Some(monitors) = &plan.monitors
                && !monitors.matches_entry(block, &locks[context], budget)?
            {
                return Ok(false);
            }
            let Some(mark) = seen.get_mut(ix(block.0)?) else {
                return Ok(false);
            };
            if std::mem::replace(mark, true) {
                return Ok(false);
            }
            let original = &ssa.blocks[ix(block.0)?];
            for pc in original
                .ops
                .iter()
                .map(|op| {
                    (op.kind.effects().may_throw
                        && !matches!(
                            op.kind,
                            crate::p2_flow::Op::Monitor(crate::p2_flow::MonitorKind::Exit, _)
                        ))
                    .then_some(op.pc)
                })
                .chain(std::iter::once(
                    matches!(original.terminal, Terminal::Throw(_)).then_some(original.terminal_pc),
                ))
            {
                budget.checkpoint()?;
                let Some(pc) = pc else {
                    continue;
                };
                let mut rank = 0usize;
                for edge in &ssa.exception_edges {
                    budget.checkpoint()?;
                    if edge.source != block || edge.pc != pc {
                        continue;
                    }
                    if contexts[context].get(rank)
                        != Some(&Clause {
                            kind: edge.kind,
                            handler: edge.target,
                        })
                    {
                        return Ok(false);
                    }
                    rank = rank.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                }
                if rank != contexts[context].len() {
                    return Ok(false);
                }
            }
        }
    }
    for covered in seen {
        budget.checkpoint()?;
        if !covered {
            return Ok(false);
        }
    }
    Ok(true)
}
