use super::dispatcher::{Case, Dispatcher, Edge, Transfer};
use super::protection_replay::membership;
use super::source_control::{Label, Step};
use crate::p2_flow::BlockId;
use crate::p6_regions::protection::{Node, Protection};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

fn broken() -> RequestError {
    RequestError::InternalInvariant("final source control differs from checked operations")
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn case(storage: &Dispatcher, id: BlockId) -> Result<&Case, RequestError> {
    storage
        .cases
        .get(ix(id.0)?)
        .filter(|case| case.key == id)
        .ok_or_else(broken)
}
fn edge(
    storage: &Dispatcher,
    source: BlockId,
    id: crate::p2_flow::EdgeId,
) -> Result<&Edge, RequestError> {
    match &case(storage, source)?.transfer {
        Transfer::Jump(edge) if edge.id == id => Ok(edge),
        Transfer::Branch { yes, .. } if yes.id == id => Ok(yes),
        Transfer::Branch { no, .. } if no.id == id => Ok(no),
        _ => Err(broken()),
    }
}

// These successors describe Java lexical execution, not the producer's CFG targets.
struct Lexical {
    next: Vec<usize>,
    alternative: Vec<Option<usize>>,
    close: Vec<Option<usize>>,
    try_body_end: Vec<Option<usize>>,
}
impl Lexical {
    fn build(steps: &[Step], budget: &mut RunContext<'_>) -> Result<Self, RequestError> {
        let mut next = Vec::new();
        let mut alternative = Vec::new();
        let mut close = Vec::new();
        let mut try_body_end = Vec::new();
        let mut owner = Vec::new();
        let mut stack = Vec::new();
        for (at, step) in steps.iter().enumerate() {
            budget.checkpoint()?;
            push(
                &mut next,
                at.checked_add(1).ok_or(RequestError::PlatformCapacity)?,
                budget,
            )?;
            push(&mut alternative, None, budget)?;
            push(&mut close, None, budget)?;
            push(&mut try_body_end, None, budget)?;
            push(&mut owner, None, budget)?;
            match step {
                Step::Loop(label) => {
                    for &opening in &stack {
                        budget.checkpoint()?;
                        if *label == Label::Loop0
                            && matches!(steps[opening], Step::Loop(Label::Loop0))
                        {
                            return Err(broken());
                        }
                    }
                    push(&mut stack, at, budget)?;
                }
                Step::If(_) | Step::Synchronized(_) | Step::Try(_) | Step::Catch { .. } => {
                    push(&mut stack, at, budget)?
                }
                Step::Else => {
                    let opening = *stack.last().ok_or_else(broken)?;
                    if !matches!(steps[opening], Step::If(_)) || alternative[opening].is_some() {
                        return Err(broken());
                    }
                    alternative[opening] = Some(at + 1);
                    owner[at] = Some(opening);
                }
                Step::CloseTryBody => {
                    let opening = *stack.last().ok_or_else(broken)?;
                    if !matches!(steps[opening], Step::Try(_))
                        || try_body_end[opening].replace(at).is_some()
                    {
                        return Err(broken());
                    }
                    owner[at] = Some(opening);
                }
                Step::End => {
                    let opening = stack.pop().ok_or_else(broken)?;
                    close[opening] = Some(at);
                    match steps[opening] {
                        Step::Loop(_) => {
                            let mut body = false;
                            for step in &steps[opening + 1..at] {
                                budget.checkpoint()?;
                                body |= matches!(step, Step::Statements(_));
                            }
                            if !body {
                                return Err(broken());
                            }
                            next[at] = opening + 1;
                        }
                        Step::Catch { .. } => {
                            let enclosing = *stack.last().ok_or_else(broken)?;
                            if !matches!(steps[enclosing], Step::Try(_)) {
                                return Err(broken());
                            }
                            owner[at] = Some(enclosing);
                        }
                        Step::If(_) | Step::Synchronized(_) => {}
                        _ => return Err(broken()),
                    }
                }
                Step::EndTry => {
                    let opening = stack.pop().ok_or_else(broken)?;
                    if !matches!(steps[opening], Step::Try(_)) {
                        return Err(broken());
                    }
                    close[opening] = Some(at);
                }
                Step::Break(label) | Step::Continue(label) => {
                    for &opening in stack.iter().rev() {
                        budget.checkpoint()?;
                        if let Step::Loop(actual) = steps[opening]
                            && (*label == Label::Nearest || *label == actual)
                        {
                            owner[at] = Some(opening);
                            break;
                        }
                    }
                    if owner[at].is_none() {
                        return Err(broken());
                    }
                }
                _ => {}
            }
        }
        if !stack.is_empty() {
            return Err(broken());
        }
        for (at, step) in steps.iter().enumerate() {
            budget.checkpoint()?;
            if let Some(opening) = owner[at] {
                next[at] = if matches!(step, Step::Continue(_)) {
                    opening + 1
                } else {
                    close[opening]
                        .ok_or_else(broken)?
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?
                };
            }
            if matches!(step, Step::If(_)) && alternative[at].is_none() {
                return Err(broken());
            }
        }
        Ok(Self {
            next,
            alternative,
            close,
            try_body_end,
        })
    }
    fn event<'a>(
        &self,
        steps: &'a [Step],
        mut at: usize,
        used: &mut [bool],
        budget: &mut RunContext<'_>,
    ) -> Result<&'a Step, RequestError> {
        for _ in steps {
            budget.checkpoint()?;
            let step = steps.get(at).ok_or_else(broken)?;
            match step {
                Step::Statements(_)
                | Step::Copies { .. }
                | Step::If(_)
                | Step::Return(_)
                | Step::ReturnVoid
                | Step::InitializerExit
                | Step::Throw(_)
                | Step::Rethrow(_) => {
                    used[at] = true;
                    return Ok(step);
                }
                _ => at = self.next[at],
            }
        }
        Err(broken())
    }
}
fn matches_copies(step: &Step, edge: &Edge) -> bool {
    matches!(step, Step::Copies {source,edge:id} if *source == edge.source && *id == edge.id)
}
fn matches_completion(step: &Step, transfer: &Transfer) -> bool {
    match (step, transfer) {
        (Step::Return(actual), Transfer::Return(expected))
        | (Step::Throw(actual), Transfer::Throw(expected)) => actual == expected,
        (Step::ReturnVoid, Transfer::ReturnVoid)
        | (Step::InitializerExit, Transfer::InitializerExit) => true,
        (Step::Rethrow(actual), Transfer::Rethrow { handler, .. }) => actual == handler,
        _ => false,
    }
}

