use super::protection::{Catch, Clause, Node, NodeId, add, blocks, entry, ix};
use crate::p2_flow::{BlockId, Terminal};
use crate::p5_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

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
    budget: &mut RunContext<'_>,
) -> Result<Option<usize>, RequestError> {
    for (index, &old) in handlers.iter().enumerate() {
        budget.checkpoint()?;
        if old == handler {
            return Ok(Some(index));
        }
    }
    Ok(None)
}
fn member(
    values: &[BlockId],
    value: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for &old in values {
        budget.checkpoint()?;
        if old == value {
            return Ok(true);
        }
    }
    Ok(false)
}
fn same(a: &[Clause], b: &[Clause], budget: &mut RunContext<'_>) -> Result<bool, RequestError> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        budget.checkpoint()?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn copy<T: Copy>(values: &[T], budget: &mut RunContext<'_>) -> Result<Vec<T>, RequestError> {
    let mut result = Vec::new();
    for &value in values {
        budget.checkpoint()?;
        push(&mut result, value, budget)?;
    }
    Ok(result)
}
// Reverse DAG dominance gives a normal follow without counting exceptional
// transfers as ordinary branches. Cyclic graphs retain their original Debug.
fn postdominators(
    storage: &StorageBody<'_>,
    loops: &[super::protected_loops::Loop],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<usize>>, RequestError> {
    let ssa = &storage.typed.ssa;
    let n = ssa.blocks.len();
    let mut degree = Vec::new();
    let mut rank = Vec::new();
    let mut queue = Vec::new();
    for _ in &ssa.blocks {
        budget.checkpoint()?;
        push(&mut degree, 0usize, budget)?;
        push(&mut rank, 0usize, budget)?;
    }
    for (index, edge) in ssa.edges.iter().enumerate() {
        budget.checkpoint()?;
        if super::protected_loops::cut(
            loops,
            crate::p2_flow::EdgeId(
                u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
            ),
            budget,
        )? {
            continue;
        }
        degree[ix(edge.target.0)?] = degree[ix(edge.target.0)?]
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    for (index, &count) in degree.iter().enumerate() {
        budget.checkpoint()?;
        if count == 0 {
            push(&mut queue, index, budget)?;
        }
    }
    let mut head = 0usize;
    while head < queue.len() {
        budget.checkpoint()?;
        let source = queue[head];
        head += 1;
        for (index, edge) in ssa.edges.iter().enumerate() {
            budget.checkpoint()?;
            if super::protected_loops::cut(
                loops,
                crate::p2_flow::EdgeId(
                    u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                ),
                budget,
            )? {
                continue;
            }
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
        budget.checkpoint()?;
        rank[block] = position;
    }
    push(&mut rank, n, budget)?;
    let mut post = Vec::new();
    for _ in 0..=n {
        budget.checkpoint()?;
        push(&mut post, n, budget)?;
    }
    for &source in queue.iter().rev() {
        budget.checkpoint()?;
        let mut join = None;
        for (index, edge) in ssa.edges.iter().enumerate() {
            budget.checkpoint()?;
            if super::protected_loops::cut(
                loops,
                crate::p2_flow::EdgeId(
                    u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                ),
                budget,
            )? {
                continue;
            }
            if ix(edge.source.0)? != source {
                continue;
            }
            let mut b = ix(edge.target.0)?;
            if let Some(mut a) = join {
                while a != b {
                    budget.checkpoint()?;
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
    Monitor {
        prefix: Vec<NodeId>,
        scope: super::monitors::ScopeId,
        enter: BlockId,
    },
    Loop {
        prefix: Vec<NodeId>,
        header: BlockId,
        body_yes: bool,
        follow: BlockId,
    },
    Trace {
        start: Option<BlockId>,
        stop: Option<BlockId>,
    },
    Join {
        prefix: Vec<NodeId>,
        block: BlockId,
    },
}
#[allow(
    clippy::too_many_arguments,
    reason = "normal reduction consumes checked monitor and loop products"
)]
fn trace(
    storage: &StorageBody<'_>,
    post: &[usize],
    monitors: Option<&super::monitors::Monitors>,
    loops: &[super::protected_loops::Loop],
    nodes: &mut Vec<Node>,
    start: BlockId,
    stop: Option<BlockId>,
    budget: &mut RunContext<'_>,
) -> Result<Option<NodeId>, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut todo = Vec::new();
    let mut results = Vec::new();
    let mut seen = Vec::new();
    for _ in &ssa.blocks {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        match work {
            Work::Trace { start, stop } => {
                let mut prefix = Vec::new();
                let mut current = start;
                let mut branch = false;
                while current != stop {
                    budget.checkpoint()?;
                    let Some(block) = current else {
                        break;
                    };
                    if std::mem::replace(&mut seen[ix(block.0)?], true) {
                        return Ok(None);
                    }
                    if let Some(monitors) = monitors {
                        let mut found = None;
                        for (index, scope) in monitors.scopes.iter().enumerate() {
                            budget.checkpoint()?;
                            if scope.block == block {
                                found = Some(index);
                                break;
                            }
                        }
                        if let Some(index) = found {
                            let scope = &monitors.scopes[index];
                            let Terminal::Jump(start) = ssa.blocks[ix(block.0)?].terminal else {
                                return Ok(None);
                            };
                            push(
                                &mut todo,
                                Work::Monitor {
                                    prefix: std::mem::take(&mut prefix),
                                    scope: super::monitors::ScopeId(
                                        u32::try_from(index)
                                            .map_err(|_| RequestError::PlatformCapacity)?,
                                    ),
                                    enter: block,
                                },
                                budget,
                            )?;
                            push(
                                &mut todo,
                                Work::Trace {
                                    start: Some(scope.follow),
                                    stop,
                                },
                                budget,
                            )?;
                            push(
                                &mut todo,
                                Work::Trace {
                                    start: Some(start),
                                    stop: Some(scope.follow),
                                },
                                budget,
                            )?;
                            branch = true;
                            break;
                        }
                    }
                    let mut region = None;
                    for candidate in loops {
                        budget.checkpoint()?;
                        if candidate.header == block {
                            region = Some(candidate);
                            break;
                        }
                    }
                    if let Some(region) = region {
                        let Terminal::Test { yes, no, .. } = ssa.blocks[ix(block.0)?].terminal
                        else {
                            return Ok(None);
                        };
                        push(
                            &mut todo,
                            Work::Loop {
                                prefix: std::mem::take(&mut prefix),
                                header: block,
                                body_yes: region.body_yes,
                                follow: region.follow,
                            },
                            budget,
                        )?;
                        push(
                            &mut todo,
                            Work::Trace {
                                start: Some(region.follow),
                                stop,
                            },
                            budget,
                        )?;
                        push(
                            &mut todo,
                            Work::Trace {
                                start: Some(if region.body_yes { yes } else { no }),
                                stop: Some(block),
                            },
                            budget,
                        )?;
                        branch = true;
                        break;
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
            Work::Monitor {
                mut prefix,
                scope,
                enter,
            } => {
                let tail = results
                    .pop()
                    .ok_or(RequestError::InternalInvariant("monitor lost follow"))?;
                let body = results
                    .pop()
                    .ok_or(RequestError::InternalInvariant("monitor lost body"))?;
                let node = add(nodes, Node::Synchronized { scope, enter, body }, budget)?;
                push(&mut prefix, node, budget)?;
                push(&mut prefix, tail, budget)?;
                let node = add(nodes, Node::Sequence(prefix), budget)?;
                push(&mut results, node, budget)?;
            }
            Work::Loop {
                mut prefix,
                header,
                body_yes,
                follow,
            } => {
                let tail = results
                    .pop()
                    .ok_or(RequestError::InternalInvariant("loop lost follow"))?;
                let body = results
                    .pop()
                    .ok_or(RequestError::InternalInvariant("loop lost body"))?;
                let node = add(
                    nodes,
                    Node::Loop {
                        header,
                        body,
                        body_yes,
                        follow,
                    },
                    budget,
                )?;
                push(&mut prefix, node, budget)?;
                push(&mut prefix, tail, budget)?;
                let node = add(nodes, Node::Sequence(prefix), budget)?;
                push(&mut results, node, budget)?;
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
    monitors: Option<&super::monitors::Monitors>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Forest>, RequestError> {
    let Some(loops) = super::protected_loops::collect(storage, budget)? else {
        return Ok(None);
    };
    let Some(post) = postdominators(storage, &loops, budget)? else {
        return Ok(None);
    };
    let mut nodes = Vec::new();
    let Some(main) = trace(
        storage,
        &post,
        monitors,
        &loops,
        &mut nodes,
        BlockId(0),
        None,
        budget,
    )?
    else {
        return Ok(None);
    };
    let ordinary = blocks(&nodes, &[main], budget)?;
    let mut roots = Vec::new();
    push(&mut roots, Some(main), budget)?;
    let mut handlers = Vec::new();
    let mut follows = Vec::new();
    let mut owned = ordinary;
    for binding in &storage.typed.exceptions {
        budget.checkpoint()?;
        if consumed_handler(monitors, binding.handler, budget)? {
            push(&mut roots, None, budget)?;
            push(&mut handlers, binding.handler, budget)?;
            push(&mut follows, None, budget)?;
            continue;
        }
        let mut stop = ix(binding.handler.0)?;
        while stop != storage.typed.ssa.blocks.len() {
            budget.checkpoint()?;
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
        let Some(root) = trace(
            storage,
            &post,
            monitors,
            &loops,
            &mut nodes,
            binding.handler,
            stop,
            budget,
        )?
        else {
            return Ok(None);
        };
        for block in blocks(&nodes, &[root], budget)? {
            budget.checkpoint()?;
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
    monitors: Option<&super::monitors::Monitors>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<Group>>, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut dependencies = Vec::new();
    for (index, root) in forest.roots.iter().enumerate().skip(1) {
        budget.checkpoint()?;
        let Some(root) = root else {
            continue;
        };
        let owned = blocks(&forest.nodes, &[*root], budget)?;
        for edge in &ssa.exception_edges {
            budget.checkpoint()?;
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
            budget.checkpoint()?;
            if b == c || a == d {
                return Ok(None);
            }
        }
    }
    let mut result: Vec<Group> = Vec::new();
    for edge in &ssa.exception_edges {
        budget.checkpoint()?;
        if edge.rank.0 != 0 {
            continue;
        }
        let mut chain = Vec::new();
        let mut depths = Vec::new();
        for clause in &ssa.exception_edges {
            budget.checkpoint()?;
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
                budget.checkpoint()?;
                inner |= handler == clause.target;
            }
            push(&mut depths, inner, budget)?;
        }
        let mut split = chain.len();
        if depths.first() == Some(&true) {
            for (index, &inner) in depths.iter().enumerate() {
                budget.checkpoint()?;
                if !inner {
                    split = index;
                    break;
                }
            }
        }
        for &inner in &depths[split..] {
            budget.checkpoint()?;
            if inner {
                return Ok(None);
            }
        }
        let mut cut = chain.len();
        for (index, clause) in chain.iter().enumerate() {
            budget.checkpoint()?;
            if consumed_handler(monitors, clause.handler, budget)? {
                cut = index;
                break;
            }
        }
        let split = split.min(cut);
        for (start, end) in [(0, split), (split, cut)] {
            if start == end {
                continue;
            }
            let clauses = &chain[start..end];
            let ambient = &chain[end..];
            let mut found = None;
            for (index, group) in result.iter().enumerate() {
                budget.checkpoint()?;
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
    monitors: Option<&super::monitors::Monitors>,
    forest: &Forest,
    group: &Group,
    selected: &[NodeId],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let owned = blocks(&forest.nodes, selected, budget)?;
    for &source in &group.sources {
        budget.checkpoint()?;
        if !member(&owned, source, budget)? {
            return Ok(false);
        }
    }
    let mut follow = None;
    for clause in &group.clauses {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        if let Some(value) = entry(&forest.nodes, node, None, budget)? {
            first = Some(value);
            break;
        }
    }
    let Some(first) = first else {
        return Ok(false);
    };
    for edge in &storage.typed.ssa.edges {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        let mut block = None;
        match &forest.nodes[ix(id.0)?] {
            Node::Synchronized { scope, enter, body } => {
                let Some(monitors) = monitors else {
                    return Ok(false);
                };
                let cleanup = monitors.cleanup(monitors.scope(*scope)?.cleanup)?;
                let mut nested = Vec::new();
                push(
                    &mut nested,
                    Clause {
                        kind: crate::input::protection::CatchType::All,
                        handler: cleanup.handler,
                    },
                    budget,
                )?;
                push(&mut todo, (*body, nested), budget)?;
                block = Some(*enter);
            }
            Node::Loop { header, body, .. } => {
                block = Some(*header);
                push(&mut todo, (*body, copy(&context, budget)?), budget)?;
            }
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
                    budget.checkpoint()?;
                    push(&mut todo, (child, copy(&context, budget)?), budget)?;
                }
            }
            Node::Try { body, catches } => {
                let mut nested = Vec::new();
                for catch in catches {
                    for &clause in &catch.clauses {
                        budget.checkpoint()?;
                        push(&mut nested, clause, budget)?;
                    }
                    push(&mut todo, (catch.body, copy(&context, budget)?), budget)?;
                }
                for &clause in &context {
                    budget.checkpoint()?;
                    if nested.last().is_some_and(|clause: &Clause| {
                        clause.kind == crate::input::protection::CatchType::All
                    }) {
                        break;
                    }
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
                let mut expected = Vec::new();
                for &clause in context.iter().chain(&group.clauses).chain(&group.ambient) {
                    budget.checkpoint()?;
                    push(&mut expected, clause, budget)?;
                    if clause.kind == crate::input::protection::CatchType::All {
                        break;
                    }
                }
                let mut actual = Vec::new();
                for edge in &storage.typed.ssa.exception_edges {
                    budget.checkpoint()?;
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
    monitors: Option<&super::monitors::Monitors>,
    forest: &mut Forest,
    group: &Group,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let mut todo = Vec::new();
    for root in forest.roots.iter().flatten() {
        push(&mut todo, *root, budget)?;
    }
    let mut chosen = None;
    while let Some(id) = todo.pop() {
        budget.checkpoint()?;
        let node = &forest.nodes[ix(id.0)?];
        if let Node::Sequence(children) = node {
            for length in 1..=children.len() {
                for start in 0..=children.len() - length {
                    budget.checkpoint()?;
                    if candidate(
                        storage,
                        monitors,
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
                budget.checkpoint()?;
                push(&mut todo, child, budget)?;
            }
        } else {
            if candidate(storage, monitors, forest, group, &[id], budget)? {
                chosen = Some((id, None));
                break;
            }
            match node {
                Node::If { yes, no, .. } => {
                    push(&mut todo, *yes, budget)?;
                    push(&mut todo, *no, budget)?;
                }
                // An outer try owns an already-reduced inner try as a whole.
                Node::Synchronized { body, .. } | Node::Loop { body, .. } => {
                    push(&mut todo, *body, budget)?
                }
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
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        children.splice(start..end, [reduced]);
    } else {
        forest.nodes[ix(id.0)?] = replacement;
    }
    Ok(true)
}
fn consumed_handler(
    monitors: Option<&super::monitors::Monitors>,
    handler: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    if let Some(monitors) = monitors {
        for scope in &monitors.scopes {
            budget.checkpoint()?;
            if monitors.cleanup(scope.cleanup)?.handler == handler {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
pub(super) fn build(
    storage: &StorageBody<'_>,
    monitors: Option<&super::monitors::Monitors>,
    budget: &mut RunContext<'_>,
) -> Result<Option<(Vec<Node>, NodeId)>, RequestError> {
    let Some(mut forest) = forest(storage, monitors, budget)? else {
        return Ok(None);
    };
    let Some(groups) = groups(storage, &forest, monitors, budget)? else {
        return Ok(None);
    };
    if groups.is_empty() && monitors.is_none() {
        return Ok(None);
    }
    for inner in [true, false] {
        for group in &groups {
            budget.checkpoint()?;
            if group.inner == inner && !reduce(storage, monitors, &mut forest, group, budget)? {
                return Ok(None);
            }
        }
    }
    for root in forest.roots.iter().skip(1) {
        budget.checkpoint()?;
        if root.is_some() {
            return Ok(None);
        }
    }
    let Some(root) = forest.roots[0] else {
        return Ok(None);
    };
    Ok(Some((forest.nodes, root)))
}
