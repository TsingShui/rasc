use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::raw::{LayoutKind, RawLiteral, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, MethodBudget, RegisterId, RequestError};

#[derive(Clone, Copy)]
pub(crate) struct ValueId(pub u32);
#[derive(Clone, Copy)]
pub(crate) struct LocalId(pub u32);
#[derive(Clone, Copy)]
pub(crate) enum Binary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Ushr,
}
#[derive(Clone, Copy)]
pub(crate) enum Unary {
    Neg,
    Not,
    Byte,
    Char,
    Short,
    NegLong,
    NotLong,
    IntToLong,
    LongToInt,
    NegFloat,
    NegDouble,
    Convert { from: ScalarType, to: ScalarType },
}
impl Unary {
    pub(crate) fn signature(self) -> (ScalarType, ScalarType) {
        use ScalarType::*;
        match self {
            Self::NegLong | Self::NotLong => (Long, Long),
            Self::IntToLong => (Int, Long),
            Self::LongToInt => (Long, Int),
            Self::NegFloat => (Float, Float),
            Self::NegDouble => (Double, Double),
            Self::Convert { from, to } => (from, to),
            _ => (Int, Int),
        }
    }
}
#[derive(Clone, Copy)]
enum Operand {
    Register(RegisterId),
    Literal(i32),
}
enum Op {
    ConstantWide(RegisterId, i64),
    MoveWide(RegisterId, RegisterId),
    BinaryWide(RegisterId, RegisterId, Binary, RegisterId),
    BinaryFp(RegisterId, RegisterId, Binary, RegisterId, ScalarType),
    ReturnWide(RegisterId),
    Constant(RegisterId, i32),
    Move(RegisterId, RegisterId),
    Unary(RegisterId, Unary, RegisterId),
    Binary(RegisterId, Operand, Binary, Operand),
    Return(RegisterId),
}
struct FlowBody<'a> {
    raw: RawMethodView<'a>,
    ops: Vec<(CodeUnitOffset, Op)>,
    parameters: Vec<ScalarType>,
    result: ScalarType,
}
pub(crate) enum Expression {
    Parameter(u32),
    Constant(i32),
    ConstantWide(i64),
    Copy(ValueId),
    Unary(Unary, ValueId),
    Binary(ValueId, Binary, ValueId),
}
pub(crate) struct Definition {
    pub kind: ScalarType,
    pub expression: Expression,
    pub pc: Option<CodeUnitOffset>,
}
struct SsaBody<'a> {
    flow: FlowBody<'a>,
    definitions: Vec<Definition>,
    returned: ValueId,
}
struct ResolvedBody<'a> {
    ssa: SsaBody<'a>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ScalarType {
    Boolean,
    Int,
    Long,
    Float,
    Double,
    Bits32,
    Bits64,
}
impl ScalarType {
    pub(crate) fn spelling(self) -> Result<&'static str, RequestError> {
        Ok(match self {
            Self::Boolean => "boolean",
            Self::Int => "int",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
            _ => {
                return Err(RequestError::InternalInvariant(
                    "unsealed scalar type entered source emission",
                ));
            }
        })
    }
    pub(crate) fn words(self) -> usize {
        match self {
            Self::Boolean | Self::Int | Self::Float | Self::Bits32 => 1,
            _ => 2,
        }
    }
}
struct TypedBody<'a> {
    resolved: ResolvedBody<'a>,
    typing: crate::p5_types::Solved,
}
struct StorageBody<'a> {
    typed: TypedBody<'a>,
    locals: Vec<LocalId>,
}
pub(crate) struct PlannedBody<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub definitions: Vec<Definition>,
    pub locals: Vec<LocalId>,
    pub returned: ValueId,
    pub parameters: Vec<ScalarType>,
    pub result: ScalarType,
    pub typing: crate::p5_types::Solved,
}

