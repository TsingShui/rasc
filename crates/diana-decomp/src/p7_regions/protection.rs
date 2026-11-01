use crate::input::protection::CatchType;
use crate::p2_flow::normal::{BlockId, Terminal};
use crate::p6_variables::normal::StorageBody;
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

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
}
pub(super) fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn add(
    nodes: &mut Vec<Node>,
    node: Node,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<NodeId, RequestError> {
    let id = NodeId(u32::try_from(nodes.len()).map_err(|_| RequestError::PlatformCapacity)?);
    push(nodes, node, budget)?;
    Ok(id)
}
pub(super) fn entry(
    nodes: &[Node],
    start: NodeId,
    follow: Option<BlockId>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<BlockId>, RequestError> {
    let mut todo = Vec::new();
    push(&mut todo, start, budget)?;
    let mut visits = 0usize;
    while let Some(id) = todo.pop() {
        budget.charge_work(1)?;
        if visits >= nodes.len() {
            return Err(RequestError::InternalInvariant("cyclic protection entry"));
        }
        visits += 1;
        match nodes
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant("foreign protection node"))?
        {
            Node::Block(block) | Node::If { block, .. } => return Ok(Some(*block)),
            Node::Try { body, .. } => push(&mut todo, *body, budget)?,
            Node::Sequence(children) => {
                for &child in children.iter().rev() {
                    budget.charge_work(1)?;
                    push(&mut todo, child, budget)?;
                }
            }
        }
    }
    Ok(follow)
}
pub(super) fn blocks(
    nodes: &[Node],
    roots: &[NodeId],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<BlockId>, RequestError> {
    let mut todo = Vec::new();
    let mut result = Vec::new();
    for &root in roots {
        push(&mut todo, root, budget)?;
    }
    while let Some(id) = todo.pop() {
        budget.charge_work(1)?;
        match &nodes[ix(id.0)?] {
            Node::Block(block) => push(&mut result, *block, budget)?,
            Node::Sequence(children) => {
                for &child in children {
                    budget.charge_work(1)?;
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
                    budget.charge_work(1)?;
                    push(&mut todo, catch.body, budget)?;
                }
            }
        }
    }
    Ok(result)
}

pub(crate) fn build(
    storage: &StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Protection>, RequestError> {
    let Some((nodes, root)) = super::protection_build::build(storage, budget)? else {
        return Ok(None);
    };
    let result = Protection {
        _sealed: (),
        nodes,
        root,
    };
    if !check(storage, &result, budget)? {
        return Ok(None);
    }
    Ok(Some(result))
}
pub(crate) fn verify(
    storage: &StorageBody<'_>,
    plan: &Protection,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    if !check(storage, plan, budget)? {
        return Err(RequestError::InternalInvariant(
            "protection certificate changed coverage, flow or catch stack",
        ));
    }
    Ok(())
}
fn check(
    storage: &StorageBody<'_>,
    plan: &Protection,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut seen = Vec::new();
    for _ in &ssa.blocks {
        budget.charge_work(1)?;
        push(&mut seen, false, budget)?;
    }
    let mut visited = Vec::new();
    for _ in &plan.nodes {
        budget.charge_work(1)?;
        push(&mut visited, false, budget)?;
    }
    let mut contexts: Vec<Vec<Clause>> = Vec::new();
    let mut depths = Vec::new();
    push(&mut depths, 0u8, budget)?;
    push(&mut contexts, Vec::new(), budget)?;
    let mut todo = Vec::new();
    push(&mut todo, (plan.root, None, 0usize), budget)?;
    if entry(&plan.nodes, plan.root, None, budget)? != Some(BlockId(0)) {
        return Ok(false);
    }
    while let Some((id, follow, context)) = todo.pop() {
        budget.charge_work(1)?;
        let Some(mark) = visited.get_mut(ix(id.0)?) else {
            return Ok(false);
        };
        if std::mem::replace(mark, true) {
            return Ok(false);
        }
        let mut original = None;
        match &plan.nodes[ix(id.0)?] {
            Node::Sequence(children) => {
                let mut next = follow;
                for &child in children.iter().rev() {
                    budget.charge_work(1)?;
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
                    budget.charge_work(1)?;
                    let Some(first) = catch.clauses.first() else {
                        return Ok(false);
                    };
                    if entry(&plan.nodes, catch.body, follow, budget)? != Some(first.handler) {
                        return Ok(false);
                    }
                    for &clause in &catch.clauses {
                        budget.charge_work(1)?;
                        if clause.handler != first.handler {
                            return Ok(false);
                        }
                        push(&mut nested, clause, budget)?;
                    }
                    push(&mut todo, (catch.body, follow, context), budget)?;
                }
                for &clause in &contexts[context] {
                    budget.charge_work(1)?;
                    push(&mut nested, clause, budget)?;
                }
                let index = contexts.len();
                let depth = depths[context] + 1;
                push(&mut depths, depth, budget)?;
                push(&mut contexts, nested, budget)?;
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
                .map(|op| op.kind.effects().may_throw.then_some(op.pc))
                .chain(std::iter::once(
                    matches!(original.terminal, Terminal::Throw(_)).then_some(original.terminal_pc),
                ))
            {
                budget.charge_work(1)?;
                let Some(pc) = pc else {
                    continue;
                };
                let mut rank = 0usize;
                for edge in &ssa.exception_edges {
                    budget.charge_work(1)?;
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
        budget.charge_work(1)?;
        if !covered {
            return Ok(false);
        }
    }
    Ok(true)
}
