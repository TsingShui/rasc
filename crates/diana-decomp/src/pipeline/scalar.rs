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
}
#[derive(Clone, Copy)]
enum Operand {
    Register(RegisterId),
    Literal(i32),
}
enum Op {
    Constant(RegisterId, i32),
    Move(RegisterId, RegisterId),
    Unary(RegisterId, Unary, RegisterId),
    Binary(RegisterId, Operand, Binary, Operand),
    Return(RegisterId),
}
struct FlowBody<'a> {
    raw: RawMethodView<'a>,
    ops: Vec<(CodeUnitOffset, Op)>,
    parameters: u32,
}
pub(crate) enum Expression {
    Parameter(u32),
    Constant(i32),
    Copy(ValueId),
    Unary(Unary, ValueId),
    Binary(ValueId, Binary, ValueId),
}
pub(crate) struct Definition {
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
#[derive(Clone, Copy)]
enum ScalarType {
    Int,
}
struct TypedBody<'a> {
    resolved: ResolvedBody<'a>,
    types: Vec<ScalarType>,
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
    pub parameters: u32,
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
    let mut types = Vec::new();
    for (index, definition) in resolved.ssa.definitions.iter().enumerate() {
        budget.charge_work(1)?;
        let check = |id: ValueId| -> Result<(), RequestError> {
            if usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)? >= index {
                return Err(RequestError::InternalInvariant(
                    "scalar use is not dominated by its definition",
                ));
            }
            Ok(())
        };
        match definition.expression {
            Expression::Copy(id) | Expression::Unary(_, id) => check(id)?,
            Expression::Binary(a, _, b) => {
                check(a)?;
                check(b)?;
            }
            _ => {}
        }
        reserve(&mut types, 1, budget)?;
        types.push(ScalarType::Int);
    }
    let typed = TypedBody { resolved, types };
    let mut locals = Vec::new();
    for index in 0..typed.types.len() {
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
    if input.access_flags() & 8 == 0
        || !raw.try_byte_range().as_range().is_empty()
        || !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[u16::from(b'I')], budget)?
    {
        return Ok(None);
    }
    let mut parameters = 0u32;
    let mut supported = true;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            supported &= name.matches_with(&[u16::from(b'I')], budget)?;
            parameters = parameters
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
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
    if !matches!(ops.last(), Some((_, Op::Return(_)))) {
        return Err(RequestError::InternalInvariant(
            "closed scalar flow has no terminal",
        ));
    }
    Ok(Some(FlowBody {
        raw,
        ops,
        parameters,
    }))
}

fn define(
    definitions: &mut Vec<Definition>,
    expression: Expression,
    pc: Option<CodeUnitOffset>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ValueId, RequestError> {
    let id = ValueId(u32::try_from(definitions.len()).map_err(|_| RequestError::PlatformCapacity)?);
    reserve(definitions, 1, budget)?;
    definitions.push(Definition { expression, pc });
    Ok(id)
}
fn read(
    slots: &[Option<ValueId>],
    register: RegisterId,
    pc: CodeUnitOffset,
) -> Result<ValueId, RequestError> {
    let index = usize::try_from(register.0).map_err(|_| RequestError::PlatformCapacity)?;
    slots
        .get(index)
        .ok_or(RequestError::InternalInvariant(
            "checked scalar register is out of range",
        ))?
        .ok_or(RequestError::UndefinedRegister { register, pc })
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
        slots.push(None);
    }
    let start = u32::from(flow.raw.registers_size())
        .checked_sub(flow.parameters)
        .ok_or(RequestError::InternalInvariant(
            "scalar parameter frame overflowed",
        ))?;
    for parameter in 0..flow.parameters {
        budget.charge_work(1)?;
        let id = define(
            &mut definitions,
            Expression::Parameter(parameter),
            None,
            budget,
        )?;
        let index = usize::try_from(
            start
                .checked_add(parameter)
                .ok_or(RequestError::PlatformCapacity)?,
        )
        .map_err(|_| RequestError::PlatformCapacity)?;
        *slots.get_mut(index).ok_or(RequestError::InternalInvariant(
            "scalar parameter is outside its frame",
        ))? = Some(id);
    }
    let mut returned = None;
    for &(pc, ref op) in &flow.ops {
        budget.charge_work(1)?;
        let (destination, expression) = match *op {
            Op::Constant(reg, value) => (reg, Expression::Constant(value)),
            Op::Move(reg, source) => (reg, Expression::Copy(read(&slots, source, pc)?)),
            Op::Unary(reg, kind, source) => {
                (reg, Expression::Unary(kind, read(&slots, source, pc)?))
            }
            Op::Binary(reg, a, kind, b) => {
                let mut value = |operand| -> Result<ValueId, RequestError> {
                    match operand {
                        Operand::Register(reg) => read(&slots, reg, pc),
                        Operand::Literal(value) => define(
                            &mut definitions,
                            Expression::Constant(value),
                            Some(pc),
                            budget,
                        ),
                    }
                };
                let a = value(a)?;
                let b = value(b)?;
                (reg, Expression::Binary(a, kind, b))
            }
            Op::Return(reg) => {
                returned = Some(read(&slots, reg, pc)?);
                continue;
            }
        };
        let id = define(&mut definitions, expression, Some(pc), budget)?;
        let index = usize::try_from(destination.0).map_err(|_| RequestError::PlatformCapacity)?;
        *slots.get_mut(index).ok_or(RequestError::InternalInvariant(
            "scalar destination is outside its frame",
        ))? = Some(id);
    }
    Ok(SsaBody {
        flow,
        definitions,
        returned: returned.ok_or(RequestError::InternalInvariant(
            "scalar return lost its definition",
        ))?,
    })
}
