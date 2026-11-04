use crate::input::{MethodSource, RawMethodView};
use crate::raw::{LayoutKind, RawLiteral};
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RegisterId, RequestError, RunContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BlockId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EdgeId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum DeclarationType {
    Long,
    Double,
    Int,
    Boolean,
    Object,
    String,
    Float,
    PrimitiveArray(crate::substrate::primitive_types::PrimitiveArray),
    ObjectArray,
    Nominal(crate::TypeId),
    ReferenceArray(crate::input::TypeProjection),
    ProjectedNominal(crate::input::TypeProjection),
    Narrow(crate::substrate::primitive_types::Narrow),
}
impl<'a> FlowBody<'a> {
    pub(crate) fn into_data(self) -> FlowBodyData<'a> {
        self.0
    }
}
impl crate::substrate::primitive_types::PrimitiveArray {
    pub(crate) fn value_type(self) -> crate::p2_flow::normal::DeclarationType {
        use crate::p2_flow::normal::DeclarationType;
        match self {
            Self::Boolean => DeclarationType::Boolean,
            Self::Byte => DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Byte),
            Self::Short => {
                DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Short)
            }
            Self::Char => DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Char),
            Self::Int => DeclarationType::Int,
            Self::Float => DeclarationType::Float,
            Self::Long => DeclarationType::Long,
            Self::Double => DeclarationType::Double,
        }
    }
}
impl DeclarationType {
    pub(crate) fn field_offset(self) -> u8 {
        match self {
            Self::Int | Self::Float => 0,
            Self::Long | Self::Double => 1,
            Self::Boolean => 3,
            Self::Narrow(crate::substrate::primitive_types::Narrow::Byte) => 4,
            Self::Narrow(crate::substrate::primitive_types::Narrow::Char) => 5,
            Self::Narrow(crate::substrate::primitive_types::Narrow::Short) => 6,
            Self::Object
            | Self::String
            | Self::PrimitiveArray(_)
            | Self::ObjectArray
            | Self::Nominal(_)
            | Self::ReferenceArray(_)
            | Self::ProjectedNominal(_) => 2,
        }
    }
    pub(crate) fn width(self) -> ValueWidth {
        if matches!(self, Self::Long | Self::Double) {
            ValueWidth::Two
        } else {
            ValueWidth::One
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ResultType {
    Void,
    Value(DeclarationType),
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ValueWidth {
    One,
    Two,
}
impl ValueWidth {
    pub(crate) fn words(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}
impl ResultType {
    pub(crate) fn width(self) -> ValueWidth {
        match self {
            Self::Void => ValueWidth::One,
            Self::Value(kind) => kind.width(),
        }
    }
}
pub(crate) use crate::substrate::operators::Arithmetic;
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
    Wide(DeclarationType),
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
    pub formals: Vec<DeclarationType>,
    pub target: CallTarget,
    pub owner: crate::TypeId,
    pub kind: InvokeKind,
    pub reference: crate::MethodId,
    pub result: InvokeResult,
    pub registers: Vec<RegisterId>,
    pub arguments: Vec<RegisterId>,
    pub widths: Vec<ValueWidth>,
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
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ArrayKind {
    Primitive(crate::substrate::primitive_types::PrimitiveArray),
    Object,
    Reference(crate::input::TypeProjection),
}
impl ArrayKind {
    pub(crate) fn parse(
        input: &MethodSource<'_, '_>,
        id: crate::TypeId,
        budget: &mut RunContext<'_>,
    ) -> Result<Option<Self>, RequestError> {
        Ok(
            match declaration_for_projection(input, input.type_projection(id, budget)?, budget)? {
                Some(DeclarationType::PrimitiveArray(kind)) => Some(Self::Primitive(kind)),
                Some(DeclarationType::ObjectArray) => Some(Self::Object),
                Some(DeclarationType::ReferenceArray(key)) => Some(Self::Reference(key)),
                _ => None,
            },
        )
    }
    pub(crate) fn element_type(
        self,
        input: &MethodSource<'_, '_>,
        budget: &mut RunContext<'_>,
    ) -> Result<DeclarationType, RequestError> {
        match self {
            Self::Primitive(kind) => Ok(kind.value_type()),
            Self::Object => Ok(DeclarationType::Object),
            Self::Reference(key) => {
                declaration_for_projection(input, input.component_projection(key, budget)?, budget)?
                    .ok_or(RequestError::InternalInvariant(
                        "array lost a component type",
                    ))
            }
        }
    }
    pub(crate) fn object_descriptor() -> &'static [u16] {
        &[
            91, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
        ]
    }
    pub(crate) fn declaration(self) -> DeclarationType {
        match self {
            Self::Primitive(kind) => DeclarationType::PrimitiveArray(kind),
            Self::Object => DeclarationType::ObjectArray,
            Self::Reference(key) => DeclarationType::ReferenceArray(key),
        }
    }
}
pub(crate) struct ArrayCreation {
    pub reference: crate::TypeId,
    pub component: ArrayKind,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FilledArrayId(pub u32);
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum FilledResult {
    Discarded,
    Stored {
        register: RegisterId,
        pc: CodeUnitOffset,
    },
}
pub(crate) struct FilledArraySite {
    pub reference: crate::TypeId,
    pub component: ArrayKind,
    pub registers: Vec<RegisterId>,
    pub pc: CodeUnitOffset,
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
    pub kind: DeclarationType,
}
pub(crate) struct StringLiteral<'a> {
    pub reference: crate::StringId,
    pub value: crate::input::DexString<'a>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ArrayElement {
    Int,
    Wide,
    Object,
    Boolean,
    Byte,
    Char,
    Short,
}
impl ArrayElement {
    pub(crate) fn offset(self) -> u8 {
        match self {
            Self::Int => 0,
            Self::Wide => 1,
            Self::Object => 2,
            Self::Boolean => 3,
            Self::Byte => 4,
            Self::Char => 5,
            Self::Short => 6,
        }
    }
    pub(crate) fn accepts(self, kind: DeclarationType) -> bool {
        match (self, kind) {
            (Self::Object, DeclarationType::Object) => true,
            _ => kind.field_offset() == self.offset(),
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Op {
    Monitor(MonitorKind, RegisterId),
    Class(RegisterId, crate::TypeId),
    Exception(RegisterId),
    ConstantWide(RegisterId, i64),
    MoveWide(RegisterId, RegisterId),
    LongBinary(RegisterId, RegisterId, Arithmetic, RegisterId),
    DoubleBinary(RegisterId, RegisterId, Arithmetic, RegisterId),
    FloatBinary(RegisterId, RegisterId, Arithmetic, RegisterId),
    Unary(RegisterId, crate::p2_flow::primitives::Unary, RegisterId),
    ReverseSubtract(RegisterId, i32, RegisterId),
    NewInstance(RegisterId, ObjectCreationId),
    Initialize(CallId),
    NewArray(RegisterId, RegisterId, ArrayCreationId),
    FilledArray(FilledArrayId, FilledResult),
    FreshFill(RegisterId, FreshFillId),
    EmptyFill(RegisterId, EmptyFillId),
    Cast(RegisterId, CastId),
    InstanceOf(RegisterId, RegisterId, crate::TypeId, DeclarationType),
    String(RegisterId, StringLiteralId),
    FieldLoad(RegisterId, RegisterId, crate::FieldId, DeclarationType),
    FieldStore(RegisterId, RegisterId, crate::FieldId, DeclarationType),
    StaticLoad(RegisterId, crate::FieldId, DeclarationType),
    StaticStore(RegisterId, crate::FieldId, DeclarationType),
    ArrayLength(RegisterId, RegisterId),
    ArrayLoad(RegisterId, RegisterId, RegisterId, ArrayElement),
    ArrayStore(RegisterId, RegisterId, RegisterId, ArrayElement),
    Call(CallId, Option<RegisterId>),
    Compare(
        RegisterId,
        RegisterId,
        crate::p2_flow::primitives::Compare,
        RegisterId,
    ),
    Binary(RegisterId, RegisterId, Arithmetic, Operand),
    Constant(RegisterId, i32),
    Move(RegisterId, RegisterId),
    Nop,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum MonitorKind {
    Enter,
    Exit,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum MethodMonitor {
    DeclaringClass,
    This,
}
#[derive(Clone, Copy)]
pub(crate) struct SwitchId(pub u32);
pub(crate) struct Switch {
    pub cases: Vec<(i32, BlockId)>,
    pub default: BlockId,
}
#[derive(Clone, Copy)]
pub(crate) enum Terminal {
    Throw(RegisterId),
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
    pub monitor: bool,
    pub heap_read: bool,
    pub heap_write: bool,
    pub may_throw: bool,
    pub call: bool,
    pub class_init: bool,
}
impl Effects {
    pub(crate) fn observable(self) -> bool {
        self.monitor
            || self.heap_read
            || self.heap_write
            || self.may_throw
            || self.call
            || self.class_init
    }
}
impl Op {
    pub(crate) fn result_width(self, calls: &[CallSite]) -> Result<ValueWidth, RequestError> {
        Ok(match self {
            Self::Call(id, _)
                if matches!(
                    calls
                        .get(ix(id.0)?)
                        .ok_or(RequestError::InternalInvariant("call lost result width"))?
                        .result,
                    InvokeResult::Wide(_)
                ) =>
            {
                ValueWidth::Two
            }
            Self::ArrayLoad(_, _, _, ArrayElement::Wide)
            | Self::ConstantWide(..)
            | Self::MoveWide(..)
            | Self::LongBinary(..)
            | Self::DoubleBinary(..) => ValueWidth::Two,
            Self::FieldLoad(_, _, _, kind) | Self::StaticLoad(_, _, kind)
                if kind.width() == ValueWidth::Two =>
            {
                ValueWidth::Two
            }
            Self::Unary(_, unary, _) if unary.signature().1.words() == 2 => ValueWidth::Two,
            _ => ValueWidth::One,
        })
    }
    pub(crate) fn read_width(
        self,
        index: usize,
        calls: &[CallSite],
    ) -> Result<ValueWidth, RequestError> {
        Ok(match self {
            Self::Call(id, _) | Self::Initialize(id) => *calls
                .get(ix(id.0)?)
                .and_then(|site| site.widths.get(index))
                .ok_or(RequestError::InternalInvariant("call lost argument width"))?,
            Self::ArrayStore(_, _, _, ArrayElement::Wide) if index == 2 => ValueWidth::Two,
            Self::FieldStore(_, _, _, kind) if index == 1 && kind.width() == ValueWidth::Two => {
                ValueWidth::Two
            }
            Self::StaticStore(_, _, kind) if kind.width() == ValueWidth::Two => ValueWidth::Two,
            Self::MoveWide(..) | Self::DoubleBinary(..) => ValueWidth::Two,
            Self::LongBinary(_, _, Arithmetic::Shl | Arithmetic::Shr | Arithmetic::Ushr, _)
                if index == 1 =>
            {
                ValueWidth::One
            }
            Self::LongBinary(..) => ValueWidth::Two,
            Self::Unary(_, unary, _) if unary.signature().0.words() == 2 => ValueWidth::Two,
            Self::Compare(_, _, compare, _) if compare.operand().words() == 2 => ValueWidth::Two,
            _ => ValueWidth::One,
        })
    }
    pub(crate) fn effects(self) -> Effects {
        let conservative = matches!(self, Self::Call(_, _) | Self::Initialize(_));
        let static_access = matches!(self, Self::StaticLoad(..) | Self::StaticStore(..));
        Effects {
            monitor: matches!(self, Self::Monitor(..)),
            heap_read: conservative
                || static_access
                || matches!(self, Self::ArrayLoad(..) | Self::FieldLoad(..)),
            heap_write: conservative
                || static_access
                || matches!(
                    self,
                    Self::ArrayStore(..)
                        | Self::FieldStore(..)
                        | Self::NewInstance(..)
                        | Self::FilledArray(..)
                        | Self::NewArray(..)
                        | Self::FreshFill(..)
                ),
            may_throw: conservative
                || matches!(
                    self,
                    Self::Binary(_, _, Arithmetic::Div | Arithmetic::Rem, _)
                        | Self::LongBinary(_, _, Arithmetic::Div | Arithmetic::Rem, _)
                )
                || matches!(
                    self,
                    Self::Monitor(..)
                        | Self::Class(..)
                        | Self::NewInstance(..)
                        | Self::FilledArray(..)
                        | Self::NewArray(..)
                        | Self::FreshFill(..)
                        | Self::EmptyFill(..)
                        | Self::Cast(..)
                        | Self::InstanceOf(..)
                        | Self::String(..)
                        | Self::ArrayLength(..)
                        | Self::ArrayLoad(..)
                        | Self::ArrayStore(..)
                        | Self::FieldLoad(..)
                        | Self::FieldStore(..)
                        | Self::StaticLoad(..)
                        | Self::StaticStore(..)
                ),
            call: conservative,
            class_init: conservative || static_access || matches!(self, Self::NewInstance(..)),
        }
    }
}
#[derive(Clone, Copy)]
enum Decoded {
    Throw(RegisterId),
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
crate::substrate::checked::checked_product!(FlowBody, FlowBodyData);
pub(crate) struct FlowBodyData<'a> {
    pub object_creations: Vec<ObjectCreation>,
    pub array_creations: Vec<ArrayCreation>,
    pub filled_arrays: Vec<FilledArraySite>,
    pub fresh_fills: Vec<FreshFillSite>,
    pub empty_fills: Vec<EmptyFillSite>,
    pub casts: Vec<CastSite>,
    pub strings: Vec<StringLiteral<'a>>,
    pub frame: MethodFrame,
    pub method_monitor: Option<MethodMonitor>,
    _sealed: (),
    pub calls: Vec<CallSite>,
    pub result: ResultType,
    pub switches: Vec<Switch>,
    pub raw: RawMethodView<'a>,
    /// The P1-disassembled original directory this flow was built from. Later
    /// stages read decoded operands and layouts from it.
    pub decoded: crate::input::DecodedMethod<'a>,
    pub parameters: Vec<DeclarationType>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub protection: crate::input::protection::CheckedProtection,
    pub exception_edges: Vec<super::protection::ExceptionEdge>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(crate) fn destination(op: Op) -> Option<RegisterId> {
    match op {
        Op::Call(_, destination) => destination,
        Op::FilledArray(_, result) => match result {
            FilledResult::Discarded => None,
            FilledResult::Stored { register, .. } => Some(register),
        },
        Op::Class(r, _)
        | Op::Exception(r)
        | Op::ConstantWide(r, _)
        | Op::MoveWide(r, _)
        | Op::LongBinary(r, _, _, _)
        | Op::DoubleBinary(r, _, _, _)
        | Op::Unary(r, _, _)
        | Op::ReverseSubtract(r, _, _)
        | Op::NewInstance(r, _)
        | Op::NewArray(r, _, _)
        | Op::Cast(r, _)
        | Op::InstanceOf(r, _, _, _)
        | Op::String(r, _)
        | Op::Constant(r, _)
        | Op::Move(r, _)
        | Op::FloatBinary(r, _, _, _)
        | Op::Binary(r, _, _, _)
        | Op::Compare(r, _, _, _)
        | Op::ArrayLength(r, _)
        | Op::ArrayLoad(r, _, _, _)
        | Op::FieldLoad(r, _, _, _)
        | Op::StaticLoad(r, _, _) => Some(r),
        Op::Monitor(..)
        | Op::Nop
        | Op::ArrayStore(..)
        | Op::FieldStore(..)
        | Op::StaticStore(..)
        | Op::EmptyFill(..)
        | Op::FreshFill(..)
        | Op::Initialize(..) => None,
    }
}
pub(crate) enum Reads<'a> {
    Many(std::slice::Iter<'a, RegisterId>),
    Local(std::array::IntoIter<Option<RegisterId>, 3>),
}
impl Iterator for Reads<'_> {
    type Item = RegisterId;
    fn next(&mut self) -> Option<RegisterId> {
        match self {
            Self::Many(registers) => registers.next().copied(),
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
    pub(crate) fn reads<'a>(
        self,
        calls: &'a [CallSite],
        filled_arrays: &'a [FilledArraySite],
    ) -> Result<Reads<'a>, RequestError> {
        let registers = match self {
            Self::Call(id, _) | Self::Initialize(id) => {
                let call = calls.get(ix(id.0)?).ok_or(RequestError::InternalInvariant(
                    "operation has a foreign call identity",
                ))?;
                return Ok(Reads::Many(call.arguments.iter()));
            }
            Self::FilledArray(id, _) => {
                let site = filled_arrays
                    .get(ix(id.0)?)
                    .ok_or(RequestError::InternalInvariant(
                        "filled array has a foreign identity",
                    ))?;
                return Ok(Reads::Many(site.registers.iter()));
            }
            Self::MoveWide(_, source)
            | Self::Unary(_, _, source)
            | Self::ReverseSubtract(_, _, source) => [Some(source), None, None],
            Self::Monitor(_, lock) => [Some(lock), None, None],
            Self::NewArray(_, length, _) => [Some(length), None, None],
            Self::FreshFill(register, _) => [Some(register), None, None],
            Self::EmptyFill(register, _) => [Some(register), None, None],
            Self::Cast(register, _) => [Some(register), None, None],
            Self::InstanceOf(_, source, _, _) => [Some(source), None, None],
            Self::StaticLoad(..) => [None, None, None],
            Self::StaticStore(value, _, _) => [Some(value), None, None],
            Self::FieldLoad(_, receiver, _, _) => [Some(receiver), None, None],
            Self::FieldStore(receiver, value, _, _) => [Some(receiver), Some(value), None],
            Self::ArrayLength(_, array) => [Some(array), None, None],
            Self::ArrayLoad(_, array, index, _) => [Some(array), Some(index), None],
            Self::ArrayStore(array, index, value, _) => [Some(array), Some(index), Some(value)],
            Self::LongBinary(_, left, _, right)
            | Self::DoubleBinary(_, left, _, right)
            | Self::FloatBinary(_, left, _, right)
            | Self::Compare(_, left, _, right) => [Some(left), Some(right), None],
            Self::Binary(_, left, _, Operand::Register(right)) => [Some(left), Some(right), None],
            Self::Binary(_, left, _, Operand::Literal(_)) | Self::Move(_, left) => {
                [Some(left), None, None]
            }
            Self::Class(..)
            | Self::Exception(_)
            | Self::ConstantWide(_, _)
            | Self::NewInstance(..)
            | Self::String(..)
            | Self::Constant(_, _)
            | Self::Nop => [None, None, None],
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
        Terminal::Throw(r)
        | Terminal::Return(r)
        | Terminal::Test { register: r, .. }
        | Terminal::Switch { register: r, .. } => Some(r),
        Terminal::Jump(_) | Terminal::ReturnVoid => None,
    }
}
fn contains_pc(
    pcs: &[CodeUnitOffset],
    pc: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    for old in pcs {
        budget.checkpoint()?;
        if *old == pc {
            return Ok(true);
        }
    }
    Ok(false)
}

fn block_at(
    pcs: &[CodeUnitOffset],
    pc: CodeUnitOffset,
    budget: &mut RunContext<'_>,
) -> Result<BlockId, RequestError> {
    for (index, old) in pcs.iter().enumerate() {
        budget.checkpoint()?;
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

pub(crate) fn declaration_for_projection(
    input: &MethodSource<'_, '_>,
    key: crate::input::TypeProjection,
    budget: &mut RunContext<'_>,
) -> Result<Option<DeclarationType>, RequestError> {
    declaration_from_descriptor(input.projected_name(key, budget)?, key, budget)
}

pub(super) fn declaration_from_descriptor(
    descriptor: crate::input::DexString<'_>,
    key: crate::input::TypeProjection,
    budget: &mut RunContext<'_>,
) -> Result<Option<DeclarationType>, RequestError> {
    if let Some(kind) = primitive_field_type(descriptor, budget)? {
        return Ok(Some(kind));
    }
    if descriptor.matches_with(
        crate::substrate::primitive_types::BuiltinReference::Object.descriptor(),
        budget,
    )? {
        return Ok(Some(DeclarationType::Object));
    }
    if descriptor.matches_with(
        crate::substrate::primitive_types::BuiltinReference::String.descriptor(),
        budget,
    )? {
        return Ok(Some(DeclarationType::String));
    }
    if let Some(kind) =
        crate::substrate::primitive_types::PrimitiveArray::parse(descriptor, budget)?
    {
        return Ok(Some(DeclarationType::PrimitiveArray(kind)));
    }
    if descriptor.matches_with(ArrayKind::object_descriptor(), budget)? {
        return Ok(Some(DeclarationType::ObjectArray));
    }
    Ok(match descriptor.reader()?.next(budget)? {
        Some(91) => Some(DeclarationType::ReferenceArray(key)),
        Some(76) if key.removed() == 0 => Some(DeclarationType::Nominal(key.root())),
        Some(76) => Some(DeclarationType::ProjectedNominal(key)),
        _ => None,
    })
}
pub(crate) fn reference_type(
    input: &MethodSource<'_, '_>,
    id: crate::TypeId,
    budget: &mut RunContext<'_>,
) -> Result<Option<DeclarationType>, RequestError> {
    Ok(
        declaration_for_projection(input, input.type_projection(id, budget)?, budget)?
            .filter(|kind| kind.field_offset() == 2),
    )
}
pub(crate) fn field_type(
    input: &MethodSource<'_, '_>,
    _descriptor: crate::input::DexString<'_>,
    id: crate::TypeId,
    budget: &mut RunContext<'_>,
) -> Result<Option<DeclarationType>, RequestError> {
    declaration_for_projection(input, input.type_projection(id, budget)?, budget)
}
pub(crate) fn primitive_field_type(
    descriptor: crate::input::DexString<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Option<DeclarationType>, RequestError> {
    let mut units = descriptor.reader()?;
    let kind = match units.next(budget)? {
        Some(73) => DeclarationType::Int,
        Some(70) => DeclarationType::Float,
        Some(74) => DeclarationType::Long,
        Some(68) => DeclarationType::Double,
        Some(90) => DeclarationType::Boolean,
        Some(66) => DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Byte),
        Some(83) => DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Short),
        Some(67) => DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Char),
        _ => return Ok(None),
    };
    Ok(if units.next(budget)?.is_none() {
        Some(kind)
    } else {
        None
    })
}
pub(crate) fn decode<'a>(
    _schedule: &crate::engine::SchedulePermit,
    input: &MethodSource<'_, 'a>,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<FlowBody<'a>>, RequestError> {
    decode_impl(input, budget)
}

fn decode_impl<'a>(
    input: &MethodSource<'_, 'a>,
    budget: &mut RunContext<'_>,
) -> Result<crate::substrate::outcome::Outcome<FlowBody<'a>>, RequestError> {
    let Some(decoded) = input.decoded() else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaAnalysisUnavailable,
        ));
    };
    let raw = decoded.original().clone();
    let protection = decoded.protection().clone();
    let result = if input
        .declaration()
        .return_type
        .matches_with(&[86], budget)?
    {
        ResultType::Void
    } else {
        ResultType::Value(
            if input
                .declaration()
                .return_type
                .matches_with(&[73], budget)?
            {
                DeclarationType::Int
            } else if input
                .declaration()
                .return_type
                .matches_with(&[90], budget)?
            {
                DeclarationType::Boolean
            } else if input
                .declaration()
                .return_type
                .matches_with(&[70], budget)?
            {
                DeclarationType::Float
            } else if input
                .declaration()
                .return_type
                .matches_with(&[74], budget)?
            {
                DeclarationType::Long
            } else if input
                .declaration()
                .return_type
                .matches_with(&[68], budget)?
            {
                DeclarationType::Double
            } else if input
                .declaration()
                .return_type
                .matches_with(&[66], budget)?
            {
                DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Byte)
            } else if input
                .declaration()
                .return_type
                .matches_with(&[83], budget)?
            {
                DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Short)
            } else if input
                .declaration()
                .return_type
                .matches_with(&[67], budget)?
            {
                DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Char)
            } else if let Some(kind) = reference_type(
                input,
                crate::TypeId(input.declaration().return_type_id),
                budget,
            )? {
                kind
            } else {
                return Ok(crate::substrate::outcome::Outcome::Declined(
                    crate::OutputReason::JavaCheckFailed {
                        code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                        phase: "P2",
                        check: "return_type_projection",
                        pc: None,
                    },
                ));
            },
        )
    };
    let frame = if input.access_flags() & crate::substrate::flags::STATIC != 0 {
        MethodFrame::Static
    } else {
        let owner = input
            .method_reference(input.method_id(), budget)?
            .owner_type;
        if input.access_flags() & crate::substrate::flags::CONSTRUCTOR != 0 {
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
    for (name, id) in input
        .declaration()
        .parameter_types
        .iter()
        .zip(&input.declaration().parameter_type_ids)
    {
        budget.checkpoint()?;
        let id = crate::TypeId(*id);
        let kind = (|| -> Result<Option<DeclarationType>, RequestError> {
            let kind = if name.matches_with(&[73], budget)? {
                DeclarationType::Int
            } else if name.matches_with(&[90], budget)? {
                DeclarationType::Boolean
            } else if let Some(kind) =
                crate::substrate::primitive_types::PrimitiveArray::parse_units(
                    name.units(),
                    budget,
                )?
            {
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
            } else if name.matches_with(&[74], budget)? {
                DeclarationType::Long
            } else if name.matches_with(&[68], budget)? {
                DeclarationType::Double
            } else if name.matches_with(&[66], budget)? {
                DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Byte)
            } else if name.matches_with(&[83], budget)? {
                DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Short)
            } else if name.matches_with(&[67], budget)? {
                DeclarationType::Narrow(crate::substrate::primitive_types::Narrow::Char)
            } else if name.matches_with(
                &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Object;") },
                budget,
            )? {
                DeclarationType::Object
            } else if name.matches_with(
                crate::substrate::primitive_types::BuiltinReference::String.descriptor(),
                budget,
            )? {
                DeclarationType::String
            } else if let Some(kind) = reference_type(input, id, budget)? {
                kind
            } else {
                supported = false;
                return Ok(None);
            };
            Ok(Some(kind))
        })()?;
        let Some(kind) = kind else {
            supported = false;
            continue;
        };
        push(&mut parameters, kind, budget)?;
    }
    if !supported {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P2",
                check: "parameter_type_projection",
                pc: None,
            },
        ));
    }
    let mut instructions = Vec::new();
    let mut calls = Vec::new();
    let mut strings = Vec::new();
    let mut casts = Vec::new();
    let mut empty_fills = Vec::new();
    let mut array_creations = Vec::new();
    let mut filled_arrays = Vec::new();
    let mut object_creations = Vec::new();
    let mut fresh_fills = Vec::new();
    let mut raw_switches: Vec<(Vec<(i32, CodeUnitOffset)>, CodeUnitOffset)> = Vec::new();
    let mut leaders = Vec::new();
    push(&mut leaders, CodeUnitOffset(0), budget)?;
    for boundary in protection.boundaries() {
        budget.checkpoint()?;
        push(
            &mut leaders,
            super::protection::boundary(&raw, boundary)?,
            budget,
        )?;
    }
    for target in protection.targets() {
        budget.checkpoint()?;
        if decoded.is_reachable(target) {
            push(&mut leaders, target, budget)?;
        }
    }
    let mut pc = CodeUnitOffset(0);
    while pc.0 < raw.code_units().0 {
        budget.checkpoint()?;
        let layout = decoded.layout_at(pc)?;
        let mut next = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
        if decoded.is_reachable(pc) {
            let LayoutKind::Instruction(spec) = layout.kind() else {
                return Ok(crate::substrate::outcome::Outcome::Declined(
                    crate::OutputReason::JavaCheckFailed {
                        code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                        phase: "P2",
                        check: "reachable_payload",
                        pc: Some(pc),
                    },
                ));
            };
            let operands = decoded.operands_at(pc)?;
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
            let local = super::local_ops::local(spec, operands)?;
            let initialization = super::local_ops::initialization(
                input.program(),
                pc,
                spec,
                layout,
                operands,
                &mut calls,
                budget,
            )?;
            let decoded = match spec.opcode {
                0 | 1..=9 | 0x12..=0x19 => Decoded::Op(local.ok_or(
                    RequestError::InternalInvariant("local opcode lost shared operation"),
                )?),
                0x70 | 0x76 if initialization.is_some() => Decoded::Op(initialization.ok_or(
                    RequestError::InternalInvariant("constructor opcode lost shared operation"),
                )?),
                0x0d => Decoded::Op(Op::Exception(reg(0)?)),
                0x1c => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant(
                            "class literal lost reference",
                        ));
                    };
                    let crate::raw::OperandReference::Type(id) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "class literal changed reference kind",
                        ));
                    };
                    Decoded::Op(Op::Class(reg(0)?, id))
                }
                0x1d | 0x1e => Decoded::Op(Op::Monitor(
                    if spec.opcode == 0x1d {
                        MonitorKind::Enter
                    } else {
                        MonitorKind::Exit
                    },
                    reg(0)?,
                )),
                0x27 => Decoded::Throw(reg(0)?),
                0x7b..=0x8f => {
                    let unary = match spec.opcode {
                        0x7b => crate::p2_flow::primitives::Unary::Neg,
                        0x7c => crate::p2_flow::primitives::Unary::Not,
                        0x7d => crate::p2_flow::primitives::Unary::NegLong,
                        0x7e => crate::p2_flow::primitives::Unary::NotLong,
                        0x7f => crate::p2_flow::primitives::Unary::NegFloat,
                        0x80 => crate::p2_flow::primitives::Unary::NegDouble,
                        0x81 => crate::p2_flow::primitives::Unary::IntToLong,
                        0x83 => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Int,
                            to: crate::p2_flow::primitives::ScalarType::Double,
                        },
                        0x84 => crate::p2_flow::primitives::Unary::LongToInt,
                        0x85 => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Long,
                            to: crate::p2_flow::primitives::ScalarType::Float,
                        },
                        0x86 => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Long,
                            to: crate::p2_flow::primitives::ScalarType::Double,
                        },
                        0x88 => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Float,
                            to: crate::p2_flow::primitives::ScalarType::Long,
                        },
                        0x89 => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Float,
                            to: crate::p2_flow::primitives::ScalarType::Double,
                        },
                        0x8a => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Double,
                            to: crate::p2_flow::primitives::ScalarType::Int,
                        },
                        0x8b => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Double,
                            to: crate::p2_flow::primitives::ScalarType::Long,
                        },
                        0x8c => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Double,
                            to: crate::p2_flow::primitives::ScalarType::Float,
                        },
                        0x82 => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Int,
                            to: crate::p2_flow::primitives::ScalarType::Float,
                        },
                        0x87 => crate::p2_flow::primitives::Unary::Convert {
                            from: crate::p2_flow::primitives::ScalarType::Float,
                            to: crate::p2_flow::primitives::ScalarType::Int,
                        },
                        0x8d => crate::p2_flow::primitives::Unary::Byte,
                        0x8e => crate::p2_flow::primitives::Unary::Char,
                        _ => crate::p2_flow::primitives::Unary::Short,
                    };
                    Decoded::Op(Op::Unary(reg(0)?, unary, reg(1)?))
                }
                0x9b..=0xa5 | 0xbb..=0xc5 => {
                    let arithmetic = match spec.opcode {
                        0x9b | 0xbb => Arithmetic::Add,
                        0x9c | 0xbc => Arithmetic::Sub,
                        0x9d | 0xbd => Arithmetic::Mul,
                        0x9e | 0xbe => Arithmetic::Div,
                        0x9f | 0xbf => Arithmetic::Rem,
                        0xa0 | 0xc0 => Arithmetic::And,
                        0xa1 | 0xc1 => Arithmetic::Or,
                        0xa2 | 0xc2 => Arithmetic::Xor,
                        0xa3 | 0xc3 => Arithmetic::Shl,
                        0xa4 | 0xc4 => Arithmetic::Shr,
                        _ => Arithmetic::Ushr,
                    };
                    let (left, right) = if spec.opcode < 0xb0 {
                        (reg(1)?, reg(2)?)
                    } else {
                        (reg(0)?, reg(1)?)
                    };
                    Decoded::Op(Op::LongBinary(reg(0)?, left, arithmetic, right))
                }
                0xab..=0xaf | 0xcb..=0xcf => {
                    let arithmetic = match spec.opcode {
                        0xab | 0xcb => Arithmetic::Add,
                        0xac | 0xcc => Arithmetic::Sub,
                        0xad | 0xcd => Arithmetic::Mul,
                        0xae | 0xce => Arithmetic::Div,
                        _ => Arithmetic::Rem,
                    };
                    let (left, right) = if spec.opcode < 0xc0 {
                        (reg(1)?, reg(2)?)
                    } else {
                        (reg(0)?, reg(1)?)
                    };
                    Decoded::Op(Op::DoubleBinary(reg(0)?, left, arithmetic, right))
                }
                0xa6..=0xaa | 0xc6..=0xca => {
                    let arithmetic = match spec.opcode {
                        0xa6 | 0xc6 => Arithmetic::Add,
                        0xa7 | 0xc7 => Arithmetic::Sub,
                        0xa8 | 0xc8 => Arithmetic::Mul,
                        0xa9 | 0xc9 => Arithmetic::Div,
                        _ => Arithmetic::Rem,
                    };
                    let (left, right) = if spec.opcode < 0xc0 {
                        (reg(1)?, reg(2)?)
                    } else {
                        (reg(0)?, reg(1)?)
                    };
                    Decoded::Op(Op::FloatBinary(reg(0)?, left, arithmetic, right))
                }
                0x90..=0x9a | 0xb0..=0xba | 0xd0..=0xe2 => {
                    let arithmetic = match spec.opcode {
                        0x90 | 0xb0 | 0xd0 | 0xd8 => Arithmetic::Add,
                        0x91 | 0xb1 | 0xd1 | 0xd9 => Arithmetic::Sub,
                        0x92 | 0xb2 | 0xd2 | 0xda => Arithmetic::Mul,
                        0x93 | 0xb3 | 0xd3 | 0xdb => Arithmetic::Div,
                        0x94 | 0xb4 | 0xd4 | 0xdc => Arithmetic::Rem,
                        0x95 | 0xb5 | 0xd5 | 0xdd => Arithmetic::And,
                        0x96 | 0xb6 | 0xd6 | 0xde => Arithmetic::Or,
                        0x97 | 0xb7 | 0xd7 | 0xdf => Arithmetic::Xor,
                        0x98 | 0xb8 | 0xe0 => Arithmetic::Shl,
                        0x99 | 0xb9 | 0xe1 => Arithmetic::Shr,
                        _ => Arithmetic::Ushr,
                    };
                    let (left, right) = match spec.opcode {
                        0x90..=0x9a => (reg(1)?, Operand::Register(reg(2)?)),
                        0xb0..=0xba => (reg(0)?, Operand::Register(reg(1)?)),
                        _ => (
                            reg(1)?,
                            Operand::Literal(match operands.literal() {
                                Some(RawLiteral::Signed8(value)) => i32::from(value),
                                Some(RawLiteral::Signed16(value)) => i32::from(value),
                                _ => {
                                    return Err(RequestError::InternalInvariant(
                                        "int literal lost signed operand",
                                    ));
                                }
                            }),
                        ),
                    };
                    if matches!(spec.opcode, 0xd1 | 0xd9) {
                        let Operand::Literal(literal) = right else {
                            return Err(RequestError::InternalInvariant("rsub lost literal"));
                        };
                        Decoded::Op(Op::ReverseSubtract(reg(0)?, literal, left))
                    } else {
                        Decoded::Op(Op::Binary(reg(0)?, left, arithmetic, right))
                    }
                }
                0x52..=0x6d => {
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
                    let field = input.field_reference(id, budget)?;
                    let Some(kind) = field_type(
                        input,
                        field.declaration.field_type,
                        field.declaration.field_type_idx,
                        budget,
                    )?
                    else {
                        return Ok(crate::substrate::outcome::Outcome::Declined(
                            crate::OutputReason::JavaCheckFailed {
                                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                                phase: "P2",
                                check: "field_type_projection",
                                pc: Some(pc),
                            },
                        ));
                    };
                    Decoded::Op(match (spec.opcode - 0x52) / 7 {
                        0 => Op::FieldLoad(reg(0)?, reg(1)?, id, kind),
                        1 => Op::FieldStore(reg(1)?, reg(0)?, id, kind),
                        2 => Op::StaticLoad(reg(0)?, id, kind),
                        3 => Op::StaticStore(reg(0)?, id, kind),
                        _ => {
                            return Err(RequestError::InternalInvariant("unexpected field opcode"));
                        }
                    })
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
                    let Some(component) = ArrayKind::parse(input, reference, budget)? else {
                        return Ok(crate::substrate::outcome::Outcome::Declined(
                            crate::OutputReason::JavaCheckFailed {
                                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                                phase: "P2",
                                check: "new_array_component",
                                pc: Some(pc),
                            },
                        ));
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
                0x24 | 0x25 => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant("filled array lost type"));
                    };
                    let crate::raw::OperandReference::Type(reference) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "filled array changed reference domain",
                        ));
                    };
                    let Some(component) = ArrayKind::parse(input, reference, budget)? else {
                        return Ok(crate::substrate::outcome::Outcome::Declined(
                            crate::OutputReason::JavaCheckFailed {
                                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                                phase: "P2",
                                check: "filled_array_component",
                                pc: Some(pc),
                            },
                        ));
                    };
                    let mut registers = Vec::new();
                    for register in operands.registers().iter() {
                        budget.checkpoint()?;
                        push(&mut registers, register, budget)?;
                    }
                    let mut result = FilledResult::Discarded;
                    if next.0 < raw.code_units().0 {
                        let result_layout = decoded.layout_at(next)?;
                        if matches!(result_layout.kind(),LayoutKind::Instruction(spec) if spec.opcode == 0x0c)
                        {
                            let result_operands = decoded.operands_at(next)?;
                            let register = result_operands.registers().iter().next().ok_or(
                                RequestError::InternalInvariant(
                                    "filled array result lost destination",
                                ),
                            )?;
                            result = FilledResult::Stored { register, pc: next };
                            next = next
                                .checked_advance(result_layout.width())
                                .ok_or(RequestError::PlatformCapacity)?;
                        }
                    }
                    let id = FilledArrayId(
                        u32::try_from(filled_arrays.len())
                            .map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(
                        &mut filled_arrays,
                        FilledArraySite {
                            reference,
                            component,
                            registers,
                            pc,
                        },
                        budget,
                    )?;
                    Decoded::Op(Op::FilledArray(id, result))
                }
                0x26 => {
                    let delta = operands
                        .offset()
                        .ok_or(RequestError::InternalInvariant("fill lost payload offset"))?;
                    let payload = CodeUnitOffset(pc.0.checked_add_signed(delta.0).ok_or(
                        RequestError::InternalInvariant("checked fill target overflowed"),
                    )?);
                    let data = decoded.layout_at(payload)?;
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
                0x1f | 0x20 => {
                    let Some(reference) = layout.references()[0] else {
                        return Err(RequestError::InternalInvariant("cast lost target"));
                    };
                    let crate::raw::OperandReference::Type(reference) = reference.target else {
                        return Err(RequestError::InternalInvariant(
                            "cast changed reference domain",
                        ));
                    };
                    let Some(kind) = reference_type(input, reference, budget)? else {
                        return Ok(crate::substrate::outcome::Outcome::Declined(
                            crate::OutputReason::JavaCheckFailed {
                                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                                phase: "P2",
                                check: "cast_type_projection",
                                pc: Some(pc),
                            },
                        ));
                    };
                    if spec.opcode == 0x20 {
                        Decoded::Op(Op::InstanceOf(reg(0)?, reg(1)?, reference, kind))
                    } else {
                        let id = CastId(
                            u32::try_from(casts.len())
                                .map_err(|_| RequestError::PlatformCapacity)?,
                        );
                        push(&mut casts, CastSite { reference, kind }, budget)?;
                        Decoded::Op(Op::Cast(reg(0)?, id))
                    }
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
                0x44..=0x4a => Decoded::Op(Op::ArrayLoad(
                    reg(0)?,
                    reg(1)?,
                    reg(2)?,
                    match spec.opcode - 0x44 {
                        0 => ArrayElement::Int,
                        1 => ArrayElement::Wide,
                        2 => ArrayElement::Object,
                        3 => ArrayElement::Boolean,
                        4 => ArrayElement::Byte,
                        5 => ArrayElement::Char,
                        _ => ArrayElement::Short,
                    },
                )),
                0x4b..=0x51 => Decoded::Op(Op::ArrayStore(
                    reg(1)?,
                    reg(2)?,
                    reg(0)?,
                    match spec.opcode - 0x4b {
                        0 => ArrayElement::Int,
                        1 => ArrayElement::Wide,
                        2 => ArrayElement::Object,
                        3 => ArrayElement::Boolean,
                        4 => ArrayElement::Byte,
                        5 => ArrayElement::Char,
                        _ => ArrayElement::Short,
                    },
                )),
                0xe => Decoded::ReturnVoid,
                0x10 if result.width() == ValueWidth::Two => Decoded::Return(reg(0)?),
                0xf | 0x11 if result.width() == ValueWidth::One => Decoded::Return(reg(0)?),
                0x2b | 0x2c => {
                    let payload = target()?;
                    let cases = decoded.switch_cases_at(payload)?.ok_or(
                        RequestError::InternalInvariant("switch lost its decoded payload"),
                    )?;
                    let mut targets = Vec::new();
                    for case in cases {
                        budget.checkpoint()?;
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
                0x2f..=0x31 => Decoded::Op(Op::Compare(
                    reg(0)?,
                    reg(1)?,
                    match spec.opcode {
                        0x2f => crate::p2_flow::primitives::Compare::DoubleL,
                        0x30 => crate::p2_flow::primitives::Compare::DoubleG,
                        _ => crate::p2_flow::primitives::Compare::Long,
                    },
                    reg(2)?,
                )),
                0x2d | 0x2e => Decoded::Op(Op::Compare(
                    reg(0)?,
                    reg(1)?,
                    if spec.opcode == 0x2d {
                        crate::p2_flow::primitives::Compare::FloatL
                    } else {
                        crate::p2_flow::primitives::Compare::FloatG
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
                    let constructor = method.declaration.name.matches_with(
                        &const { crate::substrate::owned::ascii_units(*b"<init>") },
                        budget,
                    )?;
                    let kind = InvokeKind::from_opcode(spec.opcode)
                        .ok_or(RequestError::InternalInvariant("invoke lost its kind"))?;
                    let descriptor = method.declaration.prototype.return_type;
                    let mut units = descriptor.reader()?;
                    let result = match units.next(budget)? {
                        Some(86) => InvokeResult::Void,
                        Some(73) => InvokeResult::Word(DeclarationType::Int),
                        Some(90) => InvokeResult::Word(DeclarationType::Boolean),
                        Some(66) => InvokeResult::Word(DeclarationType::Narrow(
                            crate::substrate::primitive_types::Narrow::Byte,
                        )),
                        Some(83) => InvokeResult::Word(DeclarationType::Narrow(
                            crate::substrate::primitive_types::Narrow::Short,
                        )),
                        Some(67) => InvokeResult::Word(DeclarationType::Narrow(
                            crate::substrate::primitive_types::Narrow::Char,
                        )),
                        Some(70) => InvokeResult::Word(DeclarationType::Float),
                        Some(74) => InvokeResult::Wide(DeclarationType::Long),
                        Some(68) => InvokeResult::Wide(DeclarationType::Double),
                        Some(76 | 91) => InvokeResult::Reference(reference_type(
                            input,
                            method.declaration.prototype.return_type_idx,
                            budget,
                        )?),
                        _ => {
                            return Err(RequestError::InternalInvariant(
                                "checked invoke lost return descriptor",
                            ));
                        }
                    };
                    let mut destination = None;
                    let mut result_pc = None;
                    if next.0 < raw.code_units().0 {
                        let result_layout = decoded.layout_at(next)?;
                        if matches!(result_layout.kind(),LayoutKind::Instruction(spec) if matches!(spec.opcode,0x0a..=0x0c))
                        {
                            let valid = match result {
                                InvokeResult::Word(_) => {
                                    matches!(result_layout.kind(), LayoutKind::Instruction(spec) if spec.opcode == 0x0a)
                                }
                                InvokeResult::Reference(Some(_)) => {
                                    matches!(result_layout.kind(), LayoutKind::Instruction(spec) if spec.opcode == 0x0c)
                                }
                                InvokeResult::Wide(_) => {
                                    matches!(result_layout.kind(), LayoutKind::Instruction(spec) if spec.opcode == 0x0b)
                                }
                                InvokeResult::Reference(None) => {
                                    return Ok(crate::substrate::outcome::Outcome::Declined(
                                        crate::OutputReason::JavaCheckFailed {
                                            code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                                            phase: "P2",
                                            check: "invoke_result_type",
                                            pc: Some(pc),
                                        },
                                    ));
                                }
                                InvokeResult::Void => false,
                            };
                            if !valid {
                                return Err(RequestError::InternalInvariant(
                                    "void call acquired move-result",
                                ));
                            }
                            let result = decoded.operands_at(next)?;
                            destination = Some(result.registers().iter().next().ok_or(
                                RequestError::InternalInvariant("move-result lost destination"),
                            )?);
                            result_pc = Some(next);
                            next = next
                                .checked_advance(result_layout.width())
                                .ok_or(RequestError::PlatformCapacity)?;
                        }
                    }
                    let mut site = super::local_ops::call(
                        input.program(),
                        pc,
                        kind,
                        reference,
                        result,
                        operands,
                        budget,
                    )?;
                    site.result_pc = result_pc;
                    let call = CallId(
                        u32::try_from(calls.len()).map_err(|_| RequestError::PlatformCapacity)?,
                    );
                    push(&mut calls, site, budget)?;
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
                0xfc | 0xfd => {
                    return Ok(crate::substrate::outcome::Outcome::Declined(
                        crate::OutputReason::InvokeCustomUnsupported,
                    ));
                }
                0xfa | 0xfb => {
                    return Ok(crate::substrate::outcome::Outcome::Declined(
                        crate::OutputReason::InvokePolymorphicUnsupported,
                    ));
                }
                _ => {
                    return Ok(crate::substrate::outcome::Outcome::Declined(
                        crate::OutputReason::JavaCheckFailed {
                            code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                            phase: "P2",
                            check: "unsupported_opcode",
                            pc: Some(pc),
                        },
                    ));
                }
            };
            if let Decoded::Op(op) = decoded {
                if matches!(op, Op::Monitor(..)) {
                    push(&mut leaders, pc, budget)?;
                    push(&mut leaders, next, budget)?;
                } else if op.effects().may_throw && protection.at(pc, budget)?.is_some() {
                    push(&mut leaders, next, budget)?;
                }
            }
            push(&mut instructions, (pc, decoded), budget)?;
        }
        pc = next;
    }
    // Block order follows original instruction order; no output sorting is used.
    let mut pcs = Vec::new();
    for &(pc, _) in &instructions {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
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
                    budget.checkpoint()?;
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
            Decoded::Throw(register) => Some(Terminal::Throw(register)),
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
                    budget.checkpoint()?;
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
    let Some(exception_edges) = super::protection::build(&protection, &blocks, &edges, budget)?
    else {
        return Ok(crate::substrate::outcome::Outcome::Declined(
            crate::OutputReason::JavaCheckFailed {
                code: crate::DiagnosticCode::JavaAnalysisUnavailable,
                phase: "P2",
                check: "exception_edge_construction",
                pc: None,
            },
        ));
    };
    super::protection::verify(&raw, &protection, &blocks, &edges, &exception_edges, budget)?;
    Ok(crate::substrate::outcome::Outcome::Ready(FlowBody(
        FlowBodyData {
            object_creations,
            array_creations,
            filled_arrays,
            fresh_fills,
            empty_fills,
            casts,
            strings,
            frame,
            method_monitor: if input.access_flags() & crate::substrate::flags::SYNCHRONIZED == 0 {
                None
            } else if input.access_flags() & crate::substrate::flags::STATIC != 0 {
                Some(MethodMonitor::DeclaringClass)
            } else {
                Some(MethodMonitor::This)
            },
            _sealed: (),
            calls,
            result,
            switches,
            raw,
            decoded: decoded.clone(),
            parameters,
            blocks,
            edges,
            protection,
            exception_edges,
        },
    )))
}
