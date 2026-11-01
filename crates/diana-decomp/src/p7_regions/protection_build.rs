use super::protection::{Catch, Clause, Node, NodeId, add, blocks, entry, ix};
use crate::p2_flow::normal::{BlockId, Terminal};
use crate::p6_variables::normal::StorageBody;
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

struct Forest {
    nodes: Vec<Node>,
    roots: Vec<Option<NodeId>>,
    handlers: Vec<BlockId>,
    follows: Vec<Option<BlockId>>,
}
struct Group {
    clauses: Vec<Clause>,
    ambient: Vec<Clause>,
    sources: Vec<BlockId>,
    inner: bool,
}
fn handler_index(
    handlers: &[BlockId],
    handler: BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<usize>, RequestError> {
    for (index, &old) in handlers.iter().enumerate() {
        budget.charge_work(1)?;
        if old == handler {
            return Ok(Some(index));
        }
    }
    Ok(None)
}
fn member(
    values: &[BlockId],
    value: BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for &old in values {
        budget.charge_work(1)?;
        if old == value {
            return Ok(true);
        }
    }
    Ok(false)
}
fn same(
    a: &[Clause],
    b: &[Clause],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        budget.charge_work(1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn copy<T: Copy>(values: &[T], budget: &mut MethodBudget<'_, '_>) -> Result<Vec<T>, RequestError> {
    let mut result = Vec::new();
    for &value in values {
        budget.charge_work(1)?;
        push(&mut result, value, budget)?;
    }
    Ok(result)
}
// Reverse DAG dominance gives a normal follow without counting exceptional
// transfers as ordinary branches. Cyclic graphs retain their original Debug.
fn postdominators(
    storage: &StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<usize>>, RequestError> {
    let ssa = &storage.typed.ssa;
    let n = ssa.blocks.len();
    let mut degree = Vec::new();
    let mut rank = Vec::new();
    let mut queue = Vec::new();
    for _ in &ssa.blocks {
        budget.charge_work(1)?;
        push(&mut degree, 0usize, budget)?;
        push(&mut rank, 0usize, budget)?;
    }
    for edge in &ssa.edges {
        budget.charge_work(1)?;
        degree[ix(edge.target.0)?] = degree[ix(edge.target.0)?]
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    for (index, &count) in degree.iter().enumerate() {
        budget.charge_work(1)?;
        if count == 0 {
            push(&mut queue, index, budget)?;
        }
    }
    let mut head = 0usize;
    while head < queue.len() {
        budget.charge_work(1)?;
        let source = queue[head];
        head += 1;
        for edge in &ssa.edges {
            budget.charge_work(1)?;
            if ix(edge.source.0)? != source {
                continue;
            }
            let at = ix(edge.target.0)?;
            degree[at] = degree[at]
                .checked_sub(1)
                .ok_or(RequestError::InternalInvariant(
                    "normal DAG degree underflow",
                ))?;
            if degree[at] == 0 {
                push(&mut queue, at, budget)?;
            }
        }
    }
    if queue.len() != n {
        return Ok(None);
    }
    for (position, &block) in queue.iter().enumerate() {
        budget.charge_work(1)?;
        rank[block] = position;
    }
    push(&mut rank, n, budget)?;
    let mut post = Vec::new();
    for _ in 0..=n {
        budget.charge_work(1)?;
        push(&mut post, n, budget)?;
    }
    for &source in queue.iter().rev() {
        budget.charge_work(1)?;
        let mut join = None;
        for edge in &ssa.edges {
            budget.charge_work(1)?;
            if ix(edge.source.0)? != source {
                continue;
            }
            let mut b = ix(edge.target.0)?;
            if let Some(mut a) = join {
                while a != b {
                    budget.charge_work(1)?;
                    if rank[a] < rank[b] {
                        a = post[a];
                    } else {
                        b = post[b];
                    }
                }
                join = Some(a);
            } else {
                join = Some(b);
            }
        }
        post[source] = join.unwrap_or(n);
    }
    Ok(Some(post))
}
enum Work {
    Trace {
        start: Option<BlockId>,
        stop: Option<BlockId>,
    },
    Join {
        prefix: Vec<NodeId>,
        block: BlockId,
    },
}
fn trace(
    storage: &StorageBody<'_>,
    post: &[usize],
    nodes: &mut Vec<Node>,
    start: BlockId,
    stop: Option<BlockId>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<NodeId>, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut todo = Vec::new();
    let mut results = Vec::new();
    let mut seen = Vec::new();
    for _ in &ssa.blocks {
        budget.charge_work(1)?;
        push(&mut seen, false, budget)?;
    }
    push(
        &mut todo,
        Work::Trace {
            start: Some(start),
            stop,
        },
        budget,
    )?;
    while let Some(work) = todo.pop() {
        budget.charge_work(1)?;
        match work {
            Work::Trace { start, stop } => {
                let mut prefix = Vec::new();
                let mut current = start;
                let mut branch = false;
                while current != stop {
                    budget.charge_work(1)?;
                    let Some(block) = current else {
                        break;
                    };
                    if std::mem::replace(&mut seen[ix(block.0)?], true) {
                        return Ok(None);
                    }
                    match ssa.blocks[ix(block.0)?].terminal {
                        Terminal::Jump(target) => {
                            let id = add(nodes, Node::Block(block), budget)?;
                            push(&mut prefix, id, budget)?;
                            current = Some(target);
                        }
                        Terminal::Test { yes, no, .. } => {
                            let join = post[ix(block.0)?];
                            let join = if join == ssa.blocks.len() {
                                None
                            } else {
                                Some(BlockId(
                                    u32::try_from(join)
                                        .map_err(|_| RequestError::PlatformCapacity)?,
                                ))
                            };
                            push(
                                &mut todo,
                                Work::Join {
                                    prefix: std::mem::take(&mut prefix),
                                    block,
                                },
                                budget,
                            )?;
                            push(&mut todo, Work::Trace { start: join, stop }, budget)?;
                            push(
                                &mut todo,
                                Work::Trace {
                                    start: Some(no),
                                    stop: join,
                                },
                                budget,
                            )?;
                            push(
                                &mut todo,
                                Work::Trace {
                                    start: Some(yes),
                                    stop: join,
                                },
                                budget,
                            )?;
                            branch = true;
                            break;
                        }
                        Terminal::Return(_) | Terminal::ReturnVoid | Terminal::Throw(_) => {
                            let id = add(nodes, Node::Block(block), budget)?;
                            push(&mut prefix, id, budget)?;
                            current = stop;
                        }
                        Terminal::Switch { .. } => return Ok(None),
                    }
                }
                if !branch {
                    let id = add(nodes, Node::Sequence(prefix), budget)?;
                    push(&mut results, id, budget)?;
                }
            }
            Work::Join { mut prefix, block } => {
                let tail = results
                    .pop()
                    .ok_or(RequestError::InternalInvariant("normal region lost tail"))?;
                let no = results
                    .pop()
                    .ok_or(RequestError::InternalInvariant("normal region lost no arm"))?;
                let yes = results.pop().ok_or(RequestError::InternalInvariant(
                    "normal region lost yes arm",
                ))?;
                let branch = add(nodes, Node::If { block, yes, no }, budget)?;
                push(&mut prefix, branch, budget)?;
                push(&mut prefix, tail, budget)?;
                let sequence = add(nodes, Node::Sequence(prefix), budget)?;
                push(&mut results, sequence, budget)?;
            }
        }
    }
    if results.len() != 1 {
        return Err(RequestError::InternalInvariant("normal forest lost root"));
    }
    Ok(results.pop())
}
fn forest(
    storage: &StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Forest>, RequestError> {
    let Some(post) = postdominators(storage, budget)? else {
        return Ok(None);
    };
    let mut nodes = Vec::new();
    let Some(main) = trace(storage, &post, &mut nodes, BlockId(0), None, budget)? else {
        return Ok(None);
    };
    let ordinary = blocks(&nodes, &[main], budget)?;
    let mut roots = Vec::new();
    push(&mut roots, Some(main), budget)?;
    let mut handlers = Vec::new();
    let mut follows = Vec::new();
    let mut owned = ordinary;
    for binding in &storage.typed.exceptions {
        budget.charge_work(1)?;
        let mut stop = ix(binding.handler.0)?;
        while stop != storage.typed.ssa.blocks.len() {
            budget.charge_work(1)?;
            if member(
                &owned,
                BlockId(u32::try_from(stop).map_err(|_| RequestError::PlatformCapacity)?),
                budget,
            )? {
                break;
            }
            stop = post[stop];
        }
        let stop = if stop == storage.typed.ssa.blocks.len() {
            None
        } else {
            Some(BlockId(
                u32::try_from(stop).map_err(|_| RequestError::PlatformCapacity)?,
            ))
        };
        let Some(root) = trace(storage, &post, &mut nodes, binding.handler, stop, budget)? else {
            return Ok(None);
        };
        for block in blocks(&nodes, &[root], budget)? {
            budget.charge_work(1)?;
            if member(&owned, block, budget)? {
                return Ok(None);
            }
            push(&mut owned, block, budget)?;
        }
        push(&mut roots, Some(root), budget)?;
        push(&mut handlers, binding.handler, budget)?;
        push(&mut follows, stop, budget)?;
    }
    Ok(Some(Forest {
        nodes,
        roots,
        handlers,
        follows,
    }))
}
fn groups(
    storage: &StorageBody<'_>,
    forest: &Forest,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Group>>, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut dependencies = Vec::new();
    for (index, root) in forest.roots.iter().enumerate().skip(1) {
        budget.charge_work(1)?;
        let Some(root) = root else {
            continue;
        };
        let owned = blocks(&forest.nodes, &[*root], budget)?;
        for edge in &ssa.exception_edges {
            budget.charge_work(1)?;
            if member(&owned, edge.source, budget)? {
                if forest.handlers[index - 1] == edge.target {
                    return Ok(None);
                }
                push(
                    &mut dependencies,
                    (forest.handlers[index - 1], edge.target),
                    budget,
                )?;
            }
        }
    }
    for &(a, b) in &dependencies {
        for &(c, d) in &dependencies {
            budget.charge_work(1)?;
            if b == c || a == d {
                return Ok(None);
            }
        }
    }
    let mut result: Vec<Group> = Vec::new();
    for edge in &ssa.exception_edges {
        budget.charge_work(1)?;
        if edge.rank.0 != 0 {
            continue;
        }
        let mut chain = Vec::new();
        let mut depths = Vec::new();
        for clause in &ssa.exception_edges {
            budget.charge_work(1)?;
            if clause.source != edge.source || clause.pc != edge.pc {
                continue;
            }
            push(
                &mut chain,
                Clause {
                    kind: clause.kind,
                    handler: clause.target,
                },
                budget,
            )?;
            let mut inner = false;
            for &(handler, _) in &dependencies {
                budget.charge_work(1)?;
                inner |= handler == clause.target;
            }
            push(&mut depths, inner, budget)?;
        }
        let mut split = chain.len();
        if depths.first() == Some(&true) {
            for (index, &inner) in depths.iter().enumerate() {
                budget.charge_work(1)?;
                if !inner {
                    split = index;
                    break;
                }
            }
        }
        for &inner in &depths[split..] {
            budget.charge_work(1)?;
            if inner {
                return Ok(None);
            }
        }
        for (start, end) in [(0, split), (split, chain.len())] {
            if start == end {
                continue;
            }
            let clauses = &chain[start..end];
            let ambient = &chain[end..];
            let mut found = None;
            for (index, group) in result.iter().enumerate() {
                budget.charge_work(1)?;
                if same(&group.clauses, clauses, budget)? {
                    if !same(&group.ambient, ambient, budget)? {
                        return Ok(None);
                    }
                    found = Some(index);
                    break;
                }
            }
            let index = if let Some(index) = found {
                index
            } else {
                let index = result.len();
                push(
                    &mut result,
                    Group {
                        clauses: copy(clauses, budget)?,
                        ambient: copy(ambient, budget)?,
                        sources: Vec::new(),
                        inner: depths[start],
                    },
                    budget,
                )?;
                index
            };
            if !member(&result[index].sources, edge.source, budget)? {
                push(&mut result[index].sources, edge.source, budget)?;
            }
        }
    }
    Ok(Some(result))
}
fn candidate(
    storage: &StorageBody<'_>,
    forest: &Forest,
    group: &Group,
    selected: &[NodeId],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let owned = blocks(&forest.nodes, selected, budget)?;
    for &source in &group.sources {
        budget.charge_work(1)?;
        if !member(&owned, source, budget)? {
            return Ok(false);
        }
    }
    let mut follow = None;
    for clause in &group.clauses {
        budget.charge_work(1)?;
        if member(&owned, clause.handler, budget)? {
            return Ok(false);
        }
        let Some(index) = handler_index(&forest.handlers, clause.handler, budget)? else {
            return Ok(false);
        };
        if forest.roots[index + 1].is_none() {
            return Ok(false);
        }
        if let Some(stop) = forest.follows[index] {
            if follow.is_some_and(|old| old != stop) {
                return Ok(false);
            }
            follow = Some(stop);
        }
    }
    let mut first = None;
    for &node in selected {
        budget.charge_work(1)?;
        if let Some(value) = entry(&forest.nodes, node, None, budget)? {
            first = Some(value);
            break;
        }
    }
    let Some(first) = first else {
        return Ok(false);
    };
    for edge in &storage.typed.ssa.edges {
        budget.charge_work(1)?;
        let source = member(&owned, edge.source, budget)?;
        let target = member(&owned, edge.target, budget)?;
        if !source && target && edge.target != first {
            return Ok(false);
        }
        if source && !target {
            if follow.is_some_and(|old| old != edge.target) {
                return Ok(false);
            }
            follow = Some(edge.target);
        }
    }
    // Every throwing site in the candidate must have precisely this ambient
    // suffix after already-reduced inner catches have been consumed.
    let mut todo = Vec::new();
    for &node in selected {
        push(&mut todo, (node, Vec::<Clause>::new()), budget)?;
    }
    while let Some((id, context)) = todo.pop() {
        budget.charge_work(1)?;
        let mut block = None;
        match &forest.nodes[ix(id.0)?] {
            Node::Block(value) => block = Some(*value),
            Node::If {
                block: value,
                yes,
                no,
            } => {
                block = Some(*value);
                push(&mut todo, (*yes, copy(&context, budget)?), budget)?;
                push(&mut todo, (*no, copy(&context, budget)?), budget)?;
            }
            Node::Sequence(children) => {
                for &child in children {
                    budget.charge_work(1)?;
                    push(&mut todo, (child, copy(&context, budget)?), budget)?;
                }
            }
            Node::Try { body, catches } => {
                let mut nested = Vec::new();
                for catch in catches {
                    for &clause in &catch.clauses {
                        budget.charge_work(1)?;
                        push(&mut nested, clause, budget)?;
                    }
                    push(&mut todo, (catch.body, copy(&context, budget)?), budget)?;
                }
                for &clause in &context {
                    budget.charge_work(1)?;
                    push(&mut nested, clause, budget)?;
                }
                push(&mut todo, (*body, nested), budget)?;
            }
        }
        if let Some(block) = block {
            let original = &storage.typed.ssa.blocks[ix(block.0)?];
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
                let mut expected = copy(&context, budget)?;
                for &clause in group.clauses.iter().chain(&group.ambient) {
                    budget.charge_work(1)?;
                    push(&mut expected, clause, budget)?;
                }
                let mut actual = Vec::new();
                for edge in &storage.typed.ssa.exception_edges {
                    budget.charge_work(1)?;
                    if edge.source == block && edge.pc == pc {
                        push(
                            &mut actual,
                            Clause {
                                kind: edge.kind,
                                handler: edge.target,
                            },
                            budget,
                        )?;
                    }
                }
                if !same(&actual, &expected, budget)? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}
fn reduce(
    storage: &StorageBody<'_>,
    forest: &mut Forest,
    group: &Group,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let mut todo = Vec::new();
    for root in forest.roots.iter().flatten() {
        push(&mut todo, *root, budget)?;
    }
    let mut chosen = None;
    while let Some(id) = todo.pop() {
        budget.charge_work(1)?;
        let node = &forest.nodes[ix(id.0)?];
        if let Node::Sequence(children) = node {
            for length in 1..=children.len() {
                for start in 0..=children.len() - length {
                    budget.charge_work(1)?;
                    if candidate(
                        storage,
                        forest,
                        group,
                        &children[start..start + length],
                        budget,
                    )? {
                        chosen = Some((id, Some((start, start + length))));
                        break;
                    }
                }
                if chosen.is_some() {
                    break;
                }
            }
            if chosen.is_some() {
                break;
            }
            for &child in children {
                budget.charge_work(1)?;
                push(&mut todo, child, budget)?;
            }
        } else {
            if candidate(storage, forest, group, &[id], budget)? {
                chosen = Some((id, None));
                break;
            }
            match node {
                Node::If { yes, no, .. } => {
                    push(&mut todo, *yes, budget)?;
                    push(&mut todo, *no, budget)?;
                }
                // An outer try owns an already-reduced inner try as a whole.
                Node::Try { .. } | Node::Block(_) => {}
                Node::Sequence(_) => unreachable!(),
            }
        }
    }
    let Some((id, range)) = chosen else {
        return Ok(false);
    };
    let body = if let Some((start, end)) = range {
        let Node::Sequence(children) = &forest.nodes[ix(id.0)?] else {
            return Err(RequestError::InternalInvariant(
                "protection candidate changed kind",
            ));
        };
        let selected = copy(&children[start..end], budget)?;
        add(&mut forest.nodes, Node::Sequence(selected), budget)?
    } else {
        let old = std::mem::replace(&mut forest.nodes[ix(id.0)?], Node::Sequence(Vec::new()));
        add(&mut forest.nodes, old, budget)?
    };
    let mut catches: Vec<Catch> = Vec::new();
    let mut used = Vec::new();
    for &clause in &group.clauses {
        budget.charge_work(1)?;
        if catches
            .last()
            .and_then(|c| c.clauses.first())
            .is_some_and(|old| old.handler == clause.handler)
        {
            let index = catches.len() - 1;
            push(&mut catches[index].clauses, clause, budget)?;
            continue;
        }
        if member(&used, clause.handler, budget)? {
            return Ok(false);
        }
        push(&mut used, clause.handler, budget)?;
        let Some(index) = handler_index(&forest.handlers, clause.handler, budget)? else {
            return Ok(false);
        };
        let Some(root) = forest.roots[index + 1].take() else {
            return Ok(false);
        };
        let mut clauses = Vec::new();
        push(&mut clauses, clause, budget)?;
        push(
            &mut catches,
            Catch {
                clauses,
                body: root,
            },
            budget,
        )?;
    }
    let replacement = Node::Try { body, catches };
    if let Some((start, end)) = range {
        let reduced = add(&mut forest.nodes, replacement, budget)?;
        let Node::Sequence(children) = &mut forest.nodes[ix(id.0)?] else {
            return Err(RequestError::InternalInvariant(
                "protection sequence changed kind",
            ));
        };
        budget.charge_work(
            u64::try_from(children.len()).map_err(|_| RequestError::PlatformCapacity)?,
        )?;
        children.splice(start..end, [reduced]);
    } else {
        forest.nodes[ix(id.0)?] = replacement;
    }
    Ok(true)
}
pub(super) fn build(
    storage: &StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<(Vec<Node>, NodeId)>, RequestError> {
    let Some(mut forest) = forest(storage, budget)? else {
        return Ok(None);
    };
    let Some(groups) = groups(storage, &forest, budget)? else {
        return Ok(None);
    };
    if groups.is_empty() {
        return Ok(None);
    }
    for inner in [true, false] {
        for group in &groups {
            budget.charge_work(1)?;
            if group.inner == inner && !reduce(storage, &mut forest, group, budget)? {
                return Ok(None);
            }
        }
    }
    for root in forest.roots.iter().skip(1) {
        budget.charge_work(1)?;
        if root.is_some() {
            return Ok(None);
        }
    }
    let Some(root) = forest.roots[0] else {
        return Ok(None);
    };
    Ok(Some((forest.nodes, root)))
}