pub(crate) fn plan<'a>(
    input: &MethodInput<'_, 'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<PlannedBody<'a>>, RequestError> {
    let Some(flow) = flow(input, budget)? else {
        return Ok(None);
    };
    let ssa = bind(flow, budget)?;
    // The closed Op vocabulary has neither calls nor reference-valued operations.
    let resolved = ResolvedBody { ssa };
    let Some(typing) = crate::p5_types::solve(
        &resolved.ssa.definitions,
        resolved.ssa.returned,
        resolved.ssa.flow.result,
        budget,
    )?
    else {
        return Ok(None);
    };
    let typed = TypedBody { resolved, typing };
    let mut locals = Vec::new();
    for index in 0..typed.typing.types().len() {
        budget.charge_work(1)?;
        let id = LocalId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        reserve(&mut locals, 1, budget)?;
        locals.push(id);
    }
    let storage = StorageBody { typed, locals };
    budget.checkpoint()?;
    let ssa = storage.typed.resolved.ssa;
    Ok(Some(PlannedBody {
        _sealed: (),
        raw: ssa.flow.raw,
        definitions: ssa.definitions,
        locals: storage.locals,
        returned: ssa.returned,
        parameters: ssa.flow.parameters,
        result: ssa.flow.result,
        typing: storage.typed.typing,
    }))
}

