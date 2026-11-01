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
    IntArray,
    ObjectArray,
}
impl Kind {
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Boolean => "boolean",
            Self::Object => "java.lang.Object",
            Self::String => "java.lang.String",
            Self::Float => "float",
            Self::IntArray => "int[]",
            Self::ObjectArray => "java.lang.Object[]",
        }
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
pub(crate) enum ArrayConversion {
    Identity,
    BooleanWord,
    Null,
    UpcastObject,
}
pub(crate) enum ArrayUse {
    Length,
    Load {
        index: ArrayConversion,
    },
    Store {
        index: ArrayConversion,
        value: ArrayConversion,
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
                    | Kind::IntArray
                    | Kind::ObjectArray => 0,
                };
                (bits, declaration.range(), Some(kind))
            }
            Definition::OpResult(id) => match ssa.operation(id)?.op {
                Op::Constant(_, value) => (
                    match value {
                        0 => 1,
                        1 => 2,
                        _ => 4,
                    },
                    Some(crate::p5_types::IntRange::literal(value)),
                    Some(Kind::Int),
                ),
                Op::ArrayLength(..)
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
                Op::Nop | Op::ArrayStore(..) => {
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
    Ok(Some(TypedBody {
        _sealed: (),
        ssa,
        kinds,
        integral_bits,
        ranges,
        arrays,
        bindings,
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
                    Some(Kind::IntArray) => ArrayElement::Int,
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
                        Some(Kind::Int) => ArrayConversion::Identity,
                        Some(Kind::Boolean) => ArrayConversion::BooleanWord,
                        None => return Ok(None),
                        _ => return Err(bad()),
                    };
                    if matches!(op.op, Op::ArrayLoad(..)) {
                        ArrayUse::Load { index }
                    } else {
                        let at = ix(op.input(2)?.0)?;
                        let value = match (component, kinds[at]) {
                            (ArrayElement::Int, Some(Kind::Int))
                            | (ArrayElement::Object, Some(Kind::Object)) => {
                                ArrayConversion::Identity
                            }
                            (ArrayElement::Int, Some(Kind::Boolean)) => {
                                ArrayConversion::BooleanWord
                            }
                            (
                                ArrayElement::Object,
                                Some(Kind::String | Kind::IntArray | Kind::ObjectArray),
                            ) => ArrayConversion::UpcastObject,
                            (ArrayElement::Object, Some(Kind::Int)) if bits[at] == 1 => {
                                ArrayConversion::Null
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
