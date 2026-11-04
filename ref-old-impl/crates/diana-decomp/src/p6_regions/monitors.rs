use crate::p2_flow::{BlockId, MonitorKind, Op, OpId, Terminal};
use crate::p3_ssa::ValueId;
use crate::p4_types::reference_roots::Root;
use crate::p5_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RequestError, RunContext};

#[derive(Clone, Copy)]
pub(crate) struct CleanupId(u32);
#[derive(Clone, Copy)]
pub(crate) struct ScopeId(pub u32);
pub(crate) struct Cleanup {
    pub handler: BlockId,
    pub exit: OpId,
    pub blocks: Vec<BlockId>,
    pub root: Root,
}
pub(crate) struct Scope {
    pub enter: OpId,
    pub block: BlockId,
    pub lock: ValueId,
    pub root: Root,
    pub cleanup: CleanupId,
    pub body: Vec<BlockId>,
    pub follow: BlockId,
}
pub(crate) struct Monitors {
    // Acquisition sites keep reentrant layers distinct even when roots agree.
    states: Vec<Option<Vec<OpId>>>,
    pub scopes: Vec<Scope>,
    pub cleanups: Vec<Cleanup>,
}
impl Monitors {
    pub(crate) fn cleanup(&self, id: CleanupId) -> Result<&Cleanup, RequestError> {
        self.cleanups
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant("foreign monitor cleanup"))
    }
    pub(crate) fn scope(&self, id: ScopeId) -> Result<&Scope, RequestError> {
        self.scopes
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant("foreign monitor scope"))
    }
    pub(crate) fn matches_entry(
        &self,
        block: BlockId,
        stack: &[OpId],
        budget: &mut RunContext<'_>,
    ) -> Result<bool, RequestError> {
        let Some(Some(original)) = self.states.get(ix(block.0)?) else {
            return Ok(false);
        };
        same(original, stack, budget)
    }
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn unsupported(pc: CodeUnitOffset) -> RequestError {
    RequestError::MonitorScopeUnproved { pc }
}
fn copy(values: &[OpId], budget: &mut RunContext<'_>) -> Result<Vec<OpId>, RequestError> {
    let mut result = Vec::new();
    for &value in values {
        budget.checkpoint()?;
        push(&mut result, value, budget)?;
    }
    Ok(result)
}
fn contains(
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
fn same(a: &[OpId], b: &[OpId], budget: &mut RunContext<'_>) -> Result<bool, RequestError> {
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
fn root(
    storage: &StorageBody<'_>,
    value: ValueId,
    pc: CodeUnitOffset,
) -> Result<Root, RequestError> {
    storage
        .typed
        .roots
        .one(value)?
        .ok_or_else(|| unsupported(pc))
}
fn cleanup(
    storage: &StorageBody<'_>,
    handler: BlockId,
    budget: &mut RunContext<'_>,
) -> Result<Option<Cleanup>, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut block = handler;
    let mut blocks = Vec::new();
    let mut exception = None;
    let mut exit = None;
    loop {
        budget.checkpoint()?;
        if contains(&blocks, block, budget)? {
            return Ok(None);
        }
        push(&mut blocks, block, budget)?;
        for &id in &ssa.bound[ix(block.0)?].ops {
            budget.checkpoint()?;
            let op = ssa.operation(id)?;
            match op.op {
                Op::Exception(_) if block == handler && exception.is_none() => {
                    exception = op.result
                }
                Op::Nop | Op::Move(..) => {}
                Op::Monitor(MonitorKind::Exit, _) if exit.is_none() => {
                    exit = Some((id, root(storage, op.input(0)?, op.pc)?))
                }
                _ => return Ok(None),
            }
        }
        match ssa.blocks[ix(block.0)?].terminal {
            Terminal::Jump(target) => block = target,
            Terminal::Throw(_) => {
                let (Some(exception), Some((exit, root))) = (exception, exit) else {
                    return Ok(None);
                };
                let Some(value) = ssa.bound[ix(block.0)?].terminal else {
                    return Ok(None);
                };
                if storage.typed.roots.one(value)? != Some(Root::Definition(exception)) {
                    return Ok(None);
                }
                return Ok(Some(Cleanup {
                    handler,
                    exit,
                    blocks,
                    root,
                }));
            }
            _ => return Ok(None),
        }
    }
}
fn transfer(
    states: &mut [Option<Vec<OpId>>],
    queue: &mut Vec<BlockId>,
    target: BlockId,
    stack: &[OpId],
    pc: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    match &states[ix(target.0)?] {
        Some(old) => {
            if !same(old, stack, budget)? {
                return Err(unsupported(pc));
            }
        }
        None => {
            states[ix(target.0)?] = Some(copy(stack, budget)?);
            push(queue, target, budget)?;
        }
    }
    Ok(())
}
fn enter_root(storage: &StorageBody<'_>, id: OpId) -> Result<Root, RequestError> {
    let op = storage.typed.ssa.operation(id)?;
    root(storage, op.input(0)?, op.pc)
}
fn check_fresh(
    storage: &StorageBody<'_>,
    result: Option<ValueId>,
    stack: &[OpId],
    pc: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    // A static SSA producer is not one dynamic object if it reexecutes while held.
    if let Some(result) = result {
        for &enter in stack {
            budget.checkpoint()?;
            if enter_root(storage, enter)? == Root::Definition(result) {
                return Err(unsupported(pc));
            }
        }
    }
    Ok(())
}
pub(super) fn build(
    storage: &StorageBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Option<Monitors>, RequestError> {
    let ssa = &storage.typed.ssa;
    let mut any = false;
    for op in &ssa.operations {
        budget.checkpoint()?;
        any |= matches!(op.op, Op::Monitor(..));
    }
    if !any {
        return Ok(None);
    }
    let mut cleanups = Vec::new();
    for binding in &storage.typed.exceptions {
        budget.checkpoint()?;
        if let Some(cleanup) = cleanup(storage, binding.handler, budget)? {
            push(&mut cleanups, cleanup, budget)?;
        }
    }
    let mut states = Vec::new();
    for _ in &ssa.blocks {
        budget.checkpoint()?;
        push(&mut states, None, budget)?;
    }
    let mut queue = Vec::new();
    transfer(
        &mut states,
        &mut queue,
        BlockId(0),
        &[],
        CodeUnitOffset(0),
        budget,
    )?;
    let mut cursor = 0usize;
    while cursor < queue.len() {
        budget.checkpoint()?;
        let block = queue[cursor];
        cursor += 1;
        let mut stack = copy(
            states[ix(block.0)?]
                .as_deref()
                .ok_or(RequestError::InternalInvariant("monitor lost entry stack"))?,
            budget,
        )?;
        for &id in &ssa.bound[ix(block.0)?].ops {
            budget.checkpoint()?;
            let op = ssa.operation(id)?;
            check_fresh(storage, op.result, &stack, op.pc, budget)?;
            if let Op::Monitor(MonitorKind::Exit, _) = op.op {
                let Some(enter) = stack.pop() else {
                    return Err(unsupported(op.pc));
                };
                if enter_root(storage, enter)? != root(storage, op.input(0)?, op.pc)? {
                    return Err(unsupported(op.pc));
                }
                // Successful acquisition plus an exact top-layer match discharges
                // this exit's exceptional edge; P2 retains the original edge.
                continue;
            }
            if op.op.effects().may_throw {
                exceptions(
                    storage,
                    &mut states,
                    &mut queue,
                    block,
                    op.pc,
                    &stack,
                    budget,
                )?;
            }
            if matches!(op.op, Op::Monitor(MonitorKind::Enter, _)) {
                let _ = root(storage, op.input(0)?, op.pc)?;
                push(&mut stack, id, budget)?;
            }
        }
        let original = &ssa.blocks[ix(block.0)?];
        if matches!(original.terminal, Terminal::Throw(_)) {
            exceptions(
                storage,
                &mut states,
                &mut queue,
                block,
                original.terminal_pc,
                &stack,
                budget,
            )?;
        }
        if matches!(
            original.terminal,
            Terminal::Return(_) | Terminal::ReturnVoid
        ) && !stack.is_empty()
        {
            return Err(unsupported(original.terminal_pc));
        }
        for edge in &ssa.edges {
            budget.checkpoint()?;
            if edge.source == block {
                transfer(
                    &mut states,
                    &mut queue,
                    edge.target,
                    &stack,
                    original.terminal_pc,
                    budget,
                )?;
            }
        }
    }
    let mut scopes = Vec::new();
    for (index, block) in ssa.bound.iter().enumerate() {
        let source = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        for &id in &block.ops {
            budget.checkpoint()?;
            let op = ssa.operation(id)?;
            if !matches!(op.op, Op::Monitor(MonitorKind::Enter, _)) {
                continue;
            }
            let lock = op.input(0)?;
            let identity = root(storage, lock, op.pc)?;
            let mut body = Vec::new();
            let mut pending = Vec::new();
            let mut follow = None;
            let mut selected = None;
            for edge in &ssa.edges {
                budget.checkpoint()?;
                if edge.source == source {
                    push(&mut pending, edge.target, budget)?;
                }
            }
            while let Some(block) = pending.pop() {
                budget.checkpoint()?;
                if contains(&body, block, budget)? {
                    continue;
                }
                let stack = states[ix(block.0)?]
                    .as_deref()
                    .ok_or_else(|| unsupported(op.pc))?;
                let mut holding = false;
                for &enter in stack {
                    budget.checkpoint()?;
                    holding |= enter == id;
                }
                if !holding {
                    if follow.is_some_and(|old| old != block) {
                        return Err(unsupported(op.pc));
                    }
                    follow = Some(block);
                    continue;
                }
                push(&mut body, block, budget)?;
                for edge in &ssa.edges {
                    budget.checkpoint()?;
                    if edge.source == block {
                        push(&mut pending, edge.target, budget)?;
                    }
                }
                for edge in &ssa.exception_edges {
                    budget.checkpoint()?;
                    if edge.source != block || edge.kind != crate::input::protection::CatchType::All
                    {
                        continue;
                    }
                    for (index, cleanup) in cleanups.iter().enumerate() {
                        budget.checkpoint()?;
                        if cleanup.handler == edge.target && cleanup.root == identity {
                            // An inner reentrant cleanup is not the outer scope's release.
                            if stack.last() != Some(&id) {
                                continue;
                            }
                            if selected.is_some_and(|old| old != index) {
                                let mut site = None;
                                for operation in &ssa.operations {
                                    budget.checkpoint()?;
                                    if operation.pc == edge.pc {
                                        site = Some(operation);
                                        break;
                                    }
                                }
                                if site.is_some_and(|site| {
                                    matches!(site.op, Op::Monitor(MonitorKind::Exit, _))
                                }) {
                                    continue;
                                }
                                return Err(unsupported(op.pc));
                            }
                            selected = Some(index);
                        }
                    }
                }
            }
            let (Some(cleanup), Some(follow)) = (selected, follow) else {
                return Err(unsupported(op.pc));
            };
            push(
                &mut scopes,
                Scope {
                    enter: id,
                    block: source,
                    lock,
                    root: identity,
                    cleanup: CleanupId(
                        u32::try_from(cleanup).map_err(|_| RequestError::PlatformCapacity)?,
                    ),
                    body,
                    follow,
                },
                budget,
            )?;
        }
    }
    let result = Monitors {
        states,
        scopes,
        cleanups,
    };
    verify(storage, &result, budget)?;
    Ok(Some(result))
}
fn exceptions(
    storage: &StorageBody<'_>,
    states: &mut [Option<Vec<OpId>>],
    queue: &mut Vec<BlockId>,
    source: BlockId,
    pc: CodeUnitOffset,
    stack: &[OpId],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let mut all = false;
    for edge in &storage.typed.ssa.exception_edges {
        budget.checkpoint()?;
        if edge.source == source && edge.pc == pc {
            all |= edge.kind == crate::input::protection::CatchType::All;
            transfer(states, queue, edge.target, stack, pc, budget)?;
        }
    }
    if !stack.is_empty() && !all {
        return Err(unsupported(pc));
    }
    Ok(())
}

pub(crate) fn verify(
    storage: &StorageBody<'_>,
    plan: &Monitors,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let bug = || {
        RequestError::InternalInvariant(
            "monitor certificate changed stack, root, release or coverage",
        )
    };
    let ssa = &storage.typed.ssa;
    if plan.states.len() != ssa.blocks.len()
        || plan.states.first().and_then(Option::as_deref) != Some(&[])
    {
        return Err(bug());
    }
    for (index, block) in ssa.bound.iter().enumerate() {
        budget.checkpoint()?;
        let source = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let Some(entry) = &plan.states[index] else {
            let mut covered = false;
            for cleanup in &plan.cleanups {
                budget.checkpoint()?;
                covered |= contains(&cleanup.blocks, source, budget)?;
            }
            if !covered {
                return Err(bug());
            }
            continue;
        };
        let mut stack = copy(entry, budget)?;
        for &id in &block.ops {
            budget.checkpoint()?;
            let op = ssa.operation(id)?;
            match check_fresh(storage, op.result, &stack, op.pc, budget) {
                Ok(()) => {}
                Err(RequestError::MonitorScopeUnproved { .. }) => return Err(bug()),
                Err(error) => return Err(error),
            }
            match op.op {
                Op::Monitor(MonitorKind::Exit, _) => {
                    let Some(enter) = stack.pop() else {
                        return Err(bug());
                    };
                    if enter_root(storage, enter)? != root(storage, op.input(0)?, op.pc)? {
                        return Err(bug());
                    }
                }
                _ => {
                    if op.op.effects().may_throw {
                        verify_exception_states(storage, plan, source, op.pc, &stack, budget)?;
                    }
                    if matches!(op.op, Op::Monitor(MonitorKind::Enter, _)) {
                        push(&mut stack, id, budget)?;
                    }
                }
            }
        }
        let original = &ssa.blocks[index];
        if matches!(
            original.terminal,
            Terminal::Return(_) | Terminal::ReturnVoid
        ) && !stack.is_empty()
        {
            return Err(bug());
        }
        if matches!(original.terminal, Terminal::Throw(_)) {
            verify_exception_states(storage, plan, source, original.terminal_pc, &stack, budget)?;
        }
        for edge in &ssa.edges {
            budget.checkpoint()?;
            if edge.source == source {
                let Some(next) = plan.states[ix(edge.target.0)?].as_deref() else {
                    return Err(bug());
                };
                if !same(next, &stack, budget)? {
                    return Err(bug());
                }
            }
        }
    }
    for cleanup in &plan.cleanups {
        budget.checkpoint()?;
        if !verify_cleanup(storage, cleanup, budget)? {
            return Err(bug());
        }
    }
    for scope in &plan.scopes {
        budget.checkpoint()?;
        let op = ssa.operation(scope.enter)?;
        if !matches!(op.op, Op::Monitor(MonitorKind::Enter, _))
            || op.input(0)? != scope.lock
            || root(storage, scope.lock, op.pc)? != scope.root
            || plan
                .cleanups
                .get(ix(scope.cleanup.0)?)
                .is_none_or(|cleanup| cleanup.root != scope.root)
        {
            return Err(bug());
        }
    }
    Ok(())
}
fn verify_cleanup(
    storage: &StorageBody<'_>,
    proof: &Cleanup,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let ssa = &storage.typed.ssa;
    if proof.blocks.first() != Some(&proof.handler) {
        return Ok(false);
    }
    let mut exception = None;
    let mut released = false;
    for (index, &block) in proof.blocks.iter().enumerate() {
        budget.checkpoint()?;
        for &id in &ssa.bound[ix(block.0)?].ops {
            budget.checkpoint()?;
            let op = ssa.operation(id)?;
            match op.op {
                Op::Exception(_) if index == 0 && exception.is_none() => exception = op.result,
                Op::Nop | Op::Move(..) => {}
                Op::Monitor(MonitorKind::Exit, _) if !released && id == proof.exit => {
                    if root(storage, op.input(0)?, op.pc)? != proof.root {
                        return Ok(false);
                    }
                    released = true;
                }
                _ => return Ok(false),
            }
        }
        let next = proof.blocks.get(index + 1).copied();
        match ssa.blocks[ix(block.0)?].terminal {
            Terminal::Jump(target) if next == Some(target) => {}
            Terminal::Throw(_) if next.is_none() && released => {
                let (Some(exception), Some(value)) = (exception, ssa.bound[ix(block.0)?].terminal)
                else {
                    return Ok(false);
                };
                return Ok(storage.typed.roots.one(value)? == Some(Root::Definition(exception)));
            }
            _ => return Ok(false),
        }
    }
    Ok(false)
}
fn verify_exception_states(
    storage: &StorageBody<'_>,
    plan: &Monitors,
    source: BlockId,
    pc: CodeUnitOffset,
    stack: &[OpId],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let mut all = false;
    for edge in &storage.typed.ssa.exception_edges {
        budget.checkpoint()?;
        if edge.source != source || edge.pc != pc {
            continue;
        }
        all |= edge.kind == crate::input::protection::CatchType::All;
        let Some(next) = plan.states[ix(edge.target.0)?].as_deref() else {
            return Err(RequestError::InternalInvariant("exception lost lock stack"));
        };
        if !same(stack, next, budget)? {
            return Err(RequestError::InternalInvariant(
                "exception changed lock stack",
            ));
        }
    }
    if !stack.is_empty() && !all {
        return Err(RequestError::InternalInvariant(
            "exception escapes held lock",
        ));
    }
    Ok(())
}