fn flow<'a>(
    input: &MethodInput<'_, 'a>,
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
        .matches_with(&[u16::from(b'I')], budget)?
    {
        ScalarType::Int
    } else if input
        .declaration()
        .prototype
        .return_type
        .matches_with(&[u16::from(b'J')], budget)?
    {
        ScalarType::Long
    } else if input
        .declaration()
        .prototype
        .return_type
        .matches_with(&[u16::from(b'F')], budget)?
    {
        ScalarType::Float
    } else if input
        .declaration()
        .prototype
        .return_type
        .matches_with(&[u16::from(b'D')], budget)?
    {
        ScalarType::Double
    } else if input
        .declaration()
        .prototype
        .return_type
        .matches_with(&[u16::from(b'Z')], budget)?
    {
        ScalarType::Boolean
    } else {
        return Ok(None);
    };
    let mut parameters = Vec::new();
    let mut supported = true;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            let kind = if name.matches_with(&[u16::from(b'I')], budget)? {
                ScalarType::Int
            } else if name.matches_with(&[u16::from(b'J')], budget)? {
                ScalarType::Long
            } else if name.matches_with(&[u16::from(b'F')], budget)? {
                ScalarType::Float
            } else if name.matches_with(&[u16::from(b'D')], budget)? {
                ScalarType::Double
            } else if name.matches_with(&[u16::from(b'Z')], budget)? {
                ScalarType::Boolean
            } else {
                supported = false;
                return Ok(());
            };
            reserve(&mut parameters, 1, budget)?;
            parameters.push(kind);
            Ok(())
        },
    )?;
    if !supported {
        return Ok(None);
    }
    let mut ops = Vec::new();
    let mut pc = CodeUnitOffset(0);
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        if checked.layout().is_reachable(pc) {
            let LayoutKind::Instruction(spec) = layout.kind() else {
                return Ok(None);
            };
            let operands =
                crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
                    .map_err(crate::input::MethodError::Decode)?;
            let reg = |index| {
                operands
                    .registers()
                    .iter()
                    .nth(index)
                    .ok_or(RequestError::InternalInvariant(
                        "checked scalar operand is absent",
                    ))
            };
            let literal = || -> Result<i32, RequestError> {
                match operands.literal() {
                    Some(RawLiteral::Bits32(bits)) => Ok(i32::from_le_bytes(bits.to_le_bytes())),
                    Some(RawLiteral::Signed8(value)) => Ok(i32::from(value)),
                    Some(RawLiteral::Signed16(value)) => Ok(i32::from(value)),
                    _ => Err(RequestError::InternalInvariant(
                        "checked scalar literal is absent",
                    )),
                }
            };
            let binary = |opcode| match opcode {
                0x90 | 0xb0 | 0xd0 | 0xd8 => Binary::Add,
                0x91 | 0xb1 | 0xd1 | 0xd9 => Binary::Sub,
                0x92 | 0xb2 | 0xd2 | 0xda => Binary::Mul,
                0x93 | 0xb3 | 0xd3 | 0xdb => Binary::Div,
                0x94 | 0xb4 | 0xd4 | 0xdc => Binary::Rem,
                0x95 | 0xb5 | 0xd5 | 0xdd => Binary::And,
                0x96 | 0xb6 | 0xd6 | 0xde => Binary::Or,
                0x97 | 0xb7 | 0xd7 | 0xdf => Binary::Xor,
                0x98 | 0xb8 | 0xe0 => Binary::Shl,
                0x99 | 0xb9 | 0xe1 => Binary::Shr,
                _ => Binary::Ushr,
            };
            let op = match spec.opcode {
                0 => None,
                1..=3 => Some(Op::Move(reg(0)?, reg(1)?)),
                0x12..=0x15 => Some(Op::Constant(reg(0)?, literal()?)),
                0xf => Some(Op::Return(reg(0)?)),
                0x10 => Some(Op::ReturnWide(reg(0)?)),
                4..=6 => Some(Op::MoveWide(reg(0)?, reg(1)?)),
                0x16..=0x19 => Some(Op::ConstantWide(
                    reg(0)?,
                    match operands.literal() {
                        Some(RawLiteral::Bits64(bits)) => i64::from_le_bytes(bits.to_le_bytes()),
                        Some(RawLiteral::Signed16(value)) => i64::from(value),
                        _ => {
                            return Err(RequestError::InternalInvariant(
                                "checked wide literal is absent",
                            ));
                        }
                    },
                )),
                0x7f | 0x80 => Some(Op::Unary(
                    reg(0)?,
                    if spec.opcode == 0x7f {
                        Unary::NegFloat
                    } else {
                        Unary::NegDouble
                    },
                    reg(1)?,
                )),
                0x82 | 0x83 | 0x85..=0x8c => {
                    use ScalarType::*;
                    let (from, to) = match spec.opcode {
                        0x82 => (Int, Float),
                        0x83 => (Int, Double),
                        0x85 => (Long, Float),
                        0x86 => (Long, Double),
                        0x87 => (Float, Int),
                        0x88 => (Float, Long),
                        0x89 => (Float, Double),
                        0x8a => (Double, Int),
                        0x8b => (Double, Long),
                        _ => (Double, Float),
                    };
                    Some(Op::Unary(reg(0)?, Unary::Convert { from, to }, reg(1)?))
                }
                0xa6..=0xaf | 0xc6..=0xcf => {
                    let normalized = if spec.opcode >= 0xc6 {
                        spec.opcode - 32
                    } else {
                        spec.opcode
                    };
                    let kind = if normalized <= 0xaa {
                        ScalarType::Float
                    } else {
                        ScalarType::Double
                    };
                    let offset = if kind == ScalarType::Float {
                        normalized - 0xa6
                    } else {
                        normalized - 0xab
                    };
                    let op = binary(0x90 + offset);
                    let (a, b) = if spec.opcode >= 0xc6 {
                        (reg(0)?, reg(1)?)
                    } else {
                        (reg(1)?, reg(2)?)
                    };
                    Some(Op::BinaryFp(reg(0)?, a, op, b, kind))
                }
                0x7d | 0x7e | 0x81 | 0x84 => Some(Op::Unary(
                    reg(0)?,
                    match spec.opcode {
                        0x7d => Unary::NegLong,
                        0x7e => Unary::NotLong,
                        0x81 => Unary::IntToLong,
                        _ => Unary::LongToInt,
                    },
                    reg(1)?,
                )),
                0x9b..=0xa5 => Some(Op::BinaryWide(
                    reg(0)?,
                    reg(1)?,
                    binary(spec.opcode - 11),
                    reg(2)?,
                )),
                0xbb..=0xc5 => Some(Op::BinaryWide(
                    reg(0)?,
                    reg(0)?,
                    binary(spec.opcode - 11),
                    reg(1)?,
                )),
                0x7b | 0x7c | 0x8d..=0x8f => Some(Op::Unary(
                    reg(0)?,
                    match spec.opcode {
                        0x7b => Unary::Neg,
                        0x7c => Unary::Not,
                        0x8d => Unary::Byte,
                        0x8e => Unary::Char,
                        _ => Unary::Short,
                    },
                    reg(1)?,
                )),
                0x90..=0x9a => Some(Op::Binary(
                    reg(0)?,
                    Operand::Register(reg(1)?),
                    binary(spec.opcode),
                    Operand::Register(reg(2)?),
                )),
                0xb0..=0xba => Some(Op::Binary(
                    reg(0)?,
                    Operand::Register(reg(0)?),
                    binary(spec.opcode),
                    Operand::Register(reg(1)?),
                )),
                0xd0..=0xe2 => {
                    let a = Operand::Register(reg(1)?);
                    let b = Operand::Literal(literal()?);
                    let (a, b) = if matches!(spec.opcode, 0xd1 | 0xd9) {
                        (b, a)
                    } else {
                        (a, b)
                    };
                    Some(Op::Binary(reg(0)?, a, binary(spec.opcode), b))
                }
                _ => return Ok(None),
            };
            if let Some(op) = op {
                reserve(&mut ops, 1, budget)?;
                ops.push((pc, op));
            }
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::InternalInvariant(
                "checked scalar extent overflowed",
            ))?;
    }
    if !matches!(ops.last(), Some((_, Op::Return(_) | Op::ReturnWide(_)))) {
        return Err(RequestError::InternalInvariant(
            "closed scalar flow has no terminal",
        ));
    }
    Ok(Some(FlowBody {
        raw,
        ops,
        parameters,
        result,
    }))
}

