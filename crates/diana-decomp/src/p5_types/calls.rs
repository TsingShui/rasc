use super::normal::{Kind, UseConversion};
use super::{BuiltinReference, IntRange};
use crate::p2_flow::normal::CallId;
use crate::p3_ssa::normal::{SsaBody, ValueId};
use crate::p4_calls::{CallFormal, normal::CallEvidence};
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

pub(crate) struct TypedUse {
    pub value: ValueId,
    pub conversion: UseConversion,
}
pub(crate) struct CallUses {
    _sealed: (),
    pub arguments: Vec<TypedUse>,
    pub receiver: Option<TypedReceiver>,
}
pub(crate) struct TypedReceiver {
    pub value: ValueId,
    pub original_type: crate::TypeId,
}
fn conversion(
    ssa: &SsaBody<'_>,
    value: ValueId,
    formal: CallFormal,
    kinds: &[Option<Kind>],
    bits: &[u8],
    ranges: &[Option<IntRange>],
) -> Result<Option<UseConversion>, RequestError> {
    let index = usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?;
    if let CallFormal::Reference(kind) = formal {
        return Ok(if ssa.is_zero(value)? {
            Some(UseConversion::Null(kind))
        } else {
            match (kinds[index], kind) {
                (Some(Kind::Object), BuiltinReference::Object)
                | (Some(Kind::String), BuiltinReference::String) => Some(UseConversion::Identity),
                (
                    Some(Kind::String | Kind::IntArray | Kind::ObjectArray | Kind::Nominal(_)),
                    BuiltinReference::Object,
                ) => Some(UseConversion::UpcastObject),
                _ => None,
            }
        });
    }
    Ok(match (kinds[index], formal) {
        (Some(Kind::Int), CallFormal::Int) | (Some(Kind::Boolean), CallFormal::Boolean) => {
            Some(UseConversion::Identity)
        }
        (Some(Kind::Boolean), CallFormal::Int) => Some(UseConversion::BooleanWord),
        (Some(Kind::Int), CallFormal::Boolean) if bits[index] != 0 && bits[index] & 4 == 0 => {
            Some(UseConversion::Boolean01)
        }
        (Some(Kind::Int), CallFormal::Narrow(kind)) => {
            let range = if bits[index] != 0 && bits[index] & 4 == 0 {
                ranges[index].and_then(IntRange::known01)
            } else {
                ranges[index]
            };
            if range.is_some_and(|range| range.within(kind.range())) {
                Some(UseConversion::Narrow(kind))
            } else {
                None
            }
        }
        _ => None,
    })
}
pub(crate) fn solve(
    ssa: &SsaBody<'_>,
    bindings: &[CallEvidence],
    kinds: &[Option<Kind>],
    bits: &[u8],
    ranges: &[Option<IntRange>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<CallUses>>, RequestError> {
    let mut uses = Vec::new();
    for (index, site) in ssa.calls.iter().enumerate() {
        budget.charge_work(1)?;
        let id = CallId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let evidence = bindings.get(index).ok_or(RequestError::InternalInvariant(
            "call lost declaration constraints",
        ))?;
        let Some(formals) = evidence.formals() else {
            return Err(RequestError::BindingEvidenceMissing {
                reference: site.reference,
                pc: site.pc,
            });
        };
        let bound = ssa.call(id)?;
        let (receiver, values) = if site.kind == crate::p2_flow::normal::InvokeKind::Static {
            (None, bound.inputs.as_slice())
        } else {
            let Some((&value, arguments)) = bound.inputs.split_first() else {
                return Err(RequestError::InternalInvariant(
                    "receiver call lost receiver",
                ));
            };
            let Some(Kind::Nominal(original_type)) =
                kinds[usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
            else {
                return Ok(None);
            };
            (
                Some(TypedReceiver {
                    value,
                    original_type,
                }),
                arguments,
            )
        };
        if values.len() != formals.len() {
            return Err(RequestError::InternalInvariant(
                "original formal word constraints changed",
            ));
        }
        let mut arguments = Vec::new();
        for (&value, &formal) in values.iter().zip(formals) {
            budget.charge_work(1)?;
            let Some(conversion) = conversion(ssa, value, formal, kinds, bits, ranges)? else {
                if matches!(evidence, CallEvidence::Missing { .. }) {
                    return Err(RequestError::BindingEvidenceMissing {
                        reference: site.reference,
                        pc: site.pc,
                    });
                }
                return Ok(None);
            };
            push(&mut arguments, TypedUse { value, conversion }, budget)?;
        }
        push(
            &mut uses,
            CallUses {
                _sealed: (),
                arguments,
                receiver,
            },
            budget,
        )?;
    }
    Ok(Some(uses))
}
