use crate::p3_ssa::normal::{BoundOperand, Definition, SsaBody, Sym};
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Int,
    Boolean,
    Object,
    String,
    Float,
}
impl Kind {
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Boolean => "boolean",
            Self::Object => "java.lang.Object",
            Self::String => "java.lang.String",
            Self::Float => "float",
        }
    }
}
pub(crate) struct TypedBody<'a> {
    _sealed: (),
    pub ssa: SsaBody<'a>,
    pub kinds: Vec<Option<Kind>>,
    pub integral_bits: Vec<u8>,
    pub ranges: Vec<Option<crate::p5_types::IntRange>>,
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
    ssa: SsaBody<'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<TypedBody<'a>>, RequestError> {
    let mut integral_bits = Vec::new();
    let mut ranges = Vec::new();
    let mut kinds = Vec::new();
    let mut type_conflict = false;
    for definition in &ssa.values {
        budget.charge_work(1)?;
        let bits = match definition {
            Definition::Parameter(index) => match ssa.parameters[ix(*index)?].kind() {
                Kind::Boolean => 3,
                Kind::Int => 7,
                Kind::Object | Kind::String | Kind::Float => 0,
            },
            Definition::Constant(value) => match value {
                0 => 1,
                1 => 2,
                _ => 4,
            },
            Definition::Binary(_, _, _) | Definition::Compare(_, _, _) => 7,
            Definition::CallResult(call) => match ssa.calls[ix(call.0)?].contract.result {
                crate::p4_calls::StaticResult::Int => 7,
                crate::p4_calls::StaticResult::Boolean => 3,
                _ => 0,
            },
            _ => 0,
        };
        let range = match definition {
            Definition::Parameter(index) => ssa.parameters[ix(*index)?].range(),
            Definition::Constant(value) => Some(crate::p5_types::IntRange::literal(*value)),
            Definition::Binary(_, _, _) | Definition::Compare(_, _, _) => {
                Some(crate::p5_types::IntRange::ALL)
            }
            Definition::CallResult(call) => match ssa.calls[ix(call.0)?].contract.result {
                crate::p4_calls::StaticResult::Int => Some(crate::p5_types::IntRange::ALL),
                crate::p4_calls::StaticResult::Boolean => Some(crate::p5_types::IntRange::BOOLEAN),
                _ => None,
            },
            _ => None,
        };
        push(&mut ranges, range, budget)?;
        push(&mut integral_bits, bits, budget)?;
        push(
            &mut kinds,
            match definition {
                Definition::Parameter(index) => Some(ssa.parameters[ix(*index)?].kind()),
                Definition::Constant(_)
                | Definition::Binary(_, _, _)
                | Definition::Compare(_, _, _) => Some(Kind::Int),
                Definition::CallResult(call) => {
                    Some(match ssa.calls[ix(call.0)?].contract.result {
                        crate::p4_calls::StaticResult::Int => Kind::Int,
                        crate::p4_calls::StaticResult::Boolean => Kind::Boolean,
                        crate::p4_calls::StaticResult::Reference(
                            crate::p5_types::BuiltinReference::Object,
                        ) => Kind::Object,
                        crate::p4_calls::StaticResult::Reference(
                            crate::p5_types::BuiltinReference::String,
                        ) => Kind::String,
                        crate::p4_calls::StaticResult::Nominal => {
                            return Err(RequestError::InternalInvariant(
                                "nominal call result entered primitive SSA",
                            ));
                        }
                        crate::p4_calls::StaticResult::Void => {
                            return Err(RequestError::InternalInvariant(
                                "void call acquired a value definition",
                            ));
                        }
                    })
                }
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
                    if kinds[source].is_some_and(|kind| kind != Kind::Int) {
                        type_conflict = true;
                    }
                }
            }
            if let Definition::Move(source) = definition {
                let source = ix(source.0)?;
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
                        return Err(RequestError::InternalInvariant(
                            "unavailable Phi entered type solving",
                        ));
                    }
                    Sym::Value(source) => {
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
            }
        }
        if !changed {
            break;
        }
    }
    if type_conflict {
        return Ok(None);
    }
    Ok(Some(TypedBody {
        _sealed: (),
        ssa,
        kinds,
        integral_bits,
        ranges,
    }))
}
