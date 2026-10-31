use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::raw::{LayoutKind, RawLiteral, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BlockId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EdgeId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValueId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Int,
    Boolean,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Sym {
    Value(ValueId),
    Bad,
}
#[derive(Clone, Copy)]
pub(crate) enum Op {
    Constant(RegisterId, i32),
    Move(RegisterId, RegisterId),
    Nop,
}
#[derive(Clone, Copy)]
pub(crate) enum Terminal {
    Return(RegisterId),
    Jump(BlockId),
    Test {
        register: RegisterId,
        opcode: u8,
        yes: BlockId,
        no: BlockId,
    },
}
pub(crate) struct Block {
    pub pc: CodeUnitOffset,
    pub terminal_pc: CodeUnitOffset,
    pub ops: Vec<(CodeUnitOffset, Op)>,
    pub terminal: Terminal,
}
pub(crate) struct Edge {
    pub source: BlockId,
    pub target: BlockId,
}
#[derive(Clone, Copy)]
pub(crate) enum Definition {
    Parameter(u32),
    Constant(i32),
    Move(ValueId),
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
}
pub(crate) struct BoundBlock {
    pub ops: Vec<BoundOp>,
    pub terminal: Option<ValueId>,
}
pub(crate) struct VerifiedSsa<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<Kind>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub values: Vec<Definition>,
    pub phis: Vec<Phi>,
    pub bound: Vec<BoundBlock>,
    pub kinds: Vec<Option<Kind>>,
}
#[derive(Clone, Copy)]
enum Decoded {
    Op(Op),
    Return(RegisterId),
    Jump(CodeUnitOffset),
    Test(RegisterId, u8, CodeUnitOffset, CodeUnitOffset),
}
struct Flow<'a> {
    raw: RawMethodView<'a>,
    parameters: Vec<Kind>,
    blocks: Vec<Block>,
    edges: Vec<Edge>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(values, 1, budget)?;
    values.push(value);
    Ok(())
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
fn destination(op: Op) -> Option<RegisterId> {
    match op {
        Op::Constant(r, _) | Op::Move(r, _) => Some(r),
        Op::Nop => None,
    }
}
fn use_register(op: Op) -> Option<RegisterId> {
    match op {
        Op::Move(_, r) => Some(r),
        _ => None,
    }
}
fn terminal_register(terminal: Terminal) -> Option<RegisterId> {
    match terminal {
        Terminal::Return(r) | Terminal::Test { register: r, .. } => Some(r),
        Terminal::Jump(_) => None,
    }
}
fn contains_pc(
    pcs: &[CodeUnitOffset],
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for old in pcs {
        budget.charge_work(1)?;
        if *old == pc {
            return Ok(true);
        }
    }
    Ok(false)
}

