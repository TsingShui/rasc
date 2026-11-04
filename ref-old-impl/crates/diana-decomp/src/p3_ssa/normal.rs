use crate::input::RawMethodView;
use crate::p2_flow::{
    Block, BlockId, CallId, CallSite, DeclarationType, Edge, EdgeId, FlowBody, Op, OpId,
    ResultType, Switch, destination, terminal_other, terminal_register,
};
use crate::raw::LayoutKind;
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RegisterId, RequestError, RunContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValueId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Sym {
    High(ValueId),
    Value(ValueId),
    Bad,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Definition {
    ExceptionAt(OpId),
    Parameter(u32),
    OpResult(OpId),
    Phi,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Incoming {
    EntrySeed,
    Normal(EdgeId),
    Exceptional(crate::p2_flow::protection::ExceptionEdgeId),
}
pub(crate) struct Phi {
    pub block: BlockId,
    pub register: RegisterId,
    pub result: ValueId,
    pub inputs: Vec<(Incoming, Sym)>,
}
pub(crate) struct BoundOp {
    pub id: OpId,
    pub pc: CodeUnitOffset,
    pub op: Op,
    pub inputs: Vec<ValueId>,
    pub result: Option<ValueId>,
}
impl BoundOp {
    pub(crate) fn input(&self, index: usize) -> Result<ValueId, RequestError> {
        self.inputs
            .get(index)
            .copied()
            .ok_or(RequestError::InternalInvariant(
                "bound operation lost an input",
            ))
    }
}
pub(crate) struct BoundBlock {
    pub ops: Vec<OpId>,
    pub terminal: Option<ValueId>,
    pub other: Option<ValueId>,
}
crate::substrate::checked::checked_product!(SsaBody, SsaBodyData);
pub(crate) struct SsaBodyData<'a> {
    pub object_creations: Vec<crate::p2_flow::ObjectCreation>,
    pub array_creations: Vec<crate::p2_flow::ArrayCreation>,
    pub filled_arrays: Vec<crate::p2_flow::FilledArraySite>,
    pub fresh_fills: Vec<crate::p2_flow::FreshFillSite>,
    pub empty_fills: Vec<crate::p2_flow::EmptyFillSite>,
    pub casts: Vec<crate::p2_flow::CastSite>,
    pub strings: Vec<crate::p2_flow::StringLiteral<'a>>,
    pub frame: crate::p2_flow::MethodFrame,
    pub method_monitor: Option<crate::p2_flow::MethodMonitor>,
    _sealed: (),
    pub switches: Vec<Switch>,
    pub calls: Vec<CallSite>,
    call_ops: Vec<OpId>,
    pub operations: Vec<BoundOp>,
    pub result: ResultType,
    pub raw: RawMethodView<'a>,
    /// The P1-disassembled original directory carried through P2. Later stages
    /// read decoded layouts and operands from it, not from a second decode.
    pub decoded: crate::input::DecodedMethod<'a>,
    pub parameters: Vec<DeclarationType>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub exception_edges: Vec<crate::p2_flow::protection::ExceptionEdge>,
    pub protection: crate::input::protection::CheckedProtection,
    pub values: Vec<Definition>,
    pub phis: Vec<Phi>,
    pub bound: Vec<BoundBlock>,
}
impl SsaBody<'_> {
    pub(crate) fn operation(&self, id: OpId) -> Result<&BoundOp, RequestError> {
        self.operations
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant(
                "foreign operation identity",
            ))
    }
    pub(crate) fn call(&self, id: CallId) -> Result<&BoundOp, RequestError> {
        let op = self
            .call_ops
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant("foreign call identity"))?;
        self.operation(*op)
    }
    pub(crate) fn is_zero(&self, id: ValueId) -> Result<bool, RequestError> {
        let definition = self
            .values
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant(
                "foreign constant definition",
            ))?;
        match definition {
            Definition::OpResult(op) => Ok(matches!(self.operation(*op)?.op, Op::Constant(_, 0))),
            Definition::Parameter(_) | Definition::Phi | Definition::ExceptionAt(_) => Ok(false),
        }
    }
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn add_register(
    set: &mut Vec<RegisterId>,
    register: RegisterId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for old in set.iter() {
        budget.checkpoint()?;
        if *old == register {
            return Ok(false);
        }
    }
    push(set, register, budget)?;
    Ok(true)
}
pub(super) fn lookup(
    env: &[(RegisterId, Sym)],
    register: RegisterId,
    budget: &mut RunContext<'_>,
) -> Result<Sym, RequestError> {
    for &(slot, value) in env {
        budget.checkpoint()?;
        if slot == register {
            return Ok(value);
        }
    }
    Ok(Sym::Bad)
}
fn write(
    env: &mut Vec<(RegisterId, Sym)>,
    register: RegisterId,
    value: Sym,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for entry in env.iter_mut() {
        budget.checkpoint()?;
        if entry.0 == register {
            entry.1 = value;
            return Ok(());
        }
    }
    push(env, (register, value), budget)
}
pub(super) fn value(
    sym: Sym,
    register: RegisterId,
    pc: CodeUnitOffset,
) -> Result<ValueId, RequestError> {
    match sym {
        Sym::Value(value) => Ok(value),
        Sym::High(_) => Err(RequestError::InvalidWideRead { register, pc }),
        Sym::Bad => Err(RequestError::UndefinedRegister { register, pc }),
    }
}
pub(super) fn commit(
    env: &mut Vec<(RegisterId, Sym)>,
    register: RegisterId,
    value: ValueId,
    width: crate::p2_flow::ValueWidth,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    write(env, register, Sym::Value(value), budget)?;
    if width == crate::p2_flow::ValueWidth::Two {
        write(
            env,
            super::word_ownership::high(register)?,
            Sym::High(value),
            budget,
        )?;
    }
    Ok(())
}
// A live low slot reserves a symbolic following word only where no separate
// live slot exists. Incoming-edge ownership and grounding discharge this
// candidate relation before the SSA body is exposed.
fn phi_highs(
    phis: &[Phi],
    block: BlockId,
    registers: u16,
    env: &mut Vec<(RegisterId, Sym)>,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for phi in phis {
        budget.checkpoint()?;
        if phi.block != block {
            continue;
        }
        let high = super::word_ownership::high(phi.register)?;
        if high.0 < u32::from(registers) && lookup(env, high, budget)? == Sym::Bad {
            write(env, high, Sym::High(phi.result), budget)?;
        }
    }
    Ok(())
}
fn capture_exceptional(
    flow: &FlowBody<'_>,
    live: &[Vec<RegisterId>],
    source: BlockId,
    pc: CodeUnitOffset,
    env: &[(RegisterId, Sym)],
    snapshots: &mut [Vec<(RegisterId, Sym)>],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for (index, edge) in flow.exception_edges.iter().enumerate() {
        budget.checkpoint()?;
        if edge.source != source || edge.pc != pc {
            continue;
        }
        for &register in &live[ix(edge.target.0)?] {
            budget.checkpoint()?;
            write(
                &mut snapshots[index],
                register,
                lookup(env, register, budget)?,
                budget,
            )?;
            let high = super::word_ownership::high(register)?;
            if high.0 < u32::from(flow.raw.registers_size()) {
                write(
                    &mut snapshots[index],
                    high,
                    lookup(env, high, budget)?,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}
pub(super) struct EntrySeeds {
    pub values: Vec<Definition>,
    pub env: Vec<(RegisterId, Sym)>,
}
pub(super) fn seed_parameters(
    count: usize,
    width: impl Fn(usize) -> crate::p2_flow::ValueWidth,
    registers: u16,
    budget: &mut RunContext<'_>,
) -> Result<EntrySeeds, RequestError> {
    let mut words = 0u32;
    for i in 0..count {
        budget.checkpoint()?;
        words = words
            .checked_add(width(i).words())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    let mut register = RegisterId(
        u32::from(registers)
            .checked_sub(words)
            .ok_or(RequestError::InternalInvariant("parameter frame underflow"))?,
    );
    let mut values = Vec::new();
    let mut seeds = Vec::new();
    for i in 0..count {
        budget.checkpoint()?;
        let id = ValueId(u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?);
        push(
            &mut values,
            Definition::Parameter(u32::try_from(i).map_err(|_| RequestError::PlatformCapacity)?),
            budget,
        )?;
        commit(&mut seeds, register, id, width(i), budget)?;
        register = RegisterId(
            register
                .0
                .checked_add(width(i).words())
                .ok_or(RequestError::PlatformCapacity)?,
        );
    }
    Ok(EntrySeeds { values, env: seeds })
}
pub(super) fn bind_inputs(
    operation: crate::p2_flow::Operation,
    calls: &[CallSite],
    filled: &[crate::p2_flow::FilledArraySite],
    env: &[(RegisterId, Sym)],
    budget: &mut RunContext<'_>,
) -> Result<Vec<ValueId>, RequestError> {
    let mut inputs = Vec::new();
    for register in operation.kind.reads(calls, filled)? {
        budget.checkpoint()?;
        push(
            &mut inputs,
            value(lookup(env, register, budget)?, register, operation.pc)?,
            budget,
        )?;
    }
    Ok(inputs)
}
pub(super) fn define_result(
    operation: crate::p2_flow::Operation,
    calls: &[CallSite],
    env: &mut Vec<(RegisterId, Sym)>,
    values: &mut Vec<Definition>,
    budget: &mut RunContext<'_>,
) -> Result<Option<ValueId>, RequestError> {
    if let Some(register) = destination(operation.kind) {
        let id = ValueId(u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?);
        push(
            values,
            if matches!(operation.kind, Op::Exception(_)) {
                Definition::ExceptionAt(operation.id)
            } else {
                Definition::OpResult(operation.id)
            },
            budget,
        )?;
        commit(
            env,
            register,
            id,
            operation.kind.result_width(calls)?,
            budget,
        )?;
        Ok(Some(id))
    } else {
        Ok(None)
    }
}
pub(crate) fn build<'a>(
    _schedule: &crate::engine::SchedulePermit,
    flow: FlowBody<'a>,
    budget: &mut RunContext<'_>,
) -> Result<SsaBody<'a>, RequestError> {
    build_impl(flow, budget)
}

fn build_impl<'a>(
    flow: FlowBody<'a>,
    budget: &mut RunContext<'_>,
) -> Result<SsaBody<'a>, RequestError> {
    let mut live = Vec::new();
    for _ in &flow.blocks {
        budget.checkpoint()?;
        push(&mut live, Vec::new(), budget)?;
    }
    loop {
        let mut changed = false;
        for index in (0..flow.blocks.len()).rev() {
            budget.checkpoint()?;
            let block = &flow.blocks[index];
            let mut needed = Vec::new();
            for edge in &flow.edges {
                budget.checkpoint()?;
                if ix(edge.source.0)? == index {
                    for &register in &live[ix(edge.target.0)?] {
                        budget.checkpoint()?;
                        add_register(&mut needed, register, budget)?;
                    }
                }
            }
            if matches!(block.terminal, crate::p2_flow::Terminal::Throw(_)) {
                for edge in &flow.exception_edges {
                    budget.checkpoint()?;
                    if ix(edge.source.0)? == index && edge.pc == block.terminal_pc {
                        for &register in &live[ix(edge.target.0)?] {
                            budget.checkpoint()?;
                            add_register(&mut needed, register, budget)?;
                        }
                    }
                }
            }
            for register in [
                terminal_register(block.terminal),
                terminal_other(block.terminal),
            ]
            .into_iter()
            .flatten()
            {
                budget.checkpoint()?;
                add_register(&mut needed, register, budget)?;
            }
            for operation in block.ops.iter().rev() {
                budget.checkpoint()?;
                if let Some(destination) = destination(operation.kind) {
                    let mut position = 0;
                    while position < needed.len() {
                        budget.checkpoint()?;
                        if needed[position].0 >= destination.0
                            && needed[position].0
                                < destination
                                    .0
                                    .checked_add(operation.kind.result_width(&flow.calls)?.words())
                                    .ok_or(RequestError::PlatformCapacity)?
                        {
                            needed.swap_remove(position);
                        } else {
                            position += 1;
                        }
                    }
                }
                for register in operation.kind.reads(&flow.calls, &flow.filled_arrays)? {
                    budget.checkpoint()?;
                    add_register(&mut needed, register, budget)?;
                }
                for edge in &flow.exception_edges {
                    budget.checkpoint()?;
                    if ix(edge.source.0)? == index && edge.pc == operation.pc {
                        for &register in &live[ix(edge.target.0)?] {
                            budget.checkpoint()?;
                            add_register(&mut needed, register, budget)?;
                        }
                    }
                }
            }
            for register in needed {
                budget.checkpoint()?;
                changed |= add_register(&mut live[index], register, budget)?;
            }
        }
        if !changed {
            break;
        }
    }
    let EntrySeeds {
        mut values,
        env: seeds,
    } = seed_parameters(
        flow.parameters.len(),
        |i| flow.parameters[i].width(),
        flow.raw.registers_size(),
        budget,
    )?;
    let mut phis = Vec::new();
    for (index, registers) in live.iter().enumerate() {
        for &register in registers {
            budget.checkpoint()?;
            let result =
                ValueId(u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?);
            push(&mut values, Definition::Phi, budget)?;
            push(
                &mut phis,
                Phi {
                    block: BlockId(
                        u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                    ),
                    register,
                    result,
                    inputs: Vec::new(),
                },
                budget,
            )?;
        }
    }
    let mut bound = Vec::new();
    let mut operations = Vec::new();
    let mut call_ops = Vec::new();
    for _ in &flow.calls {
        budget.checkpoint()?;
        push(&mut call_ops, None, budget)?;
    }
    let mut outgoing = Vec::new();
    let mut exceptional = Vec::new();
    for _ in &flow.exception_edges {
        budget.checkpoint()?;
        push(&mut exceptional, Vec::new(), budget)?;
    }
    for (index, block) in flow.blocks.iter().enumerate() {
        budget.checkpoint()?;
        let block_id = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut env = Vec::new();
        for phi in &phis {
            budget.checkpoint()?;
            if phi.block == block_id {
                write(&mut env, phi.register, Sym::Value(phi.result), budget)?;
            }
        }
        phi_highs(&phis, block_id, flow.raw.registers_size(), &mut env, budget)?;
        let mut ops = Vec::new();
        for operation in &block.ops {
            budget.checkpoint()?;
            if ix(operation.id.0)? != operations.len() {
                return Err(RequestError::InternalInvariant(
                    "flow operation identity changed",
                ));
            }
            let inputs = bind_inputs(*operation, &flow.calls, &flow.filled_arrays, &env, budget)?;
            capture_exceptional(
                &flow,
                &live,
                block_id,
                operation.pc,
                &env,
                &mut exceptional,
                budget,
            )?;
            let result = define_result(*operation, &flow.calls, &mut env, &mut values, budget)?;
            if let Op::Call(call, _) | Op::Initialize(call) = operation.kind {
                let recorded = call_ops
                    .get_mut(ix(call.0)?)
                    .ok_or(RequestError::InternalInvariant("foreign flow call"))?;
                if recorded.replace(operation.id).is_some() {
                    return Err(RequestError::InternalInvariant("call was bound twice"));
                }
            }
            push(
                &mut operations,
                BoundOp {
                    id: operation.id,
                    pc: operation.pc,
                    op: operation.kind,
                    inputs,
                    result,
                },
                budget,
            )?;
            push(&mut ops, operation.id, budget)?;
        }
        if matches!(block.terminal, crate::p2_flow::Terminal::Throw(_)) {
            capture_exceptional(
                &flow,
                &live,
                block_id,
                block.terminal_pc,
                &env,
                &mut exceptional,
                budget,
            )?;
        }
        let terminal = if let Some(register) = terminal_register(block.terminal) {
            Some(value(
                lookup(&env, register, budget)?,
                register,
                block.terminal_pc,
            )?)
        } else {
            None
        };
        let other = if let Some(register) = terminal_other(block.terminal) {
            Some(value(
                lookup(&env, register, budget)?,
                register,
                block.terminal_pc,
            )?)
        } else {
            None
        };
        push(
            &mut bound,
            BoundBlock {
                ops,
                terminal,
                other,
            },
            budget,
        )?;
        let mut snapshot = Vec::new();
        for edge in &flow.edges {
            budget.checkpoint()?;
            if edge.source == block_id {
                for &register in &live[ix(edge.target.0)?] {
                    budget.checkpoint()?;
                    write(
                        &mut snapshot,
                        register,
                        lookup(&env, register, budget)?,
                        budget,
                    )?;
                    let high = super::word_ownership::high(register)?;
                    if high.0 < u32::from(flow.raw.registers_size()) {
                        write(&mut snapshot, high, lookup(&env, high, budget)?, budget)?;
                    }
                }
            }
        }
        push(&mut outgoing, snapshot, budget)?;
    }
    for phi in &mut phis {
        budget.checkpoint()?;
        if phi.block == BlockId(0) {
            push(
                &mut phi.inputs,
                (Incoming::EntrySeed, lookup(&seeds, phi.register, budget)?),
                budget,
            )?;
        }
        for (index, edge) in flow.edges.iter().enumerate() {
            budget.checkpoint()?;
            if edge.target == phi.block {
                push(
                    &mut phi.inputs,
                    (
                        Incoming::Normal(EdgeId(
                            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                        )),
                        lookup(&outgoing[ix(edge.source.0)?], phi.register, budget)?,
                    ),
                    budget,
                )?;
            }
        }
    }
    for phi in &mut phis {
        for (index, edge) in flow.exception_edges.iter().enumerate() {
            budget.checkpoint()?;
            if edge.target == phi.block {
                push(
                    &mut phi.inputs,
                    (
                        Incoming::Exceptional(crate::p2_flow::protection::ExceptionEdgeId(
                            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                        )),
                        lookup(&exceptional[index], phi.register, budget)?,
                    ),
                    budget,
                )?;
            }
        }
    }
    let mut connected_calls = Vec::new();
    for operation in call_ops {
        budget.checkpoint()?;
        push(
            &mut connected_calls,
            operation.ok_or(RequestError::InternalInvariant("unconnected call"))?,
            budget,
        )?;
    }
    let flow = flow.into_data();
    let ssa = SsaBody(SsaBodyData {
        _sealed: (),
        switches: flow.switches,
        calls: flow.calls,
        call_ops: connected_calls,
        operations,
        result: flow.result,
        raw: flow.raw,
        decoded: flow.decoded,
        object_creations: flow.object_creations,
        array_creations: flow.array_creations,
        filled_arrays: flow.filled_arrays,
        fresh_fills: flow.fresh_fills,
        empty_fills: flow.empty_fills,
        casts: flow.casts,
        strings: flow.strings,
        parameters: flow.parameters,
        frame: flow.frame,
        method_monitor: flow.method_monitor,
        blocks: flow.blocks,
        edges: flow.edges,
        exception_edges: flow.exception_edges,
        protection: flow.protection,
        values,
        phis,
        bound,
    });
    let widths = super::word_ownership::analyze(&ssa, &seeds, &outgoing, &exceptional, budget)?;
    verify(&ssa, &seeds, &widths, budget)?;
    Ok(ssa)
}
fn verify(
    ssa: &SsaBody<'_>,
    seeds: &[(RegisterId, Sym)],
    widths: &super::word_ownership::WidthFacts,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    crate::p2_flow::protection::verify(
        &ssa.raw,
        &ssa.protection,
        &ssa.blocks,
        &ssa.edges,
        &ssa.exception_edges,
        budget,
    )?;
    let mut operation_count = 0usize;
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.checkpoint()?;
        if block.pc.0 > block.terminal_pc.0 || block.terminal_pc.0 >= ssa.raw.code_units().0 {
            return Err(RequestError::InternalInvariant("invalid SSA block extent"));
        }
        let block_id = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut env = Vec::new();
        for phi in &ssa.phis {
            budget.checkpoint()?;
            if phi.block != block_id {
                continue;
            }
            let mut expected = usize::from(index == 0);
            for edge in &ssa.edges {
                budget.checkpoint()?;
                if edge.target == block_id {
                    expected = expected
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                }
            }
            for edge in &ssa.exception_edges {
                budget.checkpoint()?;
                if edge.target == block_id {
                    expected = expected
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                }
            }
            if phi.inputs.len() != expected {
                return Err(RequestError::InternalInvariant("incomplete Phi input set"));
            }
            write(&mut env, phi.register, Sym::Value(phi.result), budget)?;
            let mut seed_count = 0usize;
            for &(edge, sym) in &phi.inputs {
                budget.checkpoint()?;
                if edge == Incoming::EntrySeed {
                    seed_count = seed_count
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                    if index != 0 || sym != lookup(seeds, phi.register, budget)? {
                        return Err(RequestError::InternalInvariant("foreign EntrySeed"));
                    }
                }
            }
            if seed_count != usize::from(index == 0) {
                return Err(RequestError::InternalInvariant("entry Phi lost EntrySeed"));
            }
        }
        phi_highs(
            &ssa.phis,
            block_id,
            ssa.raw.registers_size(),
            &mut env,
            budget,
        )?;
        if ssa.bound[index].ops.len() != block.ops.len() {
            return Err(RequestError::InternalInvariant(
                "SSA changed operation coverage",
            ));
        }
        for (original, &id) in block.ops.iter().zip(&ssa.bound[index].ops) {
            budget.checkpoint()?;
            let op = ssa.operation(id)?;
            if original.id != id
                || op.id != id
                || ix(id.0)? != operation_count
                || original.pc != op.pc
                || original.kind != op.op
            {
                return Err(RequestError::InternalInvariant(
                    "SSA changed operation identity or site",
                ));
            }
            operation_count = operation_count
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            let mut inputs = op.inputs.iter();
            for (position, register) in original
                .kind
                .reads(&ssa.calls, &ssa.filled_arrays)?
                .enumerate()
            {
                budget.checkpoint()?;
                if inputs.next().copied()
                    != Some(widths.read(
                        &env,
                        register,
                        original.kind.read_width(position, &ssa.calls)?,
                        op.pc,
                        budget,
                    )?)
                {
                    return Err(RequestError::InternalInvariant(
                        "SSA input is not the latest definition",
                    ));
                }
            }
            if inputs.next().is_some() {
                return Err(RequestError::InternalInvariant(
                    "SSA added operation inputs",
                ));
            }
            if let Op::Call(call, destination) = original.kind {
                if ssa.call(call)?.id != id {
                    return Err(RequestError::InternalInvariant(
                        "call changed operation identity",
                    ));
                }
                verify_call(ssa, op.pc, call, destination, budget)?;
            } else if let Op::Initialize(call) = original.kind {
                if ssa.call(call)?.id != id {
                    return Err(RequestError::InternalInvariant(
                        "init changed operation identity",
                    ));
                }
                verify_call(ssa, op.pc, call, None, budget)?;
            }
            verify_exceptional_inputs(ssa, block_id, op.pc, &env, budget)?;
            match (destination(original.kind), op.result) {
                (Some(register), Some(result)) => {
                    let expected = if matches!(original.kind, Op::Exception(_)) {
                        Definition::ExceptionAt(id)
                    } else {
                        Definition::OpResult(id)
                    };
                    if ssa.values.get(ix(result.0)?) != Some(&expected) {
                        return Err(RequestError::InternalInvariant(
                            "operation changed result definition",
                        ));
                    }
                    commit(
                        &mut env,
                        register,
                        result,
                        original.kind.result_width(&ssa.calls)?,
                        budget,
                    )?;
                }
                (None, None) => {}
                _ => {
                    return Err(RequestError::InternalInvariant(
                        "operation changed result availability",
                    ));
                }
            }
        }
        if matches!(block.terminal, crate::p2_flow::Terminal::Throw(_)) {
            verify_exceptional_inputs(ssa, block_id, block.terminal_pc, &env, budget)?;
        }
        for (register, actual) in [
            (terminal_register(block.terminal), ssa.bound[index].terminal),
            (terminal_other(block.terminal), ssa.bound[index].other),
        ] {
            budget.checkpoint()?;
            let expected = if let Some(register) = register {
                Some(widths.read(
                    &env,
                    register,
                    super::word_ownership::terminal_width(ssa, index),
                    block.terminal_pc,
                    budget,
                )?)
            } else {
                None
            };
            if actual != expected {
                return Err(RequestError::InternalInvariant(
                    "terminal changed its bound use",
                ));
            }
        }
        for (edge_index, edge) in ssa.edges.iter().enumerate() {
            budget.checkpoint()?;
            if edge.source != block_id {
                continue;
            }
            let id = Incoming::Normal(EdgeId(
                u32::try_from(edge_index).map_err(|_| RequestError::PlatformCapacity)?,
            ));
            for phi in &ssa.phis {
                budget.checkpoint()?;
                if phi.block != edge.target {
                    continue;
                }
                let mut supplied = None;
                for &(incoming, sym) in &phi.inputs {
                    budget.checkpoint()?;
                    if incoming == id && supplied.replace(sym).is_some() {
                        return Err(RequestError::InternalInvariant("duplicate Phi edge"));
                    }
                }
                if supplied != Some(lookup(&env, phi.register, budget)?) {
                    return Err(RequestError::InternalInvariant(
                        "Phi edge is not the post-commit environment",
                    ));
                }
            }
        }
    }
    if operation_count != ssa.operations.len() {
        return Err(RequestError::InternalInvariant(
            "SSA retained foreign operations",
        ));
    }
    verify_availability(ssa, budget)
}
pub(super) fn verify_exceptional_inputs(
    ssa: &SsaBody<'_>,
    source: BlockId,
    pc: CodeUnitOffset,
    env: &[(RegisterId, Sym)],
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for (index, edge) in ssa.exception_edges.iter().enumerate() {
        budget.checkpoint()?;
        if edge.source != source || edge.pc != pc {
            continue;
        }
        let incoming = Incoming::Exceptional(crate::p2_flow::protection::ExceptionEdgeId(
            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
        ));
        for phi in &ssa.phis {
            budget.checkpoint()?;
            if phi.block != edge.target {
                continue;
            }
            let mut supplied = None;
            for &(actual, value) in &phi.inputs {
                budget.checkpoint()?;
                if actual == incoming && supplied.replace(value).is_some() {
                    return Err(RequestError::InternalInvariant(
                        "duplicate exceptional Phi edge",
                    ));
                }
            }
            if supplied != Some(lookup(env, phi.register, budget)?) {
                return Err(RequestError::InternalInvariant(
                    "exception Phi is not the pre-write environment",
                ));
            }
        }
    }
    Ok(())
}
pub(super) fn verify_call_words(
    site: &CallSite,
    original_operands: crate::raw::RawOperands,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let mut original_registers = original_operands.registers().iter();
    for &register in &site.registers {
        budget.checkpoint()?;
        if original_registers.next() != Some(register) {
            return Err(RequestError::InternalInvariant(
                "call changed original argument word order",
            ));
        }
    }
    if original_registers.next().is_some() {
        return Err(RequestError::InternalInvariant(
            "call dropped original argument words",
        ));
    }
    let mut word = 0usize;
    if site.widths.len() != site.arguments.len() {
        return Err(RequestError::InternalInvariant(
            "call changed logical arity",
        ));
    }
    for (&argument, width) in site.arguments.iter().zip(&site.widths) {
        budget.checkpoint()?;
        if site.registers.get(word) != Some(&argument)
            || *width == crate::p2_flow::ValueWidth::Two
                && word
                    .checked_add(1)
                    .and_then(|index| site.registers.get(index))
                    .map(|register| register.0)
                    != argument.0.checked_add(1)
        {
            return Err(RequestError::InternalInvariant(
                "call changed logical argument words",
            ));
        }
        word = word
            .checked_add(if *width == crate::p2_flow::ValueWidth::Two {
                2
            } else {
                1
            })
            .ok_or(RequestError::PlatformCapacity)?;
    }
    if word != site.registers.len() {
        return Err(RequestError::InternalInvariant(
            "call dropped wide argument words",
        ));
    }
    Ok(())
}
fn verify_call(
    ssa: &SsaBody<'_>,
    pc: CodeUnitOffset,
    call: CallId,
    destination: Option<RegisterId>,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let site = ssa
        .calls
        .get(ix(call.0)?)
        .ok_or(RequestError::InternalInvariant("call has foreign identity"))?;
    let original = ssa.decoded.layout_at(pc)?;
    if !matches!(original.kind(),LayoutKind::Instruction(spec) if crate::p2_flow::InvokeKind::from_opcode(spec.opcode) == Some(site.kind))
        || !matches!(original.references()[0],Some(reference) if reference.target==crate::raw::OperandReference::Method(site.reference))
    {
        return Err(RequestError::InternalInvariant(
            "call changed its original invoke or method reference",
        ));
    }
    let original_operands = ssa.decoded.operands_at(pc)?;
    verify_call_words(site, original_operands, budget)?;
    if let Some(result_pc) = site.result_pc {
        if pc.checked_advance(original.width()) != Some(result_pc) {
            return Err(RequestError::InternalInvariant(
                "call result is not adjacent",
            ));
        }
        let result = ssa.decoded.layout_at(result_pc)?;
        let operands = ssa.decoded.operands_at(result_pc)?;
        let expected = match site.result {
            crate::p2_flow::InvokeResult::Word(_) => Some(0x0a),
            crate::p2_flow::InvokeResult::Reference(_) => Some(0x0c),
            crate::p2_flow::InvokeResult::Wide(_) => Some(0x0b),
            crate::p2_flow::InvokeResult::Void => None,
        };
        if !matches!(result.kind(),LayoutKind::Instruction(spec) if Some(spec.opcode)==expected)
            || operands.registers().iter().next() != destination
        {
            return Err(RequestError::InternalInvariant(
                "call changed move-result kind or target",
            ));
        }
    }
    if site.pc != pc || destination.is_some() != site.result_pc.is_some() {
        return Err(RequestError::InternalInvariant(
            "call changed original/result sites",
        ));
    }
    Ok(())
}
fn verify_availability(ssa: &SsaBody<'_>, budget: &mut RunContext<'_>) -> Result<(), RequestError> {
    let mut bad = Vec::new();
    let mut conservative = Vec::new();
    let mut grounded = Vec::new();
    for definition in &ssa.values {
        budget.checkpoint()?;
        push(&mut bad, false, budget)?;
        push(&mut conservative, false, budget)?;
        push(
            &mut grounded,
            !matches!(definition, Definition::Phi),
            budget,
        )?;
    }
    loop {
        let mut changed = false;
        for op in &ssa.operations {
            budget.checkpoint()?;
            if let Some(result) = op.result {
                let index = ix(result.0)?;
                for source in &op.inputs {
                    budget.checkpoint()?;
                    if bad[ix(source.0)?] && !bad[index] {
                        bad[index] = true;
                        changed = true;
                    }
                    if conservative[ix(source.0)?] && !conservative[index] {
                        conservative[index] = true;
                        changed = true;
                    }
                }
            }
        }
        for phi in &ssa.phis {
            budget.checkpoint()?;
            let index = ix(phi.result.0)?;
            for &(incoming, input) in &phi.inputs {
                budget.checkpoint()?;
                let (input_bad, input_conservative, input_grounded) = match input {
                    Sym::Bad | Sym::High(_) => (true, false, false),
                    Sym::Value(source) => (
                        bad[ix(source.0)?],
                        conservative[ix(source.0)?],
                        grounded[ix(source.0)?],
                    ),
                };
                let exceptional = matches!(incoming, Incoming::Exceptional(_));
                if (input_conservative || input_bad && exceptional) && !conservative[index] {
                    conservative[index] = true;
                    changed = true;
                }
                if input_bad && !exceptional && !bad[index] {
                    bad[index] = true;
                    changed = true;
                }
                if input_grounded && !grounded[index] {
                    grounded[index] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let available = |value: ValueId, register: RegisterId, pc: CodeUnitOffset| {
        let index = ix(value.0)?;
        if bad[index] || !grounded[index] {
            Err(RequestError::UndefinedRegister { register, pc })
        } else if conservative[index] {
            Err(RequestError::DefinitionAvailabilityUnproved { register, pc })
        } else {
            Ok(())
        }
    };
    for op in &ssa.operations {
        budget.checkpoint()?;
        for (register, &source) in op.op.reads(&ssa.calls, &ssa.filled_arrays)?.zip(&op.inputs) {
            budget.checkpoint()?;
            available(source, register, op.pc)?;
        }
    }
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.checkpoint()?;
        for (register, source) in [
            (terminal_register(block.terminal), ssa.bound[index].terminal),
            (terminal_other(block.terminal), ssa.bound[index].other),
        ] {
            budget.checkpoint()?;
            if let (Some(register), Some(source)) = (register, source) {
                available(source, register, block.terminal_pc)?;
            }
        }
    }
    Ok(())
}
