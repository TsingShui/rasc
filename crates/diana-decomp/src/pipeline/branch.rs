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
#[derive(Clone, Copy)]
pub(crate) enum DeclarationType {
    Int,
    Boolean,
    Object,
    Float,
    Narrow(crate::p5_types::Narrow),
}
impl DeclarationType {
    pub(crate) fn kind(self) -> Kind {
        match self {
            Self::Int | Self::Narrow(_) => Kind::Int,
            Self::Boolean => Kind::Boolean,
            Self::Object => Kind::Object,
            Self::Float => Kind::Float,
        }
    }
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Narrow(kind) => kind.spelling(),
            _ => self.kind().spelling(),
        }
    }
    fn range(self) -> Option<crate::p5_types::IntRange> {
        use crate::p5_types::IntRange;
        match self {
            Self::Int => Some(IntRange::ALL),
            Self::Boolean => Some(IntRange::BOOLEAN),
            Self::Narrow(kind) => Some(kind.range()),
            Self::Object | Self::Float => None,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ResultType {
    Void,
    Value(DeclarationType),
}
impl ResultType {
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Value(value) => value.spelling(),
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Int,
    Boolean,
    Object,
    Float,
}
impl Kind {
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Boolean => "boolean",
            Self::Object => "java.lang.Object",
            Self::Float => "float",
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Sym {
    Value(ValueId),
    Bad,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Arithmetic {
    Add,
    Sub,
    Mul,
    And,
}
#[derive(Clone, Copy)]
pub(crate) enum Operand {
    Register(RegisterId),
    Literal(i32),
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum BoundOperand {
    Value(ValueId),
    Literal(i32),
}
#[derive(Clone, Copy)]
pub(crate) struct CallId(pub u32);
pub(crate) struct CallSite {
    pub contract: crate::p4_calls::StaticContract,
    pub registers: Vec<RegisterId>,
    pub pc: CodeUnitOffset,
    pub result_pc: Option<CodeUnitOffset>,
}
pub(crate) struct BoundCall {
    pub arguments: Vec<ValueId>,
}
#[derive(Clone, Copy)]
pub(crate) enum Op {
    Call(CallId, Option<RegisterId>),
    Compare(
        RegisterId,
        RegisterId,
        crate::pipeline::scalar::Compare,
        RegisterId,
    ),
    Binary(RegisterId, RegisterId, Arithmetic, Operand),
    Constant(RegisterId, i32),
    Move(RegisterId, RegisterId),
    Nop,
}
#[derive(Clone, Copy)]
pub(crate) struct SwitchId(pub u32);
pub(crate) struct Switch {
    pub cases: Vec<(i32, BlockId)>,
    pub default: BlockId,
}
#[derive(Clone, Copy)]
pub(crate) enum Terminal {
    ReturnVoid,
    Switch {
        register: RegisterId,
        table: SwitchId,
    },
    Return(RegisterId),
    Jump(BlockId),
    Test {
        register: RegisterId,
        other: Option<RegisterId>,
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
pub(crate) struct VerifiedSsa<'a> {
    _sealed: (),
    pub switches: Vec<Switch>,
    pub calls: Vec<CallSite>,
    pub bound_calls: Vec<Option<BoundCall>>,
    pub result: ResultType,
    pub integral_bits: Vec<u8>,
    pub ranges: Vec<Option<crate::p5_types::IntRange>>,
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<DeclarationType>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub values: Vec<Definition>,
    pub phis: Vec<Phi>,
    pub bound: Vec<BoundBlock>,
    pub kinds: Vec<Option<Kind>>,
}
#[derive(Clone, Copy)]
enum Decoded {
    ReturnVoid,
    Switch(RegisterId, u32),
    Op(Op),
    Return(RegisterId),
    Jump(CodeUnitOffset),
    Test(
        RegisterId,
        Option<RegisterId>,
        u8,
        CodeUnitOffset,
        CodeUnitOffset,
    ),
}
struct Flow<'a> {
    calls: Vec<CallSite>,
    result: ResultType,
    switches: Vec<Switch>,
    raw: RawMethodView<'a>,
    parameters: Vec<DeclarationType>,
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
        Op::Call(_, destination) => destination,
        Op::Constant(r, _) | Op::Move(r, _) | Op::Binary(r, _, _, _) | Op::Compare(r, _, _, _) => {
            Some(r)
        }
        Op::Nop => None,
    }
}
fn use_register(op: Op) -> Option<RegisterId> {
    match op {
        Op::Move(_, r) | Op::Binary(_, r, _, _) | Op::Compare(_, r, _, _) => Some(r),
        _ => None,
    }
}
fn other_register(op: Op) -> Option<RegisterId> {
    match op {
        Op::Binary(_, _, _, Operand::Register(register)) | Op::Compare(_, _, _, register) => {
            Some(register)
        }
        _ => None,
    }
}
fn terminal_other(terminal: Terminal) -> Option<RegisterId> {
    match terminal {
        Terminal::Test { other, .. } => other,
        _ => None,
    }
}
fn terminal_register(terminal: Terminal) -> Option<RegisterId> {
    match terminal {
        Terminal::Return(r)
        | Terminal::Test { register: r, .. }
        | Terminal::Switch { register: r, .. } => Some(r),
        Terminal::Jump(_) | Terminal::ReturnVoid => None,
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
    catalog: &crate::DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Flow<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 8 == 0 || !raw.try_byte_range().as_range().is_empty() {
        return Ok(None);
    }
    let result = if input
        .declaration()
        .prototype
        .return_type
        .matches_with(&[86], budget)?
    {
        ResultType::Void
    } else {
        ResultType::Value(
            if input
                .declaration()
                .prototype
                .return_type
                .matches_with(&[73], budget)?
            {
                DeclarationType::Int
            } else if input
                .declaration()
                .prototype
                .return_type
                .matches_with(&[90], budget)?
            {
                DeclarationType::Boolean
            } else if input
                .declaration()
                .prototype
                .return_type
                .matches_with(&[66], budget)?
            {
                DeclarationType::Narrow(crate::p5_types::Narrow::Byte)
            } else if input
                .declaration()
                .prototype
                .return_type
                .matches_with(&[83], budget)?
            {
                DeclarationType::Narrow(crate::p5_types::Narrow::Short)
            } else if input
                .declaration()
                .prototype
                .return_type
                .matches_with(&[67], budget)?
            {
                DeclarationType::Narrow(crate::p5_types::Narrow::Char)
            } else {
                return Ok(None);
            },
        )
    };
    let mut parameters = Vec::new();
    let mut supported = true;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            let kind = if name.matches_with(&[73], budget)? {
                DeclarationType::Int
            } else if name.matches_with(&[90], budget)? {
                DeclarationType::Boolean
            } else if name.matches_with(&[70], budget)? {
                DeclarationType::Float
            } else if name.matches_with(&[66], budget)? {
                DeclarationType::Narrow(crate::p5_types::Narrow::Byte)
            } else if name.matches_with(&[83], budget)? {
                DeclarationType::Narrow(crate::p5_types::Narrow::Short)
            } else if name.matches_with(&[67], budget)? {
                DeclarationType::Narrow(crate::p5_types::Narrow::Char)
            } else if name.matches_with(
                &[
                    76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
                ],
                budget,
            )? {
                DeclarationType::Object
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
    let mut calls = Vec::new();
    let mut raw_switches: Vec<(Vec<(i32, CodeUnitOffset)>, CodeUnitOffset)> = Vec::new();
    let mut leaders = Vec::new();
    push(&mut leaders, CodeUnitOffset(0), budget)?;
    let mut pc = CodeUnitOffset(0);
    let mut has_branch = false;
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let mut next = pc
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
                0x90..=0x92
                | 0x95
                | 0xb0..=0xb2
                | 0xb5
                | 0xd0
                | 0xd2
                | 0xd5
                | 0xd8
                | 0xda
                | 0xdd => {
                    let arithmetic = match spec.opcode {
                        0x90 | 0xb0 | 0xd0 | 0xd8 => Arithmetic::Add,
                        0x91 | 0xb1 => Arithmetic::Sub,
                        0x95 | 0xb5 | 0xd5 | 0xdd => Arithmetic::And,
                        _ => Arithmetic::Mul,
                    };
                    let (left, right) = match spec.opcode {
                        0x90..=0x92 | 0x95 => (reg(1)?, Operand::Register(reg(2)?)),
                        0xb0..=0xb2 | 0xb5 => (reg(0)?, Operand::Register(reg(1)?)),
                        _ => (
                            reg(1)?,
                            Operand::Literal(match operands.literal() {
                                Some(RawLiteral::Signed8(v)) => i32::from(v),
                                Some(RawLiteral::Signed16(v)) => i32::from(v),
                                _ => {
                                    return Err(RequestError::InternalInvariant(
                                        "checked arithmetic immediate absent",
                                    ));
                                }
                            }),
                        ),
                    };
                    Decoded::Op(Op::Binary(reg(0)?, left, arithmetic, right))
                }
                0xe => Decoded::ReturnVoid,
                0xf => Decoded::Return(reg(0)?),
                0x2b | 0x2c => {
                    has_branch = true;
                    let payload = target()?;
                    let payload_layout =
                        instruction_layout(raw.instructions(), payload, raw.version())
                            .map_err(crate::input::MethodError::Decode)?;
                    let cases = crate::input::operands::switch_payload(
                        raw.instructions(),
                        payload,
                        payload_layout,
                    )
                    .map_err(crate::input::MethodError::Decode)?;
                    let mut targets = Vec::new();
                    for case in cases {
                        budget.charge_work(1)?;
                        let to =
                            pc.0.checked_add_signed(case.offset.0)
                                .map(CodeUnitOffset)
                                .ok_or(RequestError::InternalInvariant(
                                    "checked switch target overflowed",
                                ))?;
                        let key = i32::try_from(case.key).map_err(|_| {
                            RequestError::InternalInvariant("checked switch key overflowed")
                        })?;
                        push(&mut leaders, to, budget)?;
                        push(&mut targets, (key, to), budget)?;
                    }
                    push(&mut leaders, next, budget)?;
                    let id = u32::try_from(raw_switches.len())
                        .map_err(|_| RequestError::PlatformCapacity)?;
                    push(&mut raw_switches, (targets, next), budget)?;
                    Decoded::Switch(reg(0)?, id)
                }
                0x2d | 0x2e => Decoded::Op(Op::Compare(
                    reg(0)?,
                    reg(1)?,
                    if spec.opcode == 0x2d {
                        crate::pipeline::scalar::Compare::FloatL
                    } else {
                        crate::pipeline::scalar::Compare::FloatG
                    },
                    reg(2)?,
                )),
                0x71 | 0x77 => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant(
                            "checked invoke has no reference",
                        ));
                    };
                    let crate::raw::OperandReference::Method(reference) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "checked invoke has wrong reference kind",
                        ));
                    };
                    let Some(contract) =
                        crate::p4_calls::resolve_static(input, catalog, reference, budget)?
                    else {
                        return Err(RequestError::BindingEvidenceMissing { reference, pc });
                    };
                    let mut registers = Vec::new();
                    for register in operands.registers().iter() {
                        budget.charge_work(1)?;
                        push(&mut registers, register, budget)?;
                    }
                    if registers.len() != contract.formals.len() {
                        return Err(RequestError::InternalInvariant(
                            "checked invoke differs from contract words",
                        ));
                    }
                    let mut destination = None;
                    let mut result_pc = None;
                    if next.0 < raw.code_units().0 {
                        let result_layout =
                            instruction_layout(raw.instructions(), next, raw.version())
                                .map_err(crate::input::MethodError::Decode)?;
                        if matches!(result_layout.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x0a)
                        {
                            if contract.result != crate::p4_calls::StaticResult::Int {
                                return Err(RequestError::InternalInvariant(
                                    "void call acquired move-result",
                                ));
                            }
                            let result = crate::input::operands::instruction_operands(
                                raw.instructions(),
                                next,
                                result_layout,
                            )
                            .map_err(crate::input::MethodError::Decode)?;
                            destination = Some(result.registers().iter().next().ok_or(
                                RequestError::InternalInvariant("move-result lost destination"),
                            )?);
                            result_pc = Some(next);
                            next = next
                                .checked_advance(result_layout.width())
                                .ok_or(RequestError::PlatformCapacity)?;
                        }
                    }
                    let call = CallId(
                        u32::try_from(calls.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(
                        &mut calls,
                        CallSite {
                            contract,
                            registers,
                            pc,
                            result_pc,
                        },
                        budget,
                    )?;
                    Decoded::Op(Op::Call(call, destination))
                }
                0x28..=0x2a => {
                    has_branch = true;
                    let to = target()?;
                    push(&mut leaders, to, budget)?;
                    Decoded::Jump(to)
                }
                0x32..=0x3d => {
                    has_branch = true;
                    let to = target()?;
                    push(&mut leaders, to, budget)?;
                    push(&mut leaders, next, budget)?;
                    Decoded::Test(
                        reg(0)?,
                        if spec.opcode <= 0x37 {
                            Some(reg(1)?)
                        } else {
                            None
                        },
                        spec.opcode,
                        to,
                        next,
                    )
                }
                _ => return Ok(None),
            };
            push(&mut instructions, (pc, decoded), budget)?;
        }
        pc = next;
    }
    if !has_branch
        && calls.is_empty()
        && !matches!(result, ResultType::Value(DeclarationType::Narrow(_)))
        && !parameters
            .iter()
            .any(|kind| matches!(kind, DeclarationType::Narrow(_)))
    {
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
    let mut switches = Vec::new();
    let mut current = Vec::new();
    let mut start = CodeUnitOffset(0);
    for (position, &(pc, instruction)) in instructions.iter().enumerate() {
        budget.charge_work(1)?;
        if current.is_empty() {
            start = pc;
        }
        let next = instructions.get(position + 1).map(|entry| entry.0);
        let terminal = match instruction {
            Decoded::Switch(register, id) => {
                let (targets, default) =
                    raw_switches
                        .get(ix(id)?)
                        .ok_or(RequestError::InternalInvariant(
                            "switch lost its decoded payload",
                        ))?;
                let mut cases = Vec::new();
                for &(key, to) in targets {
                    budget.charge_work(1)?;
                    push(&mut cases, (key, block_at(&pcs, to, budget)?), budget)?;
                }
                let table = SwitchId(
                    u32::try_from(switches.len()).map_err(|_| RequestError::PlatformCapacity)?,
                );
                push(
                    &mut switches,
                    Switch {
                        cases,
                        default: block_at(&pcs, *default, budget)?,
                    },
                    budget,
                )?;
                Some(Terminal::Switch { register, table })
            }
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
            Decoded::ReturnVoid => Some(Terminal::ReturnVoid),
            Decoded::Return(r) => Some(Terminal::Return(r)),
            Decoded::Jump(to) => Some(Terminal::Jump(block_at(&pcs, to, budget)?)),
            Decoded::Test(r, other, opcode, yes, no) => Some(Terminal::Test {
                register: r,
                other,
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
            Terminal::Switch { table, .. } => {
                let table = &switches[ix(table.0)?];
                for &(_, target) in &table.cases {
                    budget.charge_work(1)?;
                    push(&mut edges, Edge { source, target }, budget)?;
                }
                push(
                    &mut edges,
                    Edge {
                        source,
                        target: table.default,
                    },
                    budget,
                )?;
            }
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
        calls,
        result,
        switches,
        raw,
        parameters,
        blocks,
        edges,
    }))
}

pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &crate::DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedSsa<'a>>, RequestError> {
    let Some(flow) = flow(input, catalog, budget)? else {
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
    let ssa = VerifiedSsa {
        _sealed: (),
        switches: flow.switches,
        calls: flow.calls,
        bound_calls,
        result: flow.result,
        integral_bits: Vec::new(),
        ranges: Vec::new(),
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

fn merge_range(
    target: &mut Option<crate::p5_types::IntRange>,
    source: Option<crate::p5_types::IntRange>,
) -> bool {
    let merged = match (*target, source) {
        (None, value) => value,
        (value, None) => value,
        (Some(a), Some(b)) => Some(a.hull(b)),
    };
    if merged == *target {
        false
    } else {
        *target = merged;
        true
    }
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
                        if !matches!(result.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x0a)
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
    let mut bad = Vec::new();
    let mut grounded = Vec::new();
    let mut integral_bits = Vec::new();
    let mut ranges = Vec::new();
    let mut kinds = Vec::new();
    let mut type_conflict = false;
    for definition in &ssa.values {
        let bits = match definition {
            Definition::Parameter(index) => match ssa.parameters[ix(*index)?].kind() {
                Kind::Boolean => 3,
                Kind::Int => 7,
                Kind::Object | Kind::Float => 0,
            },
            Definition::Constant(value) => match value {
                0 => 1,
                1 => 2,
                _ => 4,
            },
            Definition::Binary(_, _, _)
            | Definition::Compare(_, _, _)
            | Definition::CallResult(_) => 7,
            _ => 0,
        };
        let range = match definition {
            Definition::Parameter(index) => ssa.parameters[ix(*index)?].range(),
            Definition::Constant(value) => Some(crate::p5_types::IntRange::literal(*value)),
            Definition::Binary(_, _, _)
            | Definition::Compare(_, _, _)
            | Definition::CallResult(_) => Some(crate::p5_types::IntRange::ALL),
            _ => None,
        };
        push(&mut ranges, range, budget)?;
        push(&mut integral_bits, bits, budget)?;
        push(&mut bad, false, budget)?;
        push(
            &mut grounded,
            !matches!(definition, Definition::Phi),
            budget,
        )?;
        push(
            &mut kinds,
            match definition {
                Definition::Parameter(index) => Some(ssa.parameters[ix(*index)?].kind()),
                Definition::Constant(_)
                | Definition::Binary(_, _, _)
                | Definition::Compare(_, _, _)
                | Definition::CallResult(_) => Some(Kind::Int),
                _ => None,
            },
            budget,
        )?;
    }
    loop {
        let mut changed = false;
        for (index, definition) in ssa.values.iter().enumerate() {
            budget.charge_work(1)?;
            if let Definition::Compare(left, _, right) = definition {
                for source in [left, right] {
                    budget.charge_work(1)?;
                    let at = ix(source.0)?;
                    if bad[at] && !bad[index] {
                        bad[index] = true;
                        changed = true;
                    }
                    if kinds[at].is_some_and(|kind| kind != Kind::Float) {
                        type_conflict = true;
                    }
                }
            }
            if let Definition::Binary(left, _, right) = definition {
                let mut sources = [Some(*left), None];
                if let BoundOperand::Value(right) = right {
                    sources[1] = Some(*right);
                }
                for source in sources.into_iter().flatten() {
                    budget.charge_work(1)?;
                    let source = ix(source.0)?;
                    if bad[source] && !bad[index] {
                        bad[index] = true;
                        changed = true;
                    }
                    if kinds[source].is_some_and(|kind| kind != Kind::Int) {
                        type_conflict = true;
                    }
                }
            }
            if let Definition::Move(source) = definition {
                let source = ix(source.0)?;
                if bad[source] && !bad[index] {
                    bad[index] = true;
                    changed = true;
                }
                let source_range = ranges[source];
                changed |= merge_range(&mut ranges[index], source_range);
                let merged = integral_bits[index] | integral_bits[source];
                if merged != integral_bits[index] {
                    integral_bits[index] = merged;
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
                        let source_range = ranges[source];
                        changed |= merge_range(&mut ranges[index], source_range);
                        let merged = integral_bits[index] | integral_bits[source];
                        if merged != integral_bits[index] {
                            integral_bits[index] = merged;
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
    for (index, block) in ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        for op in &ssa.bound[index].ops {
            budget.charge_work(1)?;
            if let Some(BoundOperand::Value(source)) = op.other
                && (bad[ix(source.0)?] || !grounded[ix(source.0)?])
            {
                return Err(RequestError::UndefinedRegister {
                    register: other_register(op.op).ok_or(RequestError::InternalInvariant(
                        "bad arithmetic right use has no register",
                    ))?,
                    pc: op.pc,
                });
            }
        }
        if let Some(source) = ssa.bound[index].other
            && (bad[ix(source.0)?] || !grounded[ix(source.0)?])
        {
            return Err(RequestError::UndefinedRegister {
                register: terminal_other(block.terminal).ok_or(RequestError::InternalInvariant(
                    "bad comparison right use has no register",
                ))?,
                pc: block.terminal_pc,
            });
        }
    }
    if type_conflict {
        return Ok(None);
    }
    for (index, site) in ssa.calls.iter().enumerate() {
        let Some(call) = &ssa.bound_calls[index] else {
            return Err(RequestError::InternalInvariant(
                "reachable call has no argument binding",
            ));
        };
        for (&register, &argument) in site.registers.iter().zip(&call.arguments) {
            budget.charge_work(1)?;
            let argument = ix(argument.0)?;
            if bad[argument] || !grounded[argument] {
                return Err(RequestError::UndefinedRegister {
                    register,
                    pc: site.pc,
                });
            }
        }
    }
    ssa.kinds = kinds;
    ssa.integral_bits = integral_bits;
    ssa.ranges = ranges;
    Ok(Some(ssa))
}