fn block_at(
    pcs: &[CodeUnitOffset],
    pc: CodeUnitOffset,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<BlockId, RequestError> {
    for (index, old) in pcs.iter().enumerate() {
        budget.charge_work(1)?;
        if *old == pc {
            return Ok(BlockId(
                u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
            ));
        }
    }
    Err(RequestError::InternalInvariant(
        "checked branch has no block leader",
    ))
}

fn flow<'a>(
    input: &MethodInput<'_, 'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Flow<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 8 == 0
        || !raw.try_byte_range().as_range().is_empty()
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[73], budget)?
    {
        return Ok(None);
    }
    let mut parameters = Vec::new();
    let mut supported = true;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            let kind = if name.matches_with(&[73], budget)? {
                Kind::Int
            } else if name.matches_with(&[90], budget)? {
                Kind::Boolean
            } else {
                supported = false;
                return Ok(());
            };
            push(&mut parameters, kind, budget)
        },
    )?;
    if !supported {
        return Ok(None);
    }
    let mut instructions = Vec::new();
    let mut leaders = Vec::new();
    push(&mut leaders, CodeUnitOffset(0), budget)?;
    let mut pc = CodeUnitOffset(0);
    let mut has_branch = false;
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let next = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
        if checked.layout().is_reachable(pc) {
            let LayoutKind::Instruction(spec) = layout.kind() else {
                return Ok(None);
            };
            let operands =
                crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
                    .map_err(crate::input::MethodError::Decode)?;
            let reg = |n| {
                operands
                    .registers()
                    .iter()
                    .nth(n)
                    .ok_or(RequestError::InternalInvariant(
                        "checked branch operand absent",
                    ))
            };
            let target = || -> Result<CodeUnitOffset, RequestError> {
                pc.0.checked_add_signed(
                    operands
                        .offset()
                        .ok_or(RequestError::InternalInvariant(
                            "checked branch delta absent",
                        ))?
                        .0,
                )
                .map(CodeUnitOffset)
                .ok_or(RequestError::InternalInvariant(
                    "checked branch target overflowed",
                ))
            };
            let decoded = match spec.opcode {
                0 => Decoded::Op(Op::Nop),
                1..=3 => Decoded::Op(Op::Move(reg(0)?, reg(1)?)),
                0x12..=0x15 => Decoded::Op(Op::Constant(
                    reg(0)?,
                    match operands.literal() {
                        Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                        Some(RawLiteral::Signed16(v)) => i32::from(v),
                        Some(RawLiteral::Signed8(v)) => i32::from(v),
                        _ => return Err(RequestError::InternalInvariant("scalar constant absent")),
                    },
                )),
                0xf => Decoded::Return(reg(0)?),
                0x28..=0x2a => {
                    has_branch = true;
                    let to = target()?;
                    push(&mut leaders, to, budget)?;
                    Decoded::Jump(to)
                }
                0x38..=0x3d => {
                    has_branch = true;
                    let to = target()?;
                    push(&mut leaders, to, budget)?;
                    push(&mut leaders, next, budget)?;
                    Decoded::Test(reg(0)?, spec.opcode, to, next)
                }
                _ => return Ok(None),
            };
            push(&mut instructions, (pc, decoded), budget)?;
        }
        pc = next;
    }
    if !has_branch {
        return Ok(None);
    }
    // Block order follows original instruction order; no output sorting is used.
    let mut pcs = Vec::new();
    for &(pc, _) in &instructions {
        budget.charge_work(1)?;
        if contains_pc(&leaders, pc, budget)? {
            push(&mut pcs, pc, budget)?;
        }
    }
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut start = CodeUnitOffset(0);
    for (position, &(pc, instruction)) in instructions.iter().enumerate() {
        budget.charge_work(1)?;
        if current.is_empty() {
            start = pc;
        }
        let next = instructions.get(position + 1).map(|entry| entry.0);
        let terminal = match instruction {
            Decoded::Op(op) => {
                push(&mut current, (pc, op), budget)?;
                if let Some(next) = next {
                    if contains_pc(&pcs, next, budget)? {
                        Some(Terminal::Jump(block_at(&pcs, next, budget)?))
                    } else {
                        None
                    }
                } else {
                    return Err(RequestError::InternalInvariant(
                        "branch flow falls off its end",
                    ));
                }
            }
            Decoded::Return(r) => Some(Terminal::Return(r)),
            Decoded::Jump(to) => Some(Terminal::Jump(block_at(&pcs, to, budget)?)),
            Decoded::Test(r, opcode, yes, no) => Some(Terminal::Test {
                register: r,
                opcode,
                yes: block_at(&pcs, yes, budget)?,
                no: block_at(&pcs, no, budget)?,
            }),
        };
        if let Some(terminal) = terminal {
            push(
                &mut blocks,
                Block {
                    pc: start,
                    terminal_pc: pc,
                    ops: std::mem::take(&mut current),
                    terminal,
                },
                budget,
            )?;
        }
    }
    if blocks.len() != pcs.len() {
        return Err(RequestError::InternalInvariant(
            "branch leader/block correspondence failed",
        ));
    }
    let mut edges = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let source = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        match block.terminal {
            Terminal::Jump(target) => push(&mut edges, Edge { source, target }, budget)?,
            Terminal::Test { yes, no, .. } => {
                push(
                    &mut edges,
                    Edge {
                        source,
                        target: yes,
                    },
                    budget,
                )?;
                push(&mut edges, Edge { source, target: no }, budget)?;
            }
            _ => {}
        }
    }
    Ok(Some(Flow {
        raw,
        parameters,
        blocks,
        edges,
    }))
}

pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedSsa<'a>>, RequestError> {
    let Some(flow) = flow(input, budget)? else {
        return Ok(None);
    };
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
                if let Some(register) = use_register(op) {
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
            let result = match op {
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
        push(&mut bound, BoundBlock { ops, terminal }, budget)?;
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
    let ssa = VerifiedSsa {
        _sealed: (),
        raw: flow.raw,
        parameters: flow.parameters,
        blocks: flow.blocks,
        edges: flow.edges,
        values,
        phis,
        bound,
        kinds: Vec::new(),
    };
    verify(ssa, &seeds, budget)
}

fn verify<'a>(
    mut ssa: VerifiedSsa<'a>,
    seeds: &[(RegisterId, Sym)],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedSsa<'a>>, RequestError> {
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
            match *op {
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
    let mut bad = Vec::new();
    let mut grounded = Vec::new();
    let mut kinds = Vec::new();
    let mut type_conflict = false;
    for definition in &ssa.values {
        push(&mut bad, false, budget)?;
        push(
            &mut grounded,
            !matches!(definition, Definition::Phi),
            budget,
        )?;
        push(
            &mut kinds,
            match definition {
                Definition::Parameter(index) => Some(ssa.parameters[ix(*index)?]),
                Definition::Constant(_) => Some(Kind::Int),
                _ => None,
            },
            budget,
        )?;
    }
    loop {
        let mut changed = false;
        for (index, definition) in ssa.values.iter().enumerate() {
            budget.charge_work(1)?;
            if let Definition::Move(source) = definition {
                let source = ix(source.0)?;
                if bad[source] && !bad[index] {
                    bad[index] = true;
                    changed = true;
                }
                if kinds[index].is_none() && kinds[source].is_some() {
                    kinds[index] = kinds[source];
                    changed = true;
                }
            }
        }
        for phi in &ssa.phis {
            let index = ix(phi.result.0)?;
            for &(_, input) in &phi.inputs {
                budget.charge_work(1)?;
                match input {
                    Sym::Bad => {
                        if !bad[index] {
                            bad[index] = true;
                            changed = true;
                        }
                    }
                    Sym::Value(source) => {
                        let source = ix(source.0)?;
                        if bad[source] && !bad[index] {
                            bad[index] = true;
                            changed = true;
                        }
                        if grounded[source] && !grounded[index] {
                            grounded[index] = true;
                            changed = true;
                        }
                        if let Some(kind) = kinds[source] {
                            if let Some(old) = kinds[index] {
                                if old != kind {
                                    type_conflict = true;
                                }
                            } else {
                                kinds[index] = Some(kind);
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (index, block) in ssa.blocks.iter().enumerate() {
        for op in &ssa.bound[index].ops {
            budget.charge_work(1)?;
            if let Some(source) = op.source
                && (bad[ix(source.0)?] || !grounded[ix(source.0)?])
            {
                return Err(RequestError::UndefinedRegister {
                    register: use_register(op.op).ok_or(RequestError::InternalInvariant(
                        "bad use has no original register",
                    ))?,
                    pc: op.pc,
                });
            }
        }
        if let Some(register) = terminal_register(block.terminal) {
            let id = ix(ssa.bound[index]
                .terminal
                .ok_or(RequestError::InternalInvariant(
                    "terminal lost its bound use",
                ))?
                .0)?;
            if bad[id] || !grounded[id] {
                return Err(RequestError::UndefinedRegister {
                    register,
                    pc: block.terminal_pc,
                });
            }
        }
    }
    if type_conflict {
        return Ok(None);
    }
    ssa.kinds = kinds;
    Ok(Some(ssa))
}