fn define(
    definitions: &mut Vec<Definition>,
    kind: ScalarType,
    expression: Expression,
    pc: Option<CodeUnitOffset>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ValueId, RequestError> {
    let id = ValueId(u32::try_from(definitions.len()).map_err(|_| RequestError::PlatformCapacity)?);
    reserve(definitions, 1, budget)?;
    definitions.push(Definition {
        kind,
        expression,
        pc,
    });
    Ok(id)
}
#[derive(Clone, Copy)]
enum Word {
    Undefined,
    Low(ValueId),
    High(ValueId),
}
fn read(
    slots: &[Word],
    definitions: &[Definition],
    register: RegisterId,
    wide: bool,
    pc: CodeUnitOffset,
) -> Result<ValueId, RequestError> {
    let index = usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)?;
    let slot = slots.get(index).ok_or(RequestError::InternalInvariant(
        "checked scalar register is out of range",
    ))?;
    match *slot {
        Word::Undefined => Err(RequestError::UndefinedRegister { register, pc }),
        Word::High(_) => Err(RequestError::InvalidWideRead { register, pc }),
        Word::Low(value) => {
            let kind = definitions
                [usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
            .kind;
            if wide {
                if kind.words() != 2
                    || !matches!(slots.get(index.checked_add(1).ok_or(RequestError::PlatformCapacity)?),Some(Word::High(high)) if high.0==value.0)
                {
                    return Err(RequestError::InvalidWideRead { register, pc });
                }
            } else if kind.words() != 1 {
                return Err(RequestError::InvalidWideRead { register, pc });
            }
            Ok(value)
        }
    }
}
fn write(
    slots: &mut [Word],
    register: RegisterId,
    value: ValueId,
    kind: ScalarType,
) -> Result<(), RequestError> {
    let index = usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)?;
    let end = index
        .checked_add(kind.words())
        .ok_or(RequestError::PlatformCapacity)?;
    let destination = slots
        .get_mut(index..end)
        .ok_or(RequestError::InternalInvariant(
            "scalar destination is outside its frame",
        ))?;
    destination[0] = Word::Low(value);
    if kind.words() == 2 {
        destination[1] = Word::High(value);
    }
    Ok(())
}
fn bind<'a>(
    flow: FlowBody<'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<SsaBody<'a>, RequestError> {
    let mut definitions = Vec::new();
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(flow.raw.registers_size()), budget)?;
    for _ in 0..flow.raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(Word::Undefined);
    }
    let mut parameter_register = u32::from(flow.raw.registers_size())
        .checked_sub(u32::from(flow.raw.ins_size()))
        .ok_or(RequestError::InternalInvariant(
            "scalar parameter frame overflowed",
        ))?;
    for (parameter, &kind) in flow.parameters.iter().enumerate() {
        budget.charge_work(1)?;
        let id = define(
            &mut definitions,
            kind,
            Expression::Parameter(
                u32::try_from(parameter).map_err(|_| RequestError::PlatformCapacity)?,
            ),
            None,
            budget,
        )?;
        write(&mut slots, RegisterId(parameter_register), id, kind)?;
        parameter_register = parameter_register
            .checked_add(u32::try_from(kind.words()).map_err(|_| RequestError::PlatformCapacity)?)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    let mut returned = None;
    for &(pc, ref op) in &flow.ops {
        budget.charge_work(1)?;
        let (destination, kind, expression) = match *op {
            Op::Constant(reg, value) => (reg, ScalarType::Bits32, Expression::Constant(value)),
            Op::ConstantWide(reg, value) => {
                (reg, ScalarType::Bits64, Expression::ConstantWide(value))
            }
            Op::Move(reg, source) => (
                reg,
                ScalarType::Bits32,
                Expression::Copy(read(&slots, &definitions, source, false, pc)?),
            ),
            Op::MoveWide(reg, source) => (
                reg,
                ScalarType::Bits64,
                Expression::Copy(read(&slots, &definitions, source, true, pc)?),
            ),
            Op::Unary(reg, op, source) => {
                let (source_kind, kind) = op.signature();
                let wide = source_kind.words() == 2;
                (
                    reg,
                    kind,
                    Expression::Unary(op, read(&slots, &definitions, source, wide, pc)?),
                )
            }
            Op::BinaryFp(reg, a, op, b, kind) => {
                let a = read(&slots, &definitions, a, kind.words() == 2, pc)?;
                let b = read(&slots, &definitions, b, kind.words() == 2, pc)?;
                (reg, kind, Expression::Binary(a, op, b))
            }
            Op::BinaryWide(reg, a, op, b) => {
                let a = read(&slots, &definitions, a, true, pc)?;
                let b = read(
                    &slots,
                    &definitions,
                    b,
                    !matches!(op, Binary::Shl | Binary::Shr | Binary::Ushr),
                    pc,
                )?;
                (reg, ScalarType::Long, Expression::Binary(a, op, b))
            }
            Op::Binary(reg, a, op, b) => {
                let mut value = |operand| -> Result<ValueId, RequestError> {
                    match operand {
                        Operand::Register(reg) => read(&slots, &definitions, reg, false, pc),
                        Operand::Literal(value) => define(
                            &mut definitions,
                            ScalarType::Int,
                            Expression::Constant(value),
                            Some(pc),
                            budget,
                        ),
                    }
                };
                let a = value(a)?;
                let b = value(b)?;
                (reg, ScalarType::Int, Expression::Binary(a, op, b))
            }
            Op::Return(reg) | Op::ReturnWide(reg) => {
                returned = Some(read(
                    &slots,
                    &definitions,
                    reg,
                    matches!(*op, Op::ReturnWide(_)),
                    pc,
                )?);
                continue;
            }
        };
        // Both original operands are bound before an overlapping destination write.
        let id = define(&mut definitions, kind, expression, Some(pc), budget)?;
        write(&mut slots, destination, id, kind)?;
    }
    Ok(SsaBody {
        flow,
        definitions,
        returned: returned.ok_or(RequestError::InternalInvariant(
            "scalar return lost its definition",
        ))?,
    })
}