pub(super) fn verify(
    steps: &[Step],
    storage: &Dispatcher,
    protection: Option<&Protection>,
    names: &[super::protected::CatchName],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let lexical = Lexical::build(steps, budget)?;
    let mut used = Vec::new();
    for _ in steps {
        budget.checkpoint()?;
        push(&mut used, false, budget)?;
    }
    let mut seen_tries = Vec::new();
    if let Some(protection) = protection {
        for _ in &protection.nodes {
            budget.checkpoint()?;
            push(&mut seen_tries, false, budget)?;
        }
    }
    let mut expected = Vec::new();
    let mut seen = Vec::new();
    let mut locks = Vec::new();
    for _ in &storage.cases {
        budget.checkpoint()?;
        push(&mut expected, protection.is_none(), budget)?;
        push(&mut seen, false, budget)?;
        push(&mut locks, None, budget)?;
    }
    if let Some(protection) = protection {
        for node in &protection.nodes {
            budget.checkpoint()?;
            let id = match node {
                Node::Block(id) | Node::If { block: id, .. } => Some(*id),
                Node::Loop { header, .. } => Some(*header),
                Node::Synchronized { enter, scope, .. } => {
                    let scope = protection
                        .monitors
                        .as_ref()
                        .ok_or_else(broken)?
                        .scope(*scope)?;
                    *locks.get_mut(ix(enter.0)?).ok_or_else(broken)? = Some(scope.enter);
                    Some(*enter)
                }
                _ => None,
            };
            if let Some(id) = id {
                *expected.get_mut(ix(id.0)?).ok_or_else(broken)? = true;
            }
        }
    }
    if !matches!(
        lexical.event(steps, 0, &mut used, budget)?,
        Step::Statements(BlockId(0))
    ) {
        return Err(broken());
    }
    for (at, step) in steps.iter().enumerate() {
        budget.checkpoint()?;
        match step {
            Step::Statements(id) => {
                let index = ix(id.0)?;
                if !expected.get(index).copied().ok_or_else(broken)? || seen[index] {
                    return Err(broken());
                }
                seen[index] = true;
                let next = lexical.event(steps, lexical.next[at], &mut used, budget)?;
                let valid = match &case(storage, *id)?.transfer {
                    Transfer::Jump(edge) => matches_copies(next, edge),
                    Transfer::Branch { .. } => matches!(next,Step::If(actual) if actual == id),
                    transfer => matches_completion(next, transfer),
                };
                if !valid {
                    return Err(broken());
                }
            }
            Step::Copies { source, edge: id } => {
                let edge = edge(storage, *source, *id)?;
                if !matches!(lexical.event(steps,lexical.next[at],&mut used,budget)?,Step::Statements(target) if *target == edge.target)
                {
                    return Err(broken());
                }
            }
            Step::If(id) => {
                let Transfer::Branch { yes, no, .. } = &case(storage, *id)?.transfer else {
                    return Err(broken());
                };
                if !matches_copies(
                    lexical.event(steps, lexical.next[at], &mut used, budget)?,
                    yes,
                ) || !matches_copies(
                    lexical.event(
                        steps,
                        lexical.alternative[at].ok_or_else(broken)?,
                        &mut used,
                        budget,
                    )?,
                    no,
                ) {
                    return Err(broken());
                }
            }
            Step::Synchronized(lock) => {
                let Some(Step::Statements(id)) =
                    at.checked_sub(1).and_then(|previous| steps.get(previous))
                else {
                    return Err(broken());
                };
                let operation = locks
                    .get_mut(ix(id.0)?)
                    .ok_or_else(broken)?
                    .take()
                    .ok_or_else(broken)?;
                let mut matched = false;
                for statement in &case(storage, *id)?.statements {
                    budget.checkpoint()?;
                    if let super::calls::Statement::Monitor {
                        operation: actual,
                        kind: crate::p2_flow::MonitorKind::Enter,
                        lock: expected,
                    } = statement
                    {
                        if matched || *actual != operation || lock != expected {
                            return Err(broken());
                        }
                        matched = true;
                    }
                }
                if !matched {
                    return Err(broken());
                }
                let protection = protection.ok_or_else(broken)?;
                let mut inner = None;
                for node in &protection.nodes {
                    budget.checkpoint()?;
                    if let Node::Synchronized { enter, body, .. } = node
                        && enter == id
                        && inner.replace(*body).is_some()
                    {
                        return Err(broken());
                    }
                }
                let blocks = crate::p6_regions::protection::blocks(
                    &protection.nodes,
                    &[inner.ok_or_else(broken)?],
                    budget,
                )?;
                let end = lexical.close[at].ok_or_else(broken)?;
                let mut membership = Vec::new();
                for _ in &storage.cases {
                    budget.checkpoint()?;
                    push(&mut membership, false, budget)?;
                }
                for block in blocks {
                    budget.checkpoint()?;
                    *membership.get_mut(ix(block.0)?).ok_or_else(broken)? = true;
                }
                for step in &steps[at + 1..end] {
                    budget.checkpoint()?;
                    if let Step::Statements(block) = step {
                        let present = membership.get_mut(ix(block.0)?).ok_or_else(broken)?;
                        if !*present {
                            return Err(broken());
                        }
                        *present = false;
                    }
                }
                for present in membership {
                    budget.checkpoint()?;
                    if present {
                        return Err(broken());
                    }
                }
            }
            Step::Try(id) => {
                let protection = protection.ok_or_else(broken)?;
                let seen = seen_tries.get_mut(ix(id.0)?).ok_or_else(broken)?;
                if *seen {
                    return Err(broken());
                }
                *seen = true;
                let Some(Node::Try { body, catches }) = protection.nodes.get(ix(id.0)?) else {
                    return Err(broken());
                };
                let body_end = lexical.try_body_end[at].ok_or_else(broken)?;
                membership(storage, protection, *body, &steps[at + 1..body_end], budget)?;
                let mut cursor = body_end + 1;
                for catch in catches {
                    budget.checkpoint()?;
                    let Some(Step::Catch {
                        alternatives,
                        handler,
                    }) = steps.get(cursor)
                    else {
                        return Err(broken());
                    };
                    if alternatives.len() != catch.clauses.len()
                        || !matches!(catch.clauses.first(),Some(clause) if clause.handler == *handler)
                    {
                        return Err(broken());
                    }
                    for (clause, actual) in catch.clauses.iter().zip(alternatives) {
                        let mut expected_name = None;
                        for name in names {
                            budget.checkpoint()?;
                            if name.kind == clause.kind {
                                expected_name = Some(name.source.as_str());
                                break;
                            }
                        }
                        if expected_name != Some(actual.as_str()) {
                            return Err(broken());
                        }
                    }
                    let end = lexical.close[cursor].ok_or_else(broken)?;
                    membership(
                        storage,
                        protection,
                        catch.body,
                        &steps[cursor + 1..end],
                        budget,
                    )?;
                    cursor = end + 1;
                }
                if Some(cursor) != lexical.close[at]
                    || !matches!(steps.get(cursor), Some(Step::EndTry))
                {
                    return Err(broken());
                }
            }
            Step::Catch { handler, .. } => {
                let next = lexical.event(steps, lexical.next[at], &mut used, budget)?;
                if !matches!(next, Step::Statements(id) if id == handler) {
                    return Err(broken());
                }
            }
            _ => {}
        }
    }
    if let Some(protection) = protection {
        for (node, seen) in protection.nodes.iter().zip(seen_tries) {
            budget.checkpoint()?;
            if matches!(node, Node::Try { .. }) != seen {
                return Err(broken());
            }
        }
    }
    for (step, used) in steps.iter().zip(used) {
        budget.checkpoint()?;
        if matches!(
            step,
            Step::Statements(_)
                | Step::Copies { .. }
                | Step::If(_)
                | Step::Return(_)
                | Step::ReturnVoid
                | Step::InitializerExit
                | Step::Throw(_)
                | Step::Rethrow(_)
        ) && !used
        {
            return Err(broken());
        }
    }
    for ((expected, seen), lock) in expected.iter().zip(&seen).zip(&locks) {
        budget.checkpoint()?;
        if expected != seen || lock.is_some() {
            return Err(broken());
        }
    }
    Ok(())
}
