use crate::input::RawMethodView;
use crate::p2_flow::normal::{
    Block, BlockId, CallId, CallSite, DeclarationType, Edge, EdgeId, FlowBody, Op, OpId,
    ResultType, Switch, destination, terminal_other, terminal_register,
};
use crate::raw::{LayoutKind, instruction_layout};
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValueId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Sym {
    Value(ValueId),
    Bad,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Definition {
    Parameter(u32),
    OpResult(OpId),
    Phi,
}
pub(crate) struct Phi {
    pub block: BlockId,
    pub register: RegisterId,
    pub result: ValueId,
    pub inputs: Vec<(Option<EdgeId>, Sym)>,
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
pub(crate) struct SsaBody<'a> {
    pub frame: crate::p2_flow::normal::MethodFrame,
    _sealed: (),
    pub switches: Vec<Switch>,
    pub calls: Vec<CallSite>,
    call_ops: Vec<OpId>,
    pub operations: Vec<BoundOp>,
    pub result: ResultType,
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<DeclarationType>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
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
            Definition::Parameter(_) | Definition::Phi => Ok(false),
        }
    }
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn add_register(
    set: &mut Vec<RegisterId>,
    register: RegisterId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for old in set.iter() {
        budget.charge_work(1)?;
        if *old == register {
            return Ok(false);
        }
    }
    push(set, register, budget)?;
    Ok(true)
}
fn lookup(
    env: &[(RegisterId, Sym)],
    register: RegisterId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Sym, RequestError> {
    for &(slot, value) in env {
        budget.charge_work(1)?;
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
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    for entry in env.iter_mut() {
        budget.charge_work(1)?;
        if entry.0 == register {
            entry.1 = value;
            return Ok(());
        }
    }
    push(env, (register, value), budget)
}
fn value(sym: Sym, register: RegisterId, pc: CodeUnitOffset) -> Result<ValueId, RequestError> {
    match sym {
        Sym::Value(value) => Ok(value),
        Sym::Bad => Err(RequestError::UndefinedRegister { register, pc }),
    }
}
pub(crate) fn build<'a>(
    flow: FlowBody<'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<SsaBody<'a>, RequestError> {
    let mut live = Vec::new();
    for _ in &flow.blocks {
        budget.charge_work(1)?;
        push(&mut live, Vec::new(), budget)?;
    }
    loop {
        let mut changed = false;
        for index in (0..flow.blocks.len()).rev() {
            budget.charge_work(1)?;
            let block = &flow.blocks[index];
            let mut needed = Vec::new();
            for edge in &flow.edges {
                budget.charge_work(1)?;
                if ix(edge.source.0)? == index {
                    for &register in &live[ix(edge.target.0)?] {
                        budget.charge_work(1)?;
                        add_register(&mut needed, register, budget)?;
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
                budget.charge_work(1)?;
                add_register(&mut needed, register, budget)?;
            }
            for operation in block.ops.iter().rev() {
                budget.charge_work(1)?;
                if let Some(destination) = destination(operation.kind) {
                    let mut position = 0;
                    while position < needed.len() {
                        budget.charge_work(1)?;
                        if needed[position] == destination {
                            needed.swap_remove(position);
                        } else {
                            position += 1;
                        }
                    }
                }
                for register in operation.kind.reads(&flow.calls)? {
                    budget.charge_work(1)?;
                    add_register(&mut needed, register, budget)?;
                }
            }
            for register in needed {
                budget.charge_work(1)?;
                changed |= add_register(&mut live[index], register, budget)?;
            }
        }
        if !changed {
            break;
        }
    }
    let mut values = Vec::new();
    let mut seeds = Vec::new();
    let first = u32::from(flow.raw.registers_size())
        .checked_sub(
            u32::try_from(flow.parameters.len()).map_err(|_| RequestError::PlatformCapacity)?,
        )
        .ok_or(RequestError::InternalInvariant("parameter frame underflow"))?;
    for index in 0..flow.parameters.len() {
        budget.charge_work(1)?;
        let number = u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?;
        let id = ValueId(u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?);
        push(&mut values, Definition::Parameter(number), budget)?;
        let register = RegisterId(
            first
                .checked_add(number)
                .ok_or(RequestError::PlatformCapacity)?,
        );
        push(&mut seeds, (register, Sym::Value(id)), budget)?;
    }
    let mut phis = Vec::new();
    for (index, registers) in live.iter().enumerate() {
        for &register in registers {
            budget.charge_work(1)?;
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
        budget.charge_work(1)?;
        push(&mut call_ops, None, budget)?;
    }
    let mut outgoing = Vec::new();
    for (index, block) in flow.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        let block_id = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut env = Vec::new();
        for phi in &phis {
            budget.charge_work(1)?;
            if phi.block == block_id {
                write(&mut env, phi.register, Sym::Value(phi.result), budget)?;
            }
        }
        let mut ops = Vec::new();
        for operation in &block.ops {
            budget.charge_work(1)?;
            if ix(operation.id.0)? != operations.len() {
                return Err(RequestError::InternalInvariant(
                    "flow operation identity changed",
                ));
            }
            let mut inputs = Vec::new();
            for register in operation.kind.reads(&flow.calls)? {
                budget.charge_work(1)?;
                push(
                    &mut inputs,
                    value(lookup(&env, register, budget)?, register, operation.pc)?,
                    budget,
                )?;
            }
            let result = if let Some(register) = destination(operation.kind) {
                let id = ValueId(
                    u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?,
                );
                push(&mut values, Definition::OpResult(operation.id), budget)?;
                write(&mut env, register, Sym::Value(id), budget)?;
                Some(id)
            } else {
                None
            };
            if let Op::Call(call, _) = operation.kind {
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
            budget.charge_work(1)?;
            if edge.source == block_id {
                for &register in &live[ix(edge.target.0)?] {
                    budget.charge_work(1)?;
                    let mut present = false;
                    for &(slot, _) in &snapshot {
                        budget.charge_work(1)?;
                        if slot == register {
                            present = true;
                            break;
                        }
                    }
                    if !present {
                        push(
                            &mut snapshot,
                            (register, lookup(&env, register, budget)?),
                            budget,
                        )?;
                    }
                }
            }
        }
        push(&mut outgoing, snapshot, budget)?;
    }
    for phi in &mut phis {
        budget.charge_work(1)?;
        if phi.block == BlockId(0) {
            push(
                &mut phi.inputs,
                (None, lookup(&seeds, phi.register, budget)?),
                budget,
            )?;
        }
        for (index, edge) in flow.edges.iter().enumerate() {
            budget.charge_work(1)?;
            if edge.target == phi.block {
                push(
                    &mut phi.inputs,
                    (
                        Some(EdgeId(
                            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                        )),
                        lookup(&outgoing[ix(edge.source.0)?], phi.register, budget)?,
                    ),
                    budget,
                )?;
            }
        }
    }
    let mut connected_calls = Vec::new();
    for operation in call_ops {
        budget.charge_work(1)?;
        push(
            &mut connected_calls,
            operation.ok_or(RequestError::InternalInvariant("unconnected call"))?,
            budget,
        )?;
    }
    let ssa = SsaBody {
        _sealed: (),
        switches: flow.switches,
        calls: flow.calls,
        call_ops: connected_calls,
        operations,
        result: flow.result,
        raw: flow.raw,
        parameters: flow.parameters,
        frame: flow.frame,
        blocks: flow.blocks,
        edges: flow.edges,
        values,
        phis,
        bound,
    };
    verify(&ssa, &seeds, budget)?;
    Ok(ssa)
}
fn verify(
    ssa: &SsaBody<'_>,
    seeds: &[(RegisterId, Sym)],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let mut operation_count = 0usize;
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        if block.pc.0 > block.terminal_pc.0 || block.terminal_pc.0 >= ssa.raw.code_units().0 {
            return Err(RequestError::InternalInvariant("invalid SSA block extent"));
        }
        let block_id = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut env = Vec::new();
        for phi in &ssa.phis {
            budget.charge_work(1)?;
            if phi.block != block_id {
                continue;
            }
            let mut expected = usize::from(index == 0);
            for edge in &ssa.edges {
                budget.charge_work(1)?;
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
                budget.charge_work(1)?;
                if edge.is_none() {
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
        if ssa.bound[index].ops.len() != block.ops.len() {
            return Err(RequestError::InternalInvariant(
                "SSA changed operation coverage",
            ));
        }
        for (original, &id) in block.ops.iter().zip(&ssa.bound[index].ops) {
            budget.charge_work(1)?;
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
            for register in original.kind.reads(&ssa.calls)? {
                budget.charge_work(1)?;
                if inputs.next().copied()
                    != Some(value(lookup(&env, register, budget)?, register, op.pc)?)
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
            }
            match (destination(original.kind), op.result) {
                (Some(register), Some(result)) => {
                    if ssa.values.get(ix(result.0)?) != Some(&Definition::OpResult(id)) {
                        return Err(RequestError::InternalInvariant(
                            "operation changed result definition",
                        ));
                    }
                    write(&mut env, register, Sym::Value(result), budget)?;
                }
                (None, None) => {}
                _ => {
                    return Err(RequestError::InternalInvariant(
                        "operation changed result availability",
                    ));
                }
            }
        }
        for (register, actual) in [
            (terminal_register(block.terminal), ssa.bound[index].terminal),
            (terminal_other(block.terminal), ssa.bound[index].other),
        ] {
            budget.charge_work(1)?;
            let expected = if let Some(register) = register {
                Some(value(
                    lookup(&env, register, budget)?,
                    register,
                    block.terminal_pc,
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
            budget.charge_work(1)?;
            if edge.source != block_id {
                continue;
            }
            let id = Some(EdgeId(
                u32::try_from(edge_index).map_err(|_| RequestError::PlatformCapacity)?,
            ));
            for phi in &ssa.phis {
                budget.charge_work(1)?;
                if phi.block != edge.target {
                    continue;
                }
                let mut supplied = None;
                for &(incoming, sym) in &phi.inputs {
                    budget.charge_work(1)?;
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
fn verify_call(
    ssa: &SsaBody<'_>,
    pc: CodeUnitOffset,
    call: CallId,
    destination: Option<RegisterId>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let site = ssa
        .calls
        .get(ix(call.0)?)
        .ok_or(RequestError::InternalInvariant("call has foreign identity"))?;
    let original = instruction_layout(ssa.raw.instructions(), pc, ssa.raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(original.kind(),LayoutKind::Instruction(spec) if crate::p2_flow::normal::InvokeKind::from_opcode(spec.opcode) == Some(site.kind))
        || !matches!(original.references()[0],Some(reference) if reference.target==crate::raw::OperandReference::Method(site.reference))
    {
        return Err(RequestError::InternalInvariant(
            "call changed its original invoke or method reference",
        ));
    }
    let original_operands =
        crate::input::operands::instruction_operands(ssa.raw.instructions(), pc, original)
            .map_err(crate::input::MethodError::Decode)?;
    let mut original_registers = original_operands.registers().iter();
    for &register in &site.registers {
        budget.charge_work(1)?;
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
    if let Some(result_pc) = site.result_pc {
        if pc.checked_advance(original.width()) != Some(result_pc) {
            return Err(RequestError::InternalInvariant(
                "call result is not adjacent",
            ));
        }
        let result = instruction_layout(ssa.raw.instructions(), result_pc, ssa.raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let operands =
            crate::input::operands::instruction_operands(ssa.raw.instructions(), result_pc, result)
                .map_err(crate::input::MethodError::Decode)?;
        let expected = match site.result {
            crate::p2_flow::normal::InvokeResult::Word(_) => Some(0x0a),
            crate::p2_flow::normal::InvokeResult::Reference(_) => Some(0x0c),
            crate::p2_flow::normal::InvokeResult::Wide => Some(0x0b),
            crate::p2_flow::normal::InvokeResult::Void => None,
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
fn verify_availability(
    ssa: &SsaBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let mut bad = Vec::new();
    let mut grounded = Vec::new();
    for definition in &ssa.values {
        budget.charge_work(1)?;
        push(&mut bad, false, budget)?;
        push(
            &mut grounded,
            !matches!(definition, Definition::Phi),
            budget,
        )?;
    }
    loop {
        let mut changed = false;
        for op in &ssa.operations {
            budget.charge_work(1)?;
            if let Some(result) = op.result {
                let index = ix(result.0)?;
                for source in &op.inputs {
                    budget.charge_work(1)?;
                    if bad[ix(source.0)?] && !bad[index] {
                        bad[index] = true;
                        changed = true;
                    }
                }
            }
        }
        for phi in &ssa.phis {
            budget.charge_work(1)?;
            let index = ix(phi.result.0)?;
            for &(_, input) in &phi.inputs {
                budget.charge_work(1)?;
                let (input_bad, input_grounded) = match input {
                    Sym::Bad => (true, false),
                    Sym::Value(source) => (bad[ix(source.0)?], grounded[ix(source.0)?]),
                };
                if input_bad && !bad[index] {
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
        } else {
            Ok(())
        }
    };
    for op in &ssa.operations {
        budget.charge_work(1)?;
        for (register, &source) in op.op.reads(&ssa.calls)?.zip(&op.inputs) {
            budget.charge_work(1)?;
            available(source, register, op.pc)?;
        }
    }
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        for (register, source) in [
            (terminal_register(block.terminal), ssa.bound[index].terminal),
            (terminal_other(block.terminal), ssa.bound[index].other),
        ] {
            budget.charge_work(1)?;
            if let (Some(register), Some(source)) = (register, source) {
                available(source, register, block.terminal_pc)?;
            }
        }
    }
    Ok(())
}
