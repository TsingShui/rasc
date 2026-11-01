use crate::input::{MethodInput, RawMethodView, RawPrintable};
use crate::raw::{LayoutKind, OperandReference, RawLiteral, instruction_layout};
use crate::substrate::owned::reserve;
use crate::{CodeUnitOffset, DeclarationCatalog, MethodBudget, RegisterId, RequestError};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Int,
    Object,
    String,
    IntArray,
    ObjectArray,
    Nominal,
}
impl Kind {
    pub(crate) fn descriptor(self) -> Option<&'static [u16]> {
        Some(match self {
            Self::Int => &[73],
            Self::Object => crate::p5_types::BuiltinReference::Object.descriptor(),
            Self::String => crate::p5_types::BuiltinReference::String.descriptor(),
            Self::IntArray => &[91, 73],
            Self::ObjectArray => &[
                91, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
            ],
            Self::Nominal => return None,
        })
    }
    pub(crate) fn spelling(self) -> Option<&'static str> {
        Some(match self {
            Self::Int => "int",
            Self::Object => "java.lang.Object",
            Self::String => "java.lang.String",
            Self::IntArray => "int[]",
            Self::ObjectArray => "java.lang.Object[]",
            Self::Nominal => return None,
        })
    }
    pub(crate) fn reference(self) -> bool {
        self != Self::Int
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ValueId(pub u32);
#[derive(Clone, Copy)]
pub(crate) enum ElementUse {
    Value(ValueId),
    Null(ValueId),
}
impl ElementUse {
    pub(crate) fn id(self) -> ValueId {
        match self {
            Self::Value(id) | Self::Null(id) => id,
        }
    }
}
pub(crate) enum Operation {
    FieldLoad {
        pc: CodeUnitOffset,
        binding: crate::p4_calls::InstanceIntAccess,
        receiver: ValueId,
        result: ValueId,
    },
    FieldStore {
        pc: CodeUnitOffset,
        binding: crate::p4_calls::InstanceIntAccess,
        receiver: ValueId,
        value: ValueId,
    },
    AddLiteral {
        pc: CodeUnitOffset,
        source: ValueId,
        result: ValueId,
        literal: i32,
    },
    Constant {
        pc: CodeUnitOffset,
        value: ValueId,
        literal: i32,
    },
    Call {
        pc: CodeUnitOffset,
        contract: crate::p4_calls::StaticContract,
        result: Option<(ValueId, CodeUnitOffset)>,
    },
    Load {
        pc: CodeUnitOffset,
        array: ValueId,
        index: ValueId,
        result: ValueId,
    },
    Store {
        pc: CodeUnitOffset,
        array: ValueId,
        index: ValueId,
        value: ElementUse,
    },
    Length {
        pc: CodeUnitOffset,
        array: ValueId,
        result: ValueId,
    },
}
#[derive(Clone, Copy)]
pub(crate) enum Return {
    Void,
    Int(ValueId),
}
pub(crate) struct AccessPlan<'a> {
    _sealed: (),
    pub raw: RawMethodView<'a>,
    pub parameters: Vec<Kind>,
    pub nominal: Option<Vec<u16>>,
    pub types: Vec<Kind>,
    pub zero: Vec<bool>,
    pub operations: Vec<Operation>,
    pub returned: Return,
    pub return_pc: CodeUnitOffset,
}
pub(crate) fn index(value: ValueId) -> Result<usize, RequestError> {
    usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)
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
fn define(
    types: &mut Vec<Kind>,
    zero: &mut Vec<bool>,
    kind: Kind,
    is_zero: bool,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ValueId, RequestError> {
    let id = ValueId(u32::try_from(types.len()).map_err(|_| RequestError::PlatformCapacity)?);
    push(types, kind, budget)?;
    push(zero, is_zero, budget)?;
    Ok(id)
}
fn bad_type(pc: CodeUnitOffset) -> RequestError {
    crate::input::MethodError::Malformed {
        pc,
        problem: crate::input::MethodProblem::InvalidArrayAccessType,
    }
    .into()
}
pub(crate) fn build<'a>(
    input: &MethodInput<'_, 'a>,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<AccessPlan<'a>>, RequestError> {
    let RawPrintable::Method(checked) = input.raw() else {
        return Ok(None);
    };
    let raw = checked.layout().original().clone();
    if input.access_flags() & 8 == 0 || !raw.try_byte_range().as_range().is_empty() {
        return Ok(None);
    }
    let returns_int = input
        .declaration()
        .prototype
        .return_type
        .matches_with(&[73], budget)?;
    if !returns_int
        && !input
            .declaration()
            .prototype
            .return_type
            .matches_with(&[86], budget)?
    {
        return Ok(None);
    }
    let mut parameters = Vec::new();
    let mut nominal: Option<Vec<u16>> = None;
    let mut supported = true;
    input
        .declaration()
        .prototype
        .visit_parameter_types(budget, |name, budget| {
            for kind in [
                Kind::Int,
                Kind::Object,
                Kind::String,
                Kind::IntArray,
                Kind::ObjectArray,
            ] {
                if let Some(descriptor) = kind.descriptor()
                    && name.matches_with(descriptor, budget)?
                {
                    push(&mut parameters, kind, budget)?;
                    return Ok::<(), RequestError>(());
                }
            }
            let mut reader = name.reader()?;
            if reader.next(budget)? == Some(76) {
                if let Some(previous) = &nominal {
                    if !name.matches_with(previous, budget)? {
                        supported = false;
                        return Ok(());
                    }
                } else {
                    nominal = Some(crate::substrate::owned::copy_units(name, budget)?);
                }
                push(&mut parameters, Kind::Nominal, budget)?;
            } else {
                supported = false;
            }
            Ok(())
        })?;
    if !supported
        || parameters.len() != usize::from(raw.ins_size())
        || !parameters
            .iter()
            .any(|kind| matches!(kind, Kind::IntArray | Kind::ObjectArray | Kind::Nominal))
    {
        return Ok(None);
    }
    let mut slots = Vec::new();
    reserve(&mut slots, usize::from(raw.registers_size()), budget)?;
    for _ in 0..raw.registers_size() {
        budget.charge_work(1)?;
        slots.push(None);
    }
    let mut types = Vec::new();
    let mut zero = Vec::new();
    let start =
        slots
            .len()
            .checked_sub(parameters.len())
            .ok_or(RequestError::InternalInvariant(
                "array parameters exceed checked frame",
            ))?;
    for (i, &kind) in parameters.iter().enumerate() {
        budget.charge_work(1)?;
        slots[start + i] = Some(define(&mut types, &mut zero, kind, false, budget)?);
    }
    let mut operations = Vec::new();
    let mut accessed = false;
    let mut pc = CodeUnitOffset(0);
    while pc.0 < raw.code_units().0 {
        budget.charge_work(1)?;
        if !checked.layout().is_reachable(pc) {
            return Ok(None);
        }
        let layout = instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        let LayoutKind::Instruction(spec) = layout.kind() else {
            return Ok(None);
        };
        let operands = crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
        let reg = |i| {
            operands
                .registers()
                .iter()
                .nth(i)
                .ok_or(RequestError::InternalInvariant(
                    "array access lost checked register",
                ))
        };
        let slot = |r: RegisterId| usize::try_from(r.0).map_err(|_| RequestError::PlatformCapacity);
        let read = |r: RegisterId| {
            slots[slot(r)?].ok_or(RequestError::UndefinedRegister { register: r, pc })
        };
        let mut next = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
        match spec.opcode {
            0 => {}
            0x12..=0x15 => {
                let literal = match operands.literal() {
                    Some(RawLiteral::Bits32(v)) => i32::from_le_bytes(v.to_le_bytes()),
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => return Err(RequestError::InternalInvariant("array constant lost bits")),
                };
                let value = define(&mut types, &mut zero, Kind::Int, literal == 0, budget)?;
                slots[slot(reg(0)?)?] = Some(value);
                push(
                    &mut operations,
                    Operation::Constant { pc, value, literal },
                    budget,
                )?;
            }
            0x01..=0x03 | 0x07..=0x09 => {
                let source = read(reg(1)?)?;
                if matches!(spec.opcode, 0x07..=0x09)
                    && !types[index(source)?].reference()
                    && !zero[index(source)?]
                {
                    return Err(bad_type(pc));
                }
                if matches!(spec.opcode, 0x01..=0x03) && types[index(source)?] != Kind::Int {
                    return Err(bad_type(pc));
                }
                slots[slot(reg(0)?)?] = Some(source);
            }
            0x71 | 0x77 => {
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant("array call lost reference"));
                };
                let OperandReference::Method(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "array call changed reference domain",
                    ));
                };
                let Some(contract) =
                    crate::p4_calls::resolve_static(input, catalog, reference, budget)?
                else {
                    return Err(RequestError::BindingEvidenceMissing { reference, pc });
                };
                if !contract.formals.is_empty()
                    || !operands.registers().is_empty()
                    || !contract.declared_throws.is_empty()
                    || !matches!(
                        contract.result,
                        crate::p4_calls::StaticResult::Int | crate::p4_calls::StaticResult::Void
                    )
                {
                    return Ok(None);
                }
                let mut result = None;
                if next.0 < raw.code_units().0 {
                    let following = instruction_layout(raw.instructions(), next, raw.version())
                        .map_err(crate::input::MethodError::Decode)?;
                    if matches!(following.kind(),LayoutKind::Instruction(spec) if spec.opcode==0x0a)
                    {
                        if contract.result != crate::p4_calls::StaticResult::Int {
                            return Err(RequestError::InternalInvariant(
                                "void array call acquired result",
                            ));
                        }
                        let output = crate::input::operands::instruction_operands(
                            raw.instructions(),
                            next,
                            following,
                        )
                        .map_err(crate::input::MethodError::Decode)?;
                        let destination = output.registers().iter().next().ok_or(
                            RequestError::InternalInvariant("array call result lost register"),
                        )?;
                        let value = define(&mut types, &mut zero, Kind::Int, false, budget)?;
                        slots[slot(destination)?] = Some(value);
                        result = Some((value, next));
                        next = next
                            .checked_advance(following.width())
                            .ok_or(RequestError::PlatformCapacity)?;
                    }
                }
                push(
                    &mut operations,
                    Operation::Call {
                        pc,
                        contract,
                        result,
                    },
                    budget,
                )?;
            }
            0x52..=0x5f => {
                let receiver = read(reg(1)?)?;
                if types[index(receiver)?] != Kind::Nominal {
                    return Ok(None);
                }
                let Some(reference) = layout.references()[0] else {
                    return Err(RequestError::InternalInvariant(
                        "field access lost reference",
                    ));
                };
                let OperandReference::Field(reference) = reference.target else {
                    return Err(RequestError::InternalInvariant(
                        "field access changed reference domain",
                    ));
                };
                let original = input.field_reference(reference, budget)?;
                if !original
                    .declaration
                    .field_type
                    .matches_with(&[73], budget)?
                {
                    return Ok(None);
                }
                let usage = match spec.opcode {
                    0x52 => crate::p4_calls::FieldUse::Read,
                    0x59 => crate::p4_calls::FieldUse::Write,
                    _ => {
                        return Err(crate::input::MethodError::Malformed {
                            pc,
                            problem: crate::input::MethodProblem::InvalidFieldKind,
                        }
                        .into());
                    }
                };
                let descriptor = nominal.as_deref().ok_or(RequestError::InternalInvariant(
                    "nominal receiver lost descriptor",
                ))?;
                let value = if usage == crate::p4_calls::FieldUse::Write {
                    let value = read(reg(0)?)?;
                    if types[index(value)?] != Kind::Int {
                        return Err(bad_type(pc));
                    }
                    Some(value)
                } else {
                    None
                };
                let binding = crate::p4_calls::bind_instance_int(
                    input, catalog, reference, descriptor, usage, pc, budget,
                )?;
                accessed = true;
                if let Some(value) = value {
                    push(
                        &mut operations,
                        Operation::FieldStore {
                            pc,
                            binding,
                            receiver,
                            value,
                        },
                        budget,
                    )?;
                } else {
                    let result = define(&mut types, &mut zero, Kind::Int, false, budget)?;
                    slots[slot(reg(0)?)?] = Some(result);
                    push(
                        &mut operations,
                        Operation::FieldLoad {
                            pc,
                            binding,
                            receiver,
                            result,
                        },
                        budget,
                    )?;
                }
            }
            0xd0 | 0xd8 => {
                let source = read(reg(1)?)?;
                if types[index(source)?] != Kind::Int {
                    return Err(bad_type(pc));
                }
                let literal = match operands.literal() {
                    Some(RawLiteral::Signed8(v)) => i32::from(v),
                    Some(RawLiteral::Signed16(v)) => i32::from(v),
                    _ => return Err(RequestError::InternalInvariant("heap add lost literal")),
                };
                let result = define(&mut types, &mut zero, Kind::Int, false, budget)?;
                slots[slot(reg(0)?)?] = Some(result);
                push(
                    &mut operations,
                    Operation::AddLiteral {
                        pc,
                        source,
                        result,
                        literal,
                    },
                    budget,
                )?;
            }
            0x44..=0x51 => {
                let array = read(reg(1)?)?;
                let index_value = read(reg(2)?)?;
                let element = match types[index(array)?] {
                    Kind::IntArray => Kind::Int,
                    Kind::ObjectArray => Kind::Object,
                    _ => return Ok(None),
                };
                let expected = match (element, spec.opcode <= 0x4a) {
                    (Kind::Int, true) => 0x44,
                    (Kind::Int, false) => 0x4b,
                    (Kind::Object, true) => 0x46,
                    (Kind::Object, false) => 0x4d,
                    _ => {
                        return Err(RequestError::InternalInvariant(
                            "array element family changed",
                        ));
                    }
                };
                if spec.opcode != expected || types[index(index_value)?] != Kind::Int {
                    return Err(bad_type(pc));
                }
                accessed = true;
                if spec.opcode <= 0x4a {
                    let result = define(&mut types, &mut zero, element, false, budget)?;
                    slots[slot(reg(0)?)?] = Some(result);
                    push(
                        &mut operations,
                        Operation::Load {
                            pc,
                            array,
                            index: index_value,
                            result,
                        },
                        budget,
                    )?;
                } else {
                    let value = read(reg(0)?)?;
                    let view = if element == Kind::Object {
                        if types[index(value)?].reference() {
                            ElementUse::Value(value)
                        } else if zero[index(value)?] {
                            ElementUse::Null(value)
                        } else {
                            return Err(bad_type(pc));
                        }
                    } else {
                        if types[index(value)?] != Kind::Int {
                            return Err(bad_type(pc));
                        }
                        ElementUse::Value(value)
                    };
                    push(
                        &mut operations,
                        Operation::Store {
                            pc,
                            array,
                            index: index_value,
                            value: view,
                        },
                        budget,
                    )?;
                }
            }
            0x21 => {
                let array = read(reg(1)?)?;
                if !matches!(types[index(array)?], Kind::IntArray | Kind::ObjectArray) {
                    return Ok(None);
                }
                let result = define(&mut types, &mut zero, Kind::Int, false, budget)?;
                slots[slot(reg(0)?)?] = Some(result);
                accessed = true;
                push(
                    &mut operations,
                    Operation::Length { pc, array, result },
                    budget,
                )?;
            }
            0x0e | 0x0f => {
                if !accessed {
                    return Ok(None);
                }
                let returned = if spec.opcode == 0x0e && !returns_int {
                    Return::Void
                } else if spec.opcode == 0x0f && returns_int {
                    let value = read(reg(0)?)?;
                    if types[index(value)?] != Kind::Int {
                        return Err(bad_type(pc));
                    }
                    Return::Int(value)
                } else {
                    return Ok(None);
                };
                return Ok(Some(AccessPlan {
                    _sealed: (),
                    raw,
                    parameters,
                    nominal,
                    types,
                    zero,
                    operations,
                    returned,
                    return_pc: pc,
                }));
            }
            _ => return Ok(None),
        }
        pc = next;
    }
    Ok(None)
}
