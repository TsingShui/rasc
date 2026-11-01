use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::p5_types::normal::Kind;
use crate::raw::{LayoutKind, RawLiteral, instruction_layout};
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BlockId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EdgeId(pub u32);
#[derive(Clone, Copy)]
pub(crate) enum DeclarationType {
    Int,
    Boolean,
    Object,
    String,
    Float,
    Narrow(crate::p5_types::Narrow),
}
impl DeclarationType {
    pub(crate) fn kind(self) -> Kind {
        match self {
            Self::Int | Self::Narrow(_) => Kind::Int,
            Self::Boolean => Kind::Boolean,
            Self::Object => Kind::Object,
            Self::String => Kind::String,
            Self::Float => Kind::Float,
        }
    }
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Narrow(kind) => kind.spelling(),
            _ => self.kind().spelling(),
        }
    }
    pub(crate) fn range(self) -> Option<crate::p5_types::IntRange> {
        use crate::p5_types::IntRange;
        match self {
            Self::Int => Some(IntRange::ALL),
            Self::Boolean => Some(IntRange::BOOLEAN),
            Self::Narrow(kind) => Some(kind.range()),
            Self::Object | Self::String | Self::Float => None,
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
pub(crate) enum Arithmetic {
    Add,
    Sub,
    Mul,
    And,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Operand {
    Register(RegisterId),
    Literal(i32),
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CallId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OpId(pub u32);
#[derive(Clone, Copy)]
pub(crate) struct Operation {
    pub id: OpId,
    pub pc: CodeUnitOffset,
    pub kind: Op,
}
pub(crate) struct CallSite {
    pub contract: crate::p4_calls::StaticContract,
    pub registers: Vec<RegisterId>,
    pub pc: CodeUnitOffset,
    pub result_pc: Option<CodeUnitOffset>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
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
    pub ops: Vec<Operation>,
    pub terminal: Terminal,
}
pub(crate) struct Edge {
    pub source: BlockId,
    pub target: BlockId,
}
#[derive(Clone, Copy)]
pub(crate) struct Effects {
    pub heap_read: bool,
    pub heap_write: bool,
    pub may_throw: bool,
    pub call: bool,
    pub class_init: bool,
}
impl Effects {
    pub(crate) fn observable(self) -> bool {
        self.heap_read || self.heap_write || self.may_throw || self.call || self.class_init
    }
}
impl Op {
    pub(crate) fn effects(self) -> Effects {
        let conservative = matches!(self, Self::Call(_, _));
        Effects {
            heap_read: conservative,
            heap_write: conservative,
            may_throw: conservative,
            call: conservative,
            class_init: conservative,
        }
    }
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
pub(crate) struct FlowBody<'a> {
    _sealed: (),
    pub calls: Vec<CallSite>,
    pub result: ResultType,
    pub switches: Vec<Switch>,
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<DeclarationType>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(crate) fn destination(op: Op) -> Option<RegisterId> {
    match op {
        Op::Call(_, destination) => destination,
        Op::Constant(r, _) | Op::Move(r, _) | Op::Binary(r, _, _, _) | Op::Compare(r, _, _, _) => {
            Some(r)
        }
        Op::Nop => None,
    }
}
pub(crate) enum Reads<'a> {
    Call(std::slice::Iter<'a, RegisterId>),
    Local(std::array::IntoIter<Option<RegisterId>, 3>),
}
impl Iterator for Reads<'_> {
    type Item = RegisterId;
    fn next(&mut self) -> Option<RegisterId> {
        match self {
            Self::Call(registers) => registers.next().copied(),
            Self::Local(registers) => {
                for register in registers {
                    if register.is_some() {
                        return register;
                    }
                }
                None
            }
        }
    }
}
impl Op {
    pub(crate) fn reads(self, calls: &[CallSite]) -> Result<Reads<'_>, RequestError> {
        let registers = match self {
            Self::Call(id, _) => {
                let call = calls.get(ix(id.0)?).ok_or(RequestError::InternalInvariant(
                    "operation has a foreign call identity",
                ))?;
                return Ok(Reads::Call(call.registers.iter()));
            }
            Self::Compare(_, left, _, right) => [Some(left), Some(right), None],
            Self::Binary(_, left, _, Operand::Register(right)) => [Some(left), Some(right), None],
            Self::Binary(_, left, _, Operand::Literal(_)) | Self::Move(_, left) => {
                [Some(left), None, None]
            }
            Self::Constant(_, _) | Self::Nop => [None, None, None],
        };
        Ok(Reads::Local(registers.into_iter()))
    }
}
pub(crate) fn terminal_other(terminal: Terminal) -> Option<RegisterId> {
    match terminal {
        Terminal::Test { other, .. } => other,
        _ => None,
    }
}
pub(crate) fn terminal_register(terminal: Terminal) -> Option<RegisterId> {
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

pub(crate) fn decode<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &crate::DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<FlowBody<'a>>, RequestError> {
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
            } else if input.declaration().prototype.return_type.matches_with(
                crate::p5_types::BuiltinReference::String.descriptor(),
                budget,
            )? {
                DeclarationType::String
            } else if input.declaration().prototype.return_type.matches_with(
                crate::p5_types::BuiltinReference::Object.descriptor(),
                budget,
            )? {
                DeclarationType::Object
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
            } else if name.matches_with(
                crate::p5_types::BuiltinReference::String.descriptor(),
                budget,
            )? {
                DeclarationType::String
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
                1..=3 | 7..=9 => Decoded::Op(Op::Move(reg(0)?, reg(1)?)),
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
                0xf | 0x11 => Decoded::Return(reg(0)?),
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
                        if matches!(result_layout.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,0x0a|0x0c))
                        {
                            let valid = match contract.result {
                                crate::p4_calls::StaticResult::Int
                                | crate::p4_calls::StaticResult::Boolean => {
                                    matches!(result_layout.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x0a)
                                }
                                crate::p4_calls::StaticResult::Reference(_) => {
                                    matches!(result_layout.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x0c)
                                }
                                crate::p4_calls::StaticResult::Void => false,
                                crate::p4_calls::StaticResult::Nominal => return Ok(None),
                            };
                            if !valid {
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
    let mut next_operation = 0u32;
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
                let id = OpId(next_operation);
                next_operation = next_operation
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                push(&mut current, Operation { id, pc, kind: op }, budget)?;
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
    Ok(Some(FlowBody {
        _sealed: (),
        calls,
        result,
        switches,
        raw,
        parameters,
        blocks,
        edges,
    }))
}
