use crate::input::RawMethodView;
use crate::p2_flow::normal::{
    Arithmetic, Block, BlockId, CallId, CallSite, DeclarationType, Edge, EdgeId, FlowBody, Op,
    Operand, ResultType, Switch, destination, other_register, terminal_other, terminal_register,
    use_register,
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
pub(crate) enum BoundOperand {
    Value(ValueId),
    Literal(i32),
}
pub(crate) struct BoundCall {
    pub arguments: Vec<ValueId>,
}
#[derive(Clone, Copy)]
pub(crate) enum Definition {
    CallResult(CallId),
    Compare(ValueId, crate::pipeline::scalar::Compare, ValueId),
    Parameter(u32),
    Constant(i32),
    Move(ValueId),
    Binary(ValueId, Arithmetic, BoundOperand),
    Phi,
}
pub(crate) struct Phi {
    pub block: BlockId,
    pub register: RegisterId,
    pub result: ValueId,
    pub inputs: Vec<(Option<EdgeId>, Sym)>,
}
pub(crate) struct BoundOp {
    pub pc: CodeUnitOffset,
    pub op: Op,
    pub result: Option<ValueId>,
    pub source: Option<ValueId>,
    pub other: Option<BoundOperand>,
}
pub(crate) struct BoundBlock {
    pub ops: Vec<BoundOp>,
    pub terminal: Option<ValueId>,
    pub other: Option<ValueId>,
}
pub(crate) struct SsaBody<'a> {
    _sealed: (),
    pub switches: Vec<Switch>,
    pub calls: Vec<CallSite>,
    pub bound_calls: Vec<Option<BoundCall>>,
    pub result: ResultType,
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<DeclarationType>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub values: Vec<Definition>,
    pub phis: Vec<Phi>,
    pub bound: Vec<BoundBlock>,
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
                        add_register(&mut needed, register, budget)?;
                    }
                }
            }
            if let Some(register) = terminal_register(block.terminal) {
                add_register(&mut needed, register, budget)?;
            }
            if let Some(register) = terminal_other(block.terminal) {
                add_register(&mut needed, register, budget)?;
            }
            for &(_, op) in block.ops.iter().rev() {
                budget.charge_work(1)?;
                if let Some(destination) = destination(op) {
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
                if let Op::Call(call, _) = op {
                    for &register in &flow.calls[ix(call.0)?].registers {
                        budget.charge_work(1)?;
                        add_register(&mut needed, register, budget)?;
                    }
                }
                if let Some(register) = use_register(op) {
                    add_register(&mut needed, register, budget)?;
                }
                if let Some(register) = other_register(op) {
                    add_register(&mut needed, register, budget)?;
                }
            }
            for register in needed {
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
        .ok_or(RequestError::InternalInvariant(
            "branch parameter frame underflow",
        ))?;
    for index in 0..flow.parameters.len() {
        let number = u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?;
        let id = ValueId(u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?);
        push(&mut values, Definition::Parameter(number), budget)?;
        push(
            &mut seeds,
            (
                RegisterId(
                    first
                        .checked_add(number)
                        .ok_or(RequestError::PlatformCapacity)?,
                ),
                Sym::Value(id),
            ),
            budget,
        )?;
    }
    let mut phis = Vec::new();
    for (index, registers) in live.iter().enumerate() {
        for &register in registers {
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
    let mut bound_calls = Vec::new();
    for _ in &flow.calls {
        push(&mut bound_calls, None, budget)?;
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
        for &(pc, op) in &block.ops {
            budget.charge_work(1)?;
            let source = if let Some(register) = use_register(op) {
                Some(value(lookup(&env, register, budget)?, register, pc)?)
            } else {
                None
            };
            let other = match op {
                Op::Compare(_, _, _, register)
                | Op::Binary(_, _, _, Operand::Register(register)) => Some(BoundOperand::Value(
                    value(lookup(&env, register, budget)?, register, pc)?,
                )),
                Op::Binary(_, _, _, Operand::Literal(literal)) => {
                    Some(BoundOperand::Literal(literal))
                }
                _ => None,
            };
            let result = match op {
                Op::Call(call, destination) => {
                    let site = &flow.calls[ix(call.0)?];
                    let mut arguments = Vec::new();
                    for &register in &site.registers {
                        budget.charge_work(1)?;
                        push(
                            &mut arguments,
                            value(lookup(&env, register, budget)?, register, pc)?,
                            budget,
                        )?;
                    }
                    if bound_calls[ix(call.0)?].is_some() {
                        return Err(RequestError::InternalInvariant("call was bound twice"));
                    }
                    bound_calls[ix(call.0)?] = Some(BoundCall { arguments });
                    if destination.is_some() {
                        let result = ValueId(
                            u32::try_from(values.len())
                                .map_err(|_| RequestError::PlatformCapacity)?,
                        );
                        push(&mut values, Definition::CallResult(call), budget)?;
                        Some(result)
                    } else {
                        None
                    }
                }
                Op::Compare(_, _, compare, _) => {
                    let id = ValueId(
                        u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    let Some(BoundOperand::Value(right)) = other else {
                        return Err(RequestError::InternalInvariant("cmp lost its right value"));
                    };
                    push(
                        &mut values,
                        Definition::Compare(
                            source.ok_or(RequestError::InternalInvariant(
                                "cmp lost its left value",
                            ))?,
                            compare,
                            right,
                        ),
                        budget,
                    )?;
                    Some(id)
                }
                Op::Binary(_, _, arithmetic, _) => {
                    let id = ValueId(
                        u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(
                        &mut values,
                        Definition::Binary(
                            source.ok_or(RequestError::InternalInvariant(
                                "arithmetic left use absent",
                            ))?,
                            arithmetic,
                            other.ok_or(RequestError::InternalInvariant(
                                "arithmetic right use absent",
                            ))?,
                        ),
                        budget,
                    )?;
                    Some(id)
                }
                Op::Nop => None,
                Op::Constant(_, constant) => {
                    let id = ValueId(
                        u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(&mut values, Definition::Constant(constant), budget)?;
                    Some(id)
                }
                Op::Move(_, _) => {
                    let id = ValueId(
                        u32::try_from(values.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(
                        &mut values,
                        Definition::Move(
                            source
                                .ok_or(RequestError::InternalInvariant("move lost its source"))?,
                        ),
                        budget,
                    )?;
                    Some(id)
                }
            };
            if let Some(register) = destination(op) {
                write(
                    &mut env,
                    register,
                    Sym::Value(result.ok_or(RequestError::InternalInvariant(
                        "write lost its fixed definition",
                    ))?),
                    budget,
                )?;
            }
            push(
                &mut ops,
                BoundOp {
                    pc,
                    op,
                    result,
                    source,
                    other,
                },
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
            budget.charge_work(1)?;
            if edge.source == block_id {
                for &register in &live[ix(edge.target.0)?] {
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
    let ssa = SsaBody {
        _sealed: (),
        switches: flow.switches,
        calls: flow.calls,
        bound_calls,
        result: flow.result,
        raw: flow.raw,
        parameters: flow.parameters,
        blocks: flow.blocks,
        edges: flow.edges,
        values,
        phis,
        bound,
    };
    verify(ssa, &seeds, budget)
}

fn verify<'a>(
    ssa: SsaBody<'a>,
    seeds: &[(RegisterId, Sym)],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<SsaBody<'a>, RequestError> {
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        if block.pc.0 > block.terminal_pc.0 || block.terminal_pc.0 >= ssa.raw.code_units().0 {
            return Err(RequestError::InternalInvariant(
                "SSA block has an invalid original extent",
            ));
        }
        let block_id = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut env = Vec::new();
        for phi in &ssa.phis {
            if phi.block == block_id {
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
                    return Err(RequestError::InternalInvariant(
                        "Phi input set is incomplete",
                    ));
                }
                write(&mut env, phi.register, Sym::Value(phi.result), budget)?;
                let mut seed_count = 0usize;
                for (edge, _) in &phi.inputs {
                    budget.charge_work(1)?;
                    if edge.is_none() {
                        seed_count = seed_count
                            .checked_add(1)
                            .ok_or(RequestError::PlatformCapacity)?;
                    }
                }
                if index == 0 && seed_count != 1 {
                    return Err(RequestError::InternalInvariant("entry Phi lost EntrySeed"));
                }
                for &(edge, sym) in &phi.inputs {
                    budget.charge_work(1)?;
                    if edge.is_none() && (index != 0 || sym != lookup(seeds, phi.register, budget)?)
                    {
                        return Err(RequestError::InternalInvariant(
                            "entry Phi uses a non-parameter seed",
                        ));
                    }
                }
            }
        }
        if ssa.bound[index].ops.len() != block.ops.len() {
            return Err(RequestError::InternalInvariant(
                "SSA changed the original operation count",
            ));
        }
        for ((pc, op), bound) in block.ops.iter().zip(&ssa.bound[index].ops) {
            budget.charge_work(1)?;
            if *pc != bound.pc {
                return Err(RequestError::InternalInvariant(
                    "SSA operation has a foreign site",
                ));
            }
            if let Some(register) = use_register(*op)
                && bound.source != Some(value(lookup(&env, register, budget)?, register, *pc)?)
            {
                return Err(RequestError::InternalInvariant(
                    "SSA use is not the latest local definition",
                ));
            }
            let expected_other = match *op {
                Op::Compare(_, _, _, register)
                | Op::Binary(_, _, _, Operand::Register(register)) => Some(BoundOperand::Value(
                    value(lookup(&env, register, budget)?, register, *pc)?,
                )),
                Op::Binary(_, _, _, Operand::Literal(literal)) => {
                    Some(BoundOperand::Literal(literal))
                }
                _ => None,
            };
            if bound.other != expected_other {
                return Err(RequestError::InternalInvariant(
                    "SSA changed the arithmetic right use",
                ));
            }
            match *op {
                Op::Call(call, destination) => {
                    let site = ssa
                        .calls
                        .get(ix(call.0)?)
                        .ok_or(RequestError::InternalInvariant("call has foreign identity"))?;
                    let original =
                        instruction_layout(ssa.raw.instructions(), *pc, ssa.raw.version())
                            .map_err(crate::input::MethodError::Decode)?;
                    if !matches!(original.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,0x71|0x77))
                        || !matches!(original.references()[0],Some(reference) if reference.target==crate::raw::OperandReference::Method(site.contract.reference))
                    {
                        return Err(RequestError::InternalInvariant(
                            "call changed its original invoke or method reference",
                        ));
                    }
                    let original_operands = crate::input::operands::instruction_operands(
                        ssa.raw.instructions(),
                        *pc,
                        original,
                    )
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
                        let result = instruction_layout(
                            ssa.raw.instructions(),
                            result_pc,
                            ssa.raw.version(),
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let operands = crate::input::operands::instruction_operands(
                            ssa.raw.instructions(),
                            result_pc,
                            result,
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let expected = match site.contract.result {
                            crate::p4_calls::StaticResult::Int
                            | crate::p4_calls::StaticResult::Boolean => Some(0x0a),
                            crate::p4_calls::StaticResult::Reference(_)
                            | crate::p4_calls::StaticResult::Nominal => Some(0x0c),
                            crate::p4_calls::StaticResult::Void => None,
                        };
                        if !matches!(result.kind(),LayoutKind::Instruction(spec) if Some(spec.opcode)==expected)
                            || operands.registers().iter().next() != destination
                        {
                            return Err(RequestError::InternalInvariant(
                                "call changed move-result kind or target",
                            ));
                        }
                    }
                    let arguments = ssa
                        .bound_calls
                        .get(ix(call.0)?)
                        .and_then(Option::as_ref)
                        .ok_or(RequestError::InternalInvariant("call lost bound arguments"))?;
                    if site.pc != *pc
                        || site.registers.len() != arguments.arguments.len()
                        || destination.is_some() != bound.result.is_some()
                        || destination.is_some() != site.result_pc.is_some()
                    {
                        return Err(RequestError::InternalInvariant(
                            "call atom changed its original/result sites",
                        ));
                    }
                    for (&register, &argument) in site.registers.iter().zip(&arguments.arguments) {
                        budget.charge_work(1)?;
                        if value(lookup(&env, register, budget)?, register, *pc)? != argument {
                            return Err(RequestError::InternalInvariant(
                                "call changed argument words or order",
                            ));
                        }
                    }
                    if let Some(result) = bound.result
                        && !matches!(ssa.values.get(ix(result.0)?),Some(Definition::CallResult(original)) if original.0==call.0)
                    {
                        return Err(RequestError::InternalInvariant(
                            "call acquired a foreign result definition",
                        ));
                    }
                }
                Op::Compare(_, _, compare, _) => {
                    if !matches!(bound.result.and_then(|id|ssa.values.get(usize::try_from(id.0).ok()?)),Some(Definition::Compare(left,kind,right)) if Some(*left)==bound.source&&*kind==compare&&Some(BoundOperand::Value(*right))==bound.other)
                    {
                        return Err(RequestError::InternalInvariant(
                            "SSA cmp changed original operands or NaN bias",
                        ));
                    }
                }
                Op::Binary(_, _, arithmetic, _) => {
                    if !matches!(bound.result.and_then(|id|ssa.values.get(usize::try_from(id.0).ok()?)),Some(Definition::Binary(left,kind,right)) if Some(*left)==bound.source && *kind==arithmetic && Some(*right)==bound.other)
                    {
                        return Err(RequestError::InternalInvariant(
                            "SSA changed an arithmetic definition",
                        ));
                    }
                }
                Op::Constant(_, literal) => {
                    if !matches!(bound.result.and_then(|id|ssa.values.get(usize::try_from(id.0).ok()?)),Some(Definition::Constant(actual)) if *actual==literal)
                    {
                        return Err(RequestError::InternalInvariant(
                            "SSA changed the original constant",
                        ));
                    }
                }
                Op::Move(_, _) => {
                    if !matches!(bound.result.and_then(|id|ssa.values.get(usize::try_from(id.0).ok()?)),Some(Definition::Move(actual)) if Some(*actual)==bound.source)
                    {
                        return Err(RequestError::InternalInvariant(
                            "SSA move result uses a different definition",
                        ));
                    }
                }
                Op::Nop => {
                    if bound.result.is_some() || bound.source.is_some() {
                        return Err(RequestError::InternalInvariant(
                            "NOP acquired a definition or use",
                        ));
                    }
                }
            }
            if let Some(register) = destination(*op) {
                write(
                    &mut env,
                    register,
                    Sym::Value(bound.result.ok_or(RequestError::InternalInvariant(
                        "SSA write has no fixed result",
                    ))?),
                    budget,
                )?;
            }
        }
        if let Some(register) = terminal_register(block.terminal)
            && ssa.bound[index].terminal
                != Some(value(
                    lookup(&env, register, budget)?,
                    register,
                    block.terminal_pc,
                )?)
        {
            return Err(RequestError::InternalInvariant(
                "SSA terminal does not read the latest definition",
            ));
        }
        let expected_other = if let Some(register) = terminal_other(block.terminal) {
            Some(value(
                lookup(&env, register, budget)?,
                register,
                block.terminal_pc,
            )?)
        } else {
            None
        };
        if ssa.bound[index].other != expected_other {
            return Err(RequestError::InternalInvariant(
                "SSA changed the comparison right use",
            ));
        }
        for (edge_index, edge) in ssa.edges.iter().enumerate() {
            budget.charge_work(1)?;
            if edge.source == block_id {
                let id = Some(EdgeId(
                    u32::try_from(edge_index).map_err(|_| RequestError::PlatformCapacity)?,
                ));
                for phi in &ssa.phis {
                    if phi.block == edge.target {
                        let mut supplied = None;
                        for &(incoming, sym) in &phi.inputs {
                            budget.charge_work(1)?;
                            if incoming == id {
                                if supplied.is_some() {
                                    return Err(RequestError::InternalInvariant(
                                        "edge has duplicate Phi inputs",
                                    ));
                                }
                                supplied = Some(sym);
                            }
                        }
                        let supplied = supplied
                            .ok_or(RequestError::InternalInvariant("edge has no Phi input"))?;
                        if supplied != lookup(&env, phi.register, budget)? {
                            return Err(RequestError::InternalInvariant(
                                "Phi edge is not the post-commit environment",
                            ));
                        }
                    }
                }
            }
        }
    }
    verify_availability(&ssa, budget)?;
    Ok(ssa)
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
        for (index, definition) in ssa.values.iter().enumerate() {
            budget.charge_work(1)?;
            let sources = match *definition {
                Definition::Move(source) => [Some(source), None],
                Definition::Compare(left, _, right) => [Some(left), Some(right)],
                Definition::Binary(left, _, BoundOperand::Value(right)) => {
                    [Some(left), Some(right)]
                }
                Definition::Binary(left, _, BoundOperand::Literal(_)) => [Some(left), None],
                _ => [None, None],
            };
            for source in sources.into_iter().flatten() {
                budget.charge_work(1)?;
                if bad[ix(source.0)?] && !bad[index] {
                    bad[index] = true;
                    changed = true;
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
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        for op in &ssa.bound[index].ops {
            budget.charge_work(1)?;
            if let Some(source) = op.source {
                available(
                    source,
                    use_register(op.op).ok_or(RequestError::InternalInvariant(
                        "bound source has no original register",
                    ))?,
                    op.pc,
                )?;
            }
            if let Some(BoundOperand::Value(source)) = op.other {
                available(
                    source,
                    other_register(op.op).ok_or(RequestError::InternalInvariant(
                        "bound right source has no original register",
                    ))?,
                    op.pc,
                )?;
            }
        }
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
    for (index, site) in ssa.calls.iter().enumerate() {
        budget.charge_work(1)?;
        let call = ssa.bound_calls[index]
            .as_ref()
            .ok_or(RequestError::InternalInvariant(
                "reachable call has no argument binding",
            ))?;
        for (&register, &argument) in site.registers.iter().zip(&call.arguments) {
            budget.charge_work(1)?;
            available(argument, register, site.pc)?;
        }
    }
    Ok(())
}
