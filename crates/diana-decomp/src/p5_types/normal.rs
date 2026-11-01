use crate::p2_flow::normal::Op;
use crate::p3_ssa::normal::{Definition, SsaBody, Sym};
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Int,
    Boolean,
    Object,
    String,
    Float,
    PrimitiveArray(super::PrimitiveArray),
    ObjectArray,
    Nominal(crate::TypeId),
}
impl Kind {
    pub(crate) fn builtin_spelling(self) -> Option<&'static str> {
        Some(match self {
            Self::Int => "int",
            Self::Boolean => "boolean",
            Self::Object => "java.lang.Object",
            Self::String => "java.lang.String",
            Self::Float => "float",
            Self::PrimitiveArray(kind) => kind.spelling(),
            Self::ObjectArray => "java.lang.Object[]",
            Self::Nominal(_) => return None,
        })
    }
}
pub(crate) struct TypedBody<'a> {
    _sealed: (),
    pub ssa: SsaBody<'a>,
    pub kinds: Vec<Option<Kind>>,
    pub integral_bits: Vec<u8>,
    pub ranges: Vec<Option<crate::p5_types::IntRange>>,
    pub arrays: Vec<Option<ArrayEvidence>>,
    pub bindings: Vec<crate::p4_calls::normal::CallEvidence>,
    pub call_uses: Vec<crate::p5_types::calls::CallUses>,
    pub cast_uses: Vec<crate::p5_types::calls::TypedUse>,
    pub initializations: super::initialization::Initializations,
    pub empty_fills: Vec<PrimitiveFillUse>,
    pub array_lengths: Vec<crate::p5_types::calls::TypedUse>,
    pub fresh_fills: Vec<PrimitiveFillUse>,

    pub field_declarations: Vec<Option<crate::p4_calls::normal::FieldEvidence>>,
    pub field_uses: Vec<Option<FieldUse>>,
}
pub(crate) struct PrimitiveFillUse {
    _sealed: (),
    pub array: crate::p3_ssa::normal::ValueId,
    pub component: super::PrimitiveArray,
}
pub(crate) enum FieldUse {
    Read,
    Write(UseConversion),
}
impl TypedBody<'_> {
    pub(crate) fn contract(
        &self,
        id: crate::p2_flow::normal::CallId,
    ) -> Result<&crate::p4_calls::StaticContract, RequestError> {
        crate::p4_calls::normal::contract(&self.ssa, &self.bindings, id)
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum UseConversion {
    Identity,
    BooleanWord,
    Boolean01,
    Null(crate::p5_types::BuiltinReference),
    UpcastObject,
    Narrow(crate::p5_types::Narrow),
}
pub(crate) enum ArrayUse {
    Length,
    Load {
        index: UseConversion,
    },
    Store {
        index: UseConversion,
        value: UseConversion,
    },
}
pub(crate) struct ArrayEvidence {
    _sealed: (),
    pub usage: ArrayUse,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
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
pub(crate) fn solve<'a>(
    resolved: crate::p4_calls::normal::ResolvedBody<'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<TypedBody<'a>>, RequestError> {
    let ssa = resolved.ssa;
    let bindings = resolved.calls;
    let field_declarations = resolved.fields;
    let mut integral_bits = Vec::new();
    let mut ranges = Vec::new();
    let mut kinds = Vec::new();
    for definition in &ssa.values {
        budget.charge_work(1)?;
        let (bits, range, kind) = match *definition {
            Definition::Parameter(index) => {
                let declaration = ssa.parameters[ix(index)?];
                let kind = declaration.kind();
                let bits = match kind {
                    Kind::Boolean => 3,
                    Kind::Int => 7,
                    Kind::Object
                    | Kind::String
                    | Kind::Float
                    | Kind::PrimitiveArray(_)
                    | Kind::ObjectArray
                    | Kind::Nominal(_) => 0,
                };
                (bits, declaration.range(), Some(kind))
            }
            Definition::OpResult(id) => match ssa.operation(id)?.op {
                Op::NewInstance(_, id) => (
                    0,
                    None,
                    Some(Kind::Nominal(
                        ssa.object_creations
                            .get(ix(id.0)?)
                            .ok_or(RequestError::InternalInvariant(
                                "object allocation lost type",
                            ))?
                            .reference,
                    )),
                ),
                Op::NewArray(_, _, id) => (
                    0,
                    None,
                    Some(Kind::PrimitiveArray(
                        ssa.array_creations
                            .get(ix(id.0)?)
                            .ok_or(RequestError::InternalInvariant(
                                "array allocation lost type",
                            ))?
                            .component,
                    )),
                ),
                Op::Cast(_, id) => (
                    0,
                    None,
                    Some(
                        match ssa
                            .casts
                            .get(ix(id.0)?)
                            .ok_or(RequestError::InternalInvariant("cast lost target type"))?
                            .kind
                        {
                            crate::p5_types::BuiltinReference::Object => Kind::Object,
                            crate::p5_types::BuiltinReference::String => Kind::String,
                        },
                    ),
                ),
                Op::String(..) => (0, None, Some(Kind::String)),
                Op::Constant(_, value) => (
                    match value {
                        0 => 1,
                        1 => 2,
                        _ => 4,
                    },
                    Some(crate::p5_types::IntRange::literal(value)),
                    Some(Kind::Int),
                ),
                Op::FieldLoad(..)
                | Op::ArrayLength(..)
                | Op::ArrayLoad(_, _, _, crate::p2_flow::normal::ArrayElement::Int)
                | Op::Binary(_, _, _, _)
                | Op::Compare(_, _, _, _) => {
                    (7, Some(crate::p5_types::IntRange::ALL), Some(Kind::Int))
                }
                Op::Call(call, _) => {
                    let declaration = match ssa.calls[ix(call.0)?].result {
                        crate::p2_flow::normal::InvokeResult::Word(kind)
                        | crate::p2_flow::normal::InvokeResult::Reference(Some(kind)) => kind,
                        _ => {
                            return Err(RequestError::InternalInvariant(
                                "unsupported raw call result entered normal typing",
                            ));
                        }
                    };
                    let kind = declaration.kind();
                    let bits = match kind {
                        Kind::Int => 7,
                        Kind::Boolean => 3,
                        _ => 0,
                    };
                    (bits, declaration.range(), Some(kind))
                }
                Op::ArrayLoad(_, _, _, crate::p2_flow::normal::ArrayElement::Object) => {
                    (0, None, Some(Kind::Object))
                }
                Op::Move(_, _) => (0, None, None),
                Op::Nop
                | Op::ArrayStore(..)
                | Op::FieldStore(..)
                | Op::EmptyFill(..)
                | Op::FreshFill(..)
                | Op::Initialize(..) => {
                    return Err(RequestError::InternalInvariant(
                        "NOP acquired a normal value definition",
                    ));
                }
            },
            Definition::Phi => (0, None, None),
        };
        push(&mut integral_bits, bits, budget)?;
        push(&mut ranges, range, budget)?;
        push(&mut kinds, kind, budget)?;
    }
    let mut type_conflict = false;
    loop {
        let mut changed = false;
        for op in &ssa.operations {
            budget.charge_work(1)?;
            match op.op {
                Op::Compare(_, _, _, _) | Op::Binary(_, _, _, _) => {
                    let expected = if matches!(op.op, Op::Compare(_, _, _, _)) {
                        Kind::Float
                    } else {
                        Kind::Int
                    };
                    for input in &op.inputs {
                        budget.charge_work(1)?;
                        if kinds[ix(input.0)?].is_some_and(|kind| kind != expected) {
                            type_conflict = true;
                        }
                    }
                }
                Op::Move(_, _) => {
                    let index = ix(op
                        .result
                        .ok_or(RequestError::InternalInvariant("move lost its result"))?
                        .0)?;
                    let source = ix(op.input(0)?.0)?;
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
                _ => {}
            }
        }
        for phi in &ssa.phis {
            budget.charge_work(1)?;
            let index = ix(phi.result.0)?;
            for &(_, input) in &phi.inputs {
                budget.charge_work(1)?;
                let Sym::Value(source) = input else {
                    return Err(RequestError::InternalInvariant(
                        "unavailable Phi entered type solving",
                    ));
                };
                let source = ix(source.0)?;
                let source_range = ranges[source];
                changed |= merge_range(&mut ranges[index], source_range);
                let merged = integral_bits[index] | integral_bits[source];
                if merged != integral_bits[index] {
                    integral_bits[index] = merged;
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
        if !changed {
            break;
        }
    }
    if type_conflict {
        return Ok(None);
    }
    let Some(arrays) = solve_arrays(&ssa, &kinds, &integral_bits, budget)? else {
        return Ok(None);
    };
    let initializations = super::initialization::solve(&ssa, budget)?;
    let Some(array_uses) = solve_new_arrays(&ssa, &kinds, budget)? else {
        return Ok(None);
    };
    let Some(empty_fills) = solve_empty_fills(&ssa, &kinds, budget)? else {
        return Ok(None);
    };
    let Some(cast_uses) = solve_casts(&ssa, &kinds, budget)? else {
        return Ok(None);
    };
    let Some(field_uses) = solve_fields(&ssa, &field_declarations, &kinds, budget)? else {
        return Ok(None);
    };
    let Some(call_uses) =
        super::calls::solve(&ssa, &bindings, &kinds, &integral_bits, &ranges, budget)?
    else {
        return Ok(None);
    };
    Ok(Some(TypedBody {
        _sealed: (),
        ssa,
        kinds,
        integral_bits,
        ranges,
        arrays,
        bindings,
        call_uses,
        cast_uses,
        initializations,
        empty_fills,
        array_lengths: array_uses.lengths,
        fresh_fills: array_uses.fills,
        field_declarations,
        field_uses,
    }))
}
fn solve_arrays(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    bits: &[u8],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Option<ArrayEvidence>>>, RequestError> {
    use crate::p2_flow::normal::ArrayElement;
    let mut result = Vec::new();
    for op in &ssa.operations {
        budget.charge_work(1)?;
        let bad = || {
            RequestError::from(crate::input::MethodError::Malformed {
                pc: op.pc,
                problem: crate::input::MethodProblem::InvalidArrayAccessType,
            })
        };
        let usage = match op.op {
            Op::ArrayLength(..) | Op::ArrayLoad(..) | Op::ArrayStore(..) => {
                let component = match kinds[ix(op.input(0)?.0)?] {
                    Some(Kind::PrimitiveArray(kind))
                        if kind == super::PrimitiveArray::Int
                            || matches!(op.op, Op::ArrayLength(..)) =>
                    {
                        ArrayElement::Int
                    }
                    Some(Kind::ObjectArray) => ArrayElement::Object,
                    _ => return Ok(None),
                };
                if matches!(op.op, Op::ArrayLength(..)) {
                    ArrayUse::Length
                } else {
                    let element = match op.op {
                        Op::ArrayLoad(_, _, _, element) | Op::ArrayStore(_, _, _, element) => {
                            element
                        }
                        _ => {
                            return Err(RequestError::InternalInvariant(
                                "array access lost element kind",
                            ));
                        }
                    };
                    if component != element {
                        return Err(bad());
                    }
                    let index = match kinds[ix(op.input(1)?.0)?] {
                        Some(Kind::Int) => UseConversion::Identity,
                        Some(Kind::Boolean) => UseConversion::BooleanWord,
                        None => return Ok(None),
                        _ => return Err(bad()),
                    };
                    if matches!(op.op, Op::ArrayLoad(..)) {
                        ArrayUse::Load { index }
                    } else {
                        let at = ix(op.input(2)?.0)?;
                        let value = match (component, kinds[at]) {
                            (ArrayElement::Int, Some(Kind::Int))
                            | (ArrayElement::Object, Some(Kind::Object)) => UseConversion::Identity,
                            (ArrayElement::Int, Some(Kind::Boolean)) => UseConversion::BooleanWord,
                            (
                                ArrayElement::Object,
                                Some(
                                    Kind::String
                                    | Kind::PrimitiveArray(_)
                                    | Kind::ObjectArray
                                    | Kind::Nominal(_),
                                ),
                            ) => UseConversion::UpcastObject,
                            (ArrayElement::Object, Some(Kind::Int)) if bits[at] == 1 => {
                                UseConversion::Null(crate::p5_types::BuiltinReference::Object)
                            }
                            (_, None) => return Ok(None),
                            _ => return Err(bad()),
                        };
                        ArrayUse::Store { index, value }
                    }
                }
            }
            _ => {
                push(&mut result, None, budget)?;
                continue;
            }
        };
        push(
            &mut result,
            Some(ArrayEvidence { _sealed: (), usage }),
            budget,
        )?;
    }
    Ok(Some(result))
}

fn solve_fields(
    ssa: &SsaBody<'_>,
    declarations: &[Option<crate::p4_calls::normal::FieldEvidence>],
    kinds: &[Option<Kind>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Option<FieldUse>>>, RequestError> {
    let mut uses = Vec::new();
    for op in &ssa.operations {
        budget.charge_work(1)?;
        let reference = match op.op {
            Op::FieldLoad(_, _, id) | Op::FieldStore(_, _, id) => id,
            _ => {
                push(&mut uses, None, budget)?;
                continue;
            }
        };
        let evidence = declarations
            .get(ix(op.id.0)?)
            .and_then(Option::as_ref)
            .ok_or(RequestError::InternalInvariant(
                "field operation lost original constraints",
            ))?;
        if evidence.reference != reference {
            return Err(RequestError::InternalInvariant(
                "field constraints changed reference",
            ));
        }
        if kinds[ix(op.input(0)?.0)?] != Some(Kind::Nominal(evidence.owner)) {
            return Ok(None);
        }
        let usage = if matches!(op.op, Op::FieldLoad(..)) {
            FieldUse::Read
        } else {
            let conversion = match kinds[ix(op.input(1)?.0)?] {
                Some(Kind::Int) => UseConversion::Identity,
                Some(Kind::Boolean) => UseConversion::BooleanWord,
                None => return Ok(None),
                _ => {
                    return Err(crate::input::MethodError::Malformed {
                        pc: op.pc,
                        problem: crate::input::MethodProblem::InvalidFieldKind,
                    }
                    .into());
                }
            };
            FieldUse::Write(conversion)
        };
        push(&mut uses, Some(usage), budget)?;
    }
    Ok(Some(uses))
}

fn solve_casts(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<crate::p5_types::calls::TypedUse>>, RequestError> {
    let mut uses = Vec::new();
    for op in &ssa.operations {
        budget.charge_work(1)?;
        let Op::Cast(_, id) = op.op else {
            continue;
        };
        if ix(id.0)? != uses.len() {
            return Err(RequestError::InternalInvariant(
                "cast use changed arena identity",
            ));
        }
        let value = op.input(0)?;
        let conversion = if ssa.is_zero(value)? {
            UseConversion::Null(crate::p5_types::BuiltinReference::Object)
        } else {
            match kinds[ix(value.0)?] {
                Some(Kind::Object | Kind::String) => UseConversion::Identity,
                Some(Kind::Nominal(_) | Kind::PrimitiveArray(_) | Kind::ObjectArray) => {
                    UseConversion::UpcastObject
                }
                None => return Ok(None),
                _ => {
                    return Err(crate::input::MethodError::Malformed {
                        pc: op.pc,
                        problem: crate::input::MethodProblem::InvalidTypeOperand,
                    }
                    .into());
                }
            }
        };
        push(
            &mut uses,
            crate::p5_types::calls::TypedUse { value, conversion },
            budget,
        )?;
    }
    Ok(Some(uses))
}

fn solve_empty_fills(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<PrimitiveFillUse>>, RequestError> {
    let mut uses = Vec::new();
    for op in &ssa.operations {
        budget.charge_work(1)?;
        let Op::EmptyFill(_, id) = op.op else {
            continue;
        };
        if ix(id.0)? != uses.len() {
            return Err(RequestError::InternalInvariant(
                "empty fill changed arena identity",
            ));
        }
        let site = ssa
            .empty_fills
            .get(ix(id.0)?)
            .ok_or(RequestError::InternalInvariant(
                "fill lost original payload",
            ))?;
        let array = op.input(0)?;
        let component = match kinds[ix(array.0)?] {
            Some(Kind::PrimitiveArray(component)) if component.width() == site.width => component,
            None => return Ok(None),
            _ if ssa.is_zero(array)? => return Ok(None),
            _ => {
                return Err(crate::input::MethodError::Malformed {
                    pc: op.pc,
                    problem: crate::input::MethodProblem::InvalidArrayPayloadType,
                }
                .into());
            }
        };
        push(
            &mut uses,
            PrimitiveFillUse {
                _sealed: (),
                array,
                component,
            },
            budget,
        )?;
    }
    Ok(Some(uses))
}

struct ArrayUses {
    lengths: Vec<crate::p5_types::calls::TypedUse>,
    fills: Vec<PrimitiveFillUse>,
}
fn solve_new_arrays(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ArrayUses>, RequestError> {
    let mut lengths = Vec::new();
    let mut fills = Vec::new();
    for op in &ssa.operations {
        budget.charge_work(1)?;
        match op.op {
            Op::NewArray(_, _, id) => {
                if ix(id.0)? != lengths.len() {
                    return Err(RequestError::InternalInvariant(
                        "array creation changed identity",
                    ));
                }
                let value = op.input(0)?;
                let conversion = match kinds[ix(value.0)?] {
                    Some(Kind::Int) => UseConversion::Identity,
                    Some(Kind::Boolean) => UseConversion::BooleanWord,
                    None => return Ok(None),
                    _ => {
                        return Err(crate::input::MethodError::Malformed {
                            pc: op.pc,
                            problem: crate::input::MethodProblem::InvalidTypeOperand,
                        }
                        .into());
                    }
                };
                push(
                    &mut lengths,
                    crate::p5_types::calls::TypedUse { value, conversion },
                    budget,
                )?;
            }
            Op::FreshFill(_, id) => {
                if ix(id.0)? != fills.len() {
                    return Err(RequestError::InternalInvariant(
                        "fresh fill changed identity",
                    ));
                }
                let site = ssa
                    .fresh_fills
                    .get(ix(id.0)?)
                    .ok_or(RequestError::InternalInvariant("fresh fill lost payload"))?;
                let array = op.input(0)?;
                let component = match kinds[ix(array.0)?] {
                    Some(Kind::PrimitiveArray(component)) if component.width() == site.width => {
                        component
                    }
                    None => return Ok(None),
                    _ if ssa.is_zero(array)? => return Ok(None),
                    _ => {
                        return Err(crate::input::MethodError::Malformed {
                            pc: op.pc,
                            problem: crate::input::MethodProblem::InvalidArrayPayloadType,
                        }
                        .into());
                    }
                };
                push(
                    &mut fills,
                    PrimitiveFillUse {
                        _sealed: (),
                        array,
                        component,
                    },
                    budget,
                )?;
            }
            _ => {}
        }
    }
    Ok(Some(ArrayUses { lengths, fills }))
}
