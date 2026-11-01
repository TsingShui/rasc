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
    PrimitiveArray(crate::p5_types::PrimitiveArray),
    ObjectArray,
    Nominal(crate::TypeId),
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
            Self::PrimitiveArray(kind) => Kind::PrimitiveArray(kind),
            Self::ObjectArray => Kind::ObjectArray,
            Self::Nominal(id) => Kind::Nominal(id),
        }
    }
    pub(crate) fn range(self) -> Option<crate::p5_types::IntRange> {
        use crate::p5_types::IntRange;
        match self {
            Self::Int => Some(IntRange::ALL),
            Self::Boolean => Some(IntRange::BOOLEAN),
            Self::Narrow(kind) => Some(kind.range()),
            Self::Object
            | Self::String
            | Self::Float
            | Self::PrimitiveArray(_)
            | Self::ObjectArray
            | Self::Nominal(_) => None,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ResultType {
    Void,
    Value(DeclarationType),
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
#[derive(Clone, Copy)]
pub(crate) enum InvokeResult {
    Void,
    Word(DeclarationType),
    Reference(Option<DeclarationType>),
    Wide,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum InvokeKind {
    Direct,
    Super,
    Static,
    Virtual,
    Interface,
}
impl InvokeKind {
    pub(crate) fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode {
            0x71 | 0x77 => Some(Self::Static),
            0x70 | 0x76 => Some(Self::Direct),
            0x6f | 0x75 => Some(Self::Super),
            0x6e | 0x74 => Some(Self::Virtual),
            0x72 | 0x78 => Some(Self::Interface),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CallTarget {
    Method,
    Constructor,
}
pub(crate) struct CallSite {
    pub target: CallTarget,
    pub owner: crate::TypeId,
    pub kind: InvokeKind,
    pub reference: crate::MethodId,
    pub result: InvokeResult,
    pub registers: Vec<RegisterId>,
    pub pc: CodeUnitOffset,
    pub result_pc: Option<CodeUnitOffset>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct StringLiteralId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CastId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct EmptyFillId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ArrayCreationId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ObjectCreationId(pub u32);
pub(crate) struct ObjectCreation {
    pub reference: crate::TypeId,
}
pub(crate) struct ArrayCreation {
    pub reference: crate::TypeId,
    pub component: crate::p5_types::PrimitiveArray,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct FreshFillId(pub u32);
pub(crate) struct FreshFillSite {
    pub payload: CodeUnitOffset,
    pub width: usize,
    pub count: u32,
}
pub(crate) struct EmptyFillSite {
    pub payload: CodeUnitOffset,
    pub width: usize,
}
pub(crate) struct CastSite {
    pub reference: crate::TypeId,
    pub kind: crate::p5_types::BuiltinReference,
}
pub(crate) struct StringLiteral<'a> {
    pub reference: crate::StringId,
    pub value: crate::input::DexString<'a>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ArrayElement {
    Int,
    Object,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Op {
    NewInstance(RegisterId, ObjectCreationId),
    Initialize(CallId),
    NewArray(RegisterId, RegisterId, ArrayCreationId),
    FreshFill(RegisterId, FreshFillId),
    EmptyFill(RegisterId, EmptyFillId),
    Cast(RegisterId, CastId),
    String(RegisterId, StringLiteralId),
    FieldLoad(RegisterId, RegisterId, crate::FieldId),
    FieldStore(RegisterId, RegisterId, crate::FieldId),
    ArrayLength(RegisterId, RegisterId),
    ArrayLoad(RegisterId, RegisterId, RegisterId, ArrayElement),
    ArrayStore(RegisterId, RegisterId, RegisterId, ArrayElement),
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
        let conservative = matches!(self, Self::Call(_, _) | Self::Initialize(_));
        Effects {
            heap_read: conservative || matches!(self, Self::ArrayLoad(..) | Self::FieldLoad(..)),
            heap_write: conservative
                || matches!(
                    self,
                    Self::ArrayStore(..)
                        | Self::FieldStore(..)
                        | Self::NewInstance(..)
                        | Self::NewArray(..)
                        | Self::FreshFill(..)
                ),
            may_throw: conservative
                || matches!(
                    self,
                    Self::NewInstance(..)
                        | Self::NewArray(..)
                        | Self::FreshFill(..)
                        | Self::EmptyFill(..)
                        | Self::Cast(..)
                        | Self::String(..)
                        | Self::ArrayLength(..)
                        | Self::ArrayLoad(..)
                        | Self::ArrayStore(..)
                        | Self::FieldLoad(..)
                        | Self::FieldStore(..)
                ),
            call: conservative,
            class_init: conservative || matches!(self, Self::NewInstance(..)),
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
#[derive(Clone, Copy)]
pub(crate) enum MethodFrame {
    Static,
    Instance(crate::TypeId),
    Constructor(crate::TypeId),
}
impl MethodFrame {
    pub(crate) fn implicit_words(self) -> usize {
        match self {
            Self::Static => 0,
            Self::Instance(_) | Self::Constructor(_) => 1,
        }
    }
}
pub(crate) struct FlowBody<'a> {
    pub object_creations: Vec<ObjectCreation>,
    pub array_creations: Vec<ArrayCreation>,
    pub fresh_fills: Vec<FreshFillSite>,
    pub empty_fills: Vec<EmptyFillSite>,
    pub casts: Vec<CastSite>,
    pub strings: Vec<StringLiteral<'a>>,
    pub frame: MethodFrame,
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
        Op::NewInstance(r, _)
        | Op::NewArray(r, _, _)
        | Op::Cast(r, _)
        | Op::String(r, _)
        | Op::Constant(r, _)
        | Op::Move(r, _)
        | Op::Binary(r, _, _, _)
        | Op::Compare(r, _, _, _)
        | Op::ArrayLength(r, _)
        | Op::ArrayLoad(r, _, _, _)
        | Op::FieldLoad(r, _, _) => Some(r),
        Op::Nop
        | Op::ArrayStore(..)
        | Op::FieldStore(..)
        | Op::EmptyFill(..)
        | Op::FreshFill(..)
        | Op::Initialize(..) => None,
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
            Self::Call(id, _) | Self::Initialize(id) => {
                let call = calls.get(ix(id.0)?).ok_or(RequestError::InternalInvariant(
                    "operation has a foreign call identity",
                ))?;
                return Ok(Reads::Call(call.registers.iter()));
            }
            Self::NewArray(_, length, _) => [Some(length), None, None],
            Self::FreshFill(register, _) => [Some(register), None, None],
            Self::EmptyFill(register, _) => [Some(register), None, None],
            Self::Cast(register, _) => [Some(register), None, None],
            Self::FieldLoad(_, receiver, _) => [Some(receiver), None, None],
            Self::FieldStore(receiver, value, _) => [Some(receiver), Some(value), None],
            Self::ArrayLength(_, array) => [Some(array), None, None],
            Self::ArrayLoad(_, array, index, _) => [Some(array), Some(index), None],
            Self::ArrayStore(array, index, value, _) => [Some(array), Some(index), Some(value)],
            Self::Compare(_, left, _, right) => [Some(left), Some(right), None],
            Self::Binary(_, left, _, Operand::Register(right)) => [Some(left), Some(right), None],
            Self::Binary(_, left, _, Operand::Literal(_)) | Self::Move(_, left) => {
                [Some(left), None, None]
            }
            Self::NewInstance(..) | Self::String(..) | Self::Constant(_, _) | Self::Nop => {
                [None, None, None]
            }
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
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<FlowBody<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 0x10008 == 0x10008 || !raw.try_byte_range().as_range().is_empty() {
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
            } else if let Some(kind) = crate::p5_types::PrimitiveArray::parse(
                input.declaration().prototype.return_type,
                budget,
            )? {
                DeclarationType::PrimitiveArray(kind)
            } else {
                let mut units = input.declaration().prototype.return_type.reader()?;
                if units.next(budget)? != Some(76) {
                    return Ok(None);
                }
                DeclarationType::Nominal(input.declaration().prototype.return_type_idx)
            },
        )
    };
    let frame = if input.access_flags() & 8 != 0 {
        MethodFrame::Static
    } else {
        let owner = input
            .method_reference(input.method_id(), budget)?
            .owner_type;
        if input.access_flags() & 0x10000 != 0 {
            MethodFrame::Constructor(owner)
        } else {
            MethodFrame::Instance(owner)
        }
    };
    let mut parameters = Vec::new();
    if let MethodFrame::Instance(owner) | MethodFrame::Constructor(owner) = frame {
        push(&mut parameters, DeclarationType::Nominal(owner), budget)?;
    }
    let mut supported = true;
    input.declaration().prototype.visit_parameter_type_ids(
        budget,
        |id, name, budget| -> Result<(), RequestError> {
            let kind = if name.matches_with(&[73], budget)? {
                DeclarationType::Int
            } else if name.matches_with(&[90], budget)? {
                DeclarationType::Boolean
            } else if let Some(kind) = crate::p5_types::PrimitiveArray::parse(name, budget)? {
                DeclarationType::PrimitiveArray(kind)
            } else if name.matches_with(
                &[
                    91, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116,
                    59,
                ],
                budget,
            )? {
                DeclarationType::ObjectArray
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
                let mut units = name.reader()?;
                if units.next(budget)? != Some(76) {
                    supported = false;
                    return Ok(());
                }
                DeclarationType::Nominal(id)
            };
            push(&mut parameters, kind, budget)
        },
    )?;
    if !supported {
        return Ok(None);
    }
    let mut instructions = Vec::new();
    let mut calls = Vec::new();
    let mut strings = Vec::new();
    let mut casts = Vec::new();
    let mut empty_fills = Vec::new();
    let mut array_creations = Vec::new();
    let mut object_creations = Vec::new();
    let mut fresh_fills = Vec::new();
    let mut raw_switches: Vec<(Vec<(i32, CodeUnitOffset)>, CodeUnitOffset)> = Vec::new();
    let mut leaders = Vec::new();
    push(&mut leaders, CodeUnitOffset(0), budget)?;
    let mut pc = CodeUnitOffset(0);
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
                0x52 | 0x59 => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant(
                            "field operation lost reference",
                        ));
                    };
                    let crate::raw::OperandReference::Field(id) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "field operation changed reference domain",
                        ));
                    };
                    if !input
                        .field_reference(id, budget)?
                        .declaration
                        .field_type
                        .matches_with(&[73], budget)?
                    {
                        return Ok(None);
                    }
                    if spec.opcode == 0x52 {
                        Decoded::Op(Op::FieldLoad(reg(0)?, reg(1)?, id))
                    } else {
                        Decoded::Op(Op::FieldStore(reg(1)?, reg(0)?, id))
                    }
                }
                0x22 => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant("new-instance lost type"));
                    };
                    let crate::raw::OperandReference::Type(reference) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "new-instance changed reference domain",
                        ));
                    };
                    let id = ObjectCreationId(
                        u32::try_from(object_creations.len())
                            .map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(&mut object_creations, ObjectCreation { reference }, budget)?;
                    Decoded::Op(Op::NewInstance(reg(0)?, id))
                }
                0x23 => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant(
                            "array allocation lost type",
                        ));
                    };
                    let crate::raw::OperandReference::Type(reference) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "array allocation changed reference domain",
                        ));
                    };
                    let Some(component) = crate::p5_types::PrimitiveArray::parse(
                        input.type_name(reference, budget)?,
                        budget,
                    )?
                    else {
                        return Ok(None);
                    };
                    let id = ArrayCreationId(
                        u32::try_from(array_creations.len())
                            .map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(
                        &mut array_creations,
                        ArrayCreation {
                            reference,
                            component,
                        },
                        budget,
                    )?;
                    Decoded::Op(Op::NewArray(reg(0)?, reg(1)?, id))
                }
                0x26 => {
                    let delta = operands
                        .offset()
                        .ok_or(RequestError::InternalInvariant("fill lost payload offset"))?;
                    let payload = CodeUnitOffset(pc.0.checked_add_signed(delta.0).ok_or(
                        RequestError::InternalInvariant("checked fill target overflowed"),
                    )?);
                    let data = instruction_layout(raw.instructions(), payload, raw.version())
                        .map_err(crate::input::MethodError::Decode)?;
                    let LayoutKind::ArrayData {
                        element_width,
                        element_count,
                    } = data.kind()
                    else {
                        return Err(RequestError::InternalInvariant("fill changed payload kind"));
                    };
                    if element_count != 0 {
                        let id = FreshFillId(
                            u32::try_from(fresh_fills.len())
                                .map_err(|_| RequestError::PlatformCapacity)?,
                        );
                        push(
                            &mut fresh_fills,
                            FreshFillSite {
                                payload,
                                width: element_width.0,
                                count: element_count,
                            },
                            budget,
                        )?;
                        Decoded::Op(Op::FreshFill(reg(0)?, id))
                    } else {
                        let id = EmptyFillId(
                            u32::try_from(empty_fills.len())
                                .map_err(|_| RequestError::PlatformCapacity)?,
                        );
                        push(
                            &mut empty_fills,
                            EmptyFillSite {
                                payload,
                                width: element_width.0,
                            },
                            budget,
                        )?;
                        Decoded::Op(Op::EmptyFill(reg(0)?, id))
                    }
                }
                0x1f => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant("cast lost target"));
                    };
                    let crate::raw::OperandReference::Type(reference) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "cast changed reference domain",
                        ));
                    };
                    let descriptor = input.type_name(reference, budget)?;
                    let mut target = None;
                    for kind in [
                        crate::p5_types::BuiltinReference::Object,
                        crate::p5_types::BuiltinReference::String,
                    ] {
                        if descriptor.matches_with(kind.descriptor(), budget)? {
                            target = Some(kind);
                            break;
                        }
                    }
                    let Some(kind) = target else {
                        return Ok(None);
                    };
                    let id = CastId(
                        u32::try_from(casts.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(&mut casts, CastSite { reference, kind }, budget)?;
                    Decoded::Op(Op::Cast(reg(0)?, id))
                }
                0x1a | 0x1b => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant(
                            "string literal lost reference",
                        ));
                    };
                    let crate::raw::OperandReference::String(reference) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "literal changed reference domain",
                        ));
                    };
                    let id = StringLiteralId(
                        u32::try_from(strings.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(
                        &mut strings,
                        StringLiteral {
                            reference,
                            value: input.string_literal(reference, budget)?,
                        },
                        budget,
                    )?;
                    Decoded::Op(Op::String(reg(0)?, id))
                }
                0x21 => Decoded::Op(Op::ArrayLength(reg(0)?, reg(1)?)),
                0x44 | 0x46 => Decoded::Op(Op::ArrayLoad(
                    reg(0)?,
                    reg(1)?,
                    reg(2)?,
                    if spec.opcode == 0x44 {
                        ArrayElement::Int
                    } else {
                        ArrayElement::Object
                    },
                )),
                0x4b | 0x4d => Decoded::Op(Op::ArrayStore(
                    reg(1)?,
                    reg(2)?,
                    reg(0)?,
                    if spec.opcode == 0x4b {
                        ArrayElement::Int
                    } else {
                        ArrayElement::Object
                    },
                )),
                0xe => Decoded::ReturnVoid,
                0xf | 0x11 => Decoded::Return(reg(0)?),
                0x2b | 0x2c => {
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
                0x6e..=0x72 | 0x74..=0x78 => {
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
                    let method = input.method_reference(reference, budget)?;
                    let constructor = method
                        .declaration
                        .name
                        .matches_with(&[60, 105, 110, 105, 116, 62], budget)?;
                    let mut one_word = true;
                    method.declaration.prototype.visit_parameter_types(
                        budget,
                        |descriptor, budget| {
                            let mut units = descriptor.reader()?;
                            if matches!(units.next(budget)?, Some(74 | 68)) {
                                one_word = false;
                            }
                            Ok::<(), RequestError>(())
                        },
                    )?;
                    if !one_word {
                        return Ok(None);
                    }
                    let descriptor = method.declaration.prototype.return_type;
                    let mut units = descriptor.reader()?;
                    let result = match units.next(budget)? {
                        Some(86) => InvokeResult::Void,
                        Some(73) => InvokeResult::Word(DeclarationType::Int),
                        Some(90) => InvokeResult::Word(DeclarationType::Boolean),
                        Some(66) => InvokeResult::Word(DeclarationType::Narrow(
                            crate::p5_types::Narrow::Byte,
                        )),
                        Some(83) => InvokeResult::Word(DeclarationType::Narrow(
                            crate::p5_types::Narrow::Short,
                        )),
                        Some(67) => InvokeResult::Word(DeclarationType::Narrow(
                            crate::p5_types::Narrow::Char,
                        )),
                        Some(70) => InvokeResult::Word(DeclarationType::Float),
                        Some(74 | 68) => InvokeResult::Wide,
                        Some(76 | 91) => {
                            let kind = if descriptor.matches_with(
                                crate::p5_types::BuiltinReference::Object.descriptor(),
                                budget,
                            )? {
                                Some(DeclarationType::Object)
                            } else if descriptor.matches_with(
                                crate::p5_types::BuiltinReference::String.descriptor(),
                                budget,
                            )? {
                                Some(DeclarationType::String)
                            } else {
                                let mut units = descriptor.reader()?;
                                if units.next(budget)? == Some(76) {
                                    Some(DeclarationType::Nominal(
                                        method.declaration.prototype.return_type_idx,
                                    ))
                                } else {
                                    None
                                }
                            };
                            InvokeResult::Reference(kind)
                        }
                        _ => {
                            return Err(RequestError::InternalInvariant(
                                "checked invoke lost return descriptor",
                            ));
                        }
                    };
                    let mut registers = Vec::new();
                    for register in operands.registers().iter() {
                        budget.charge_work(1)?;
                        push(&mut registers, register, budget)?;
                    }
                    let mut destination = None;
                    let mut result_pc = None;
                    if next.0 < raw.code_units().0 {
                        let result_layout =
                            instruction_layout(raw.instructions(), next, raw.version())
                                .map_err(crate::input::MethodError::Decode)?;
                        if matches!(result_layout.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,0x0a|0x0c))
                        {
                            let valid = match result {
                                InvokeResult::Word(_) => {
                                    matches!(result_layout.kind(), LayoutKind::Instruction(spec) if spec.opcode == 0x0a)
                                }
                                InvokeResult::Reference(Some(_)) => {
                                    matches!(result_layout.kind(), LayoutKind::Instruction(spec) if spec.opcode == 0x0c)
                                }
                                InvokeResult::Reference(None) | InvokeResult::Wide => {
                                    return Ok(None);
                                }
                                InvokeResult::Void => false,
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
                            target: if constructor {
                                CallTarget::Constructor
                            } else {
                                CallTarget::Method
                            },
                            owner: method.owner_type,
                            kind: InvokeKind::from_opcode(spec.opcode)
                                .ok_or(RequestError::InternalInvariant("invoke lost its kind"))?,
                            reference,
                            result,
                            registers,
                            pc,
                            result_pc,
                        },
                        budget,
                    )?;
                    if constructor {
                        Decoded::Op(Op::Initialize(call))
                    } else {
                        Decoded::Op(Op::Call(call, destination))
                    }
                }
                0x28..=0x2a => {
                    let to = target()?;
                    push(&mut leaders, to, budget)?;
                    Decoded::Jump(to)
                }
                0x32..=0x3d => {
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
        object_creations,
        array_creations,
        fresh_fills,
        empty_fills,
        casts,
        strings,
        frame,
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
