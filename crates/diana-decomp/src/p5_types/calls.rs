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
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ReceiverIdentity {
    Reference,
    ImplicitThis,
}
pub(crate) struct TypedReceiver {
    pub identity: ReceiverIdentity,
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
                    Some(
                        Kind::String
                        | Kind::PrimitiveArray(_)
                        | Kind::ObjectArray
                        | Kind::Nominal(_),
                    ),
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
                    identity: if site.kind == crate::p2_flow::normal::InvokeKind::Super {
                        if !is_this(ssa, value, budget)? {
                            return Ok(None);
                        }
                        ReceiverIdentity::ImplicitThis
                    } else {
                        ReceiverIdentity::Reference
                    },
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

fn is_this(
    ssa: &SsaBody<'_>,
    root: ValueId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    if !matches!(ssa.frame, crate::p2_flow::normal::MethodFrame::Instance(_)) {
        return Ok(false);
    }
    let mut seen = Vec::new();
    for _ in &ssa.values {
        budget.charge_work(1)?;
        push(&mut seen, false, budget)?;
    }
    let mut phi_indices = Vec::new();
    for _ in &ssa.values {
        budget.charge_work(1)?;
        push(&mut phi_indices, None, budget)?;
    }
    for (index, phi) in ssa.phis.iter().enumerate() {
        budget.charge_work(1)?;
        let result = usize::try_from(phi.result.0).map_err(|_| RequestError::PlatformCapacity)?;
        if phi_indices[result].replace(index).is_some() {
            return Err(RequestError::InternalInvariant(
                "this origin has duplicate Phi",
            ));
        }
    }
    let mut pending = Vec::new();
    push(&mut pending, root, budget)?;
    let mut anchored = false;
    while let Some(value) = pending.pop() {
        budget.charge_work(1)?;
        let index = usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?;
        if seen[index] {
            continue;
        }
        seen[index] = true;
        match ssa.values[index] {
            crate::p3_ssa::normal::Definition::Parameter(0) => anchored = true,
            crate::p3_ssa::normal::Definition::OpResult(id) => {
                let op = ssa.operation(id)?;
                if !matches!(op.op, crate::p2_flow::normal::Op::Move(..)) {
                    return Ok(false);
                }
                push(&mut pending, op.input(0)?, budget)?;
            }
            crate::p3_ssa::normal::Definition::Phi => {
                let phi_index = phi_indices[index]
                    .ok_or(RequestError::InternalInvariant("this origin lost Phi"))?;
                for &(_, source) in &ssa.phis[phi_index].inputs {
                    budget.charge_work(1)?;
                    let crate::p3_ssa::normal::Sym::Value(source) = source else {
                        return Ok(false);
                    };
                    push(&mut pending, source, budget)?;
                }
            }
            _ => return Ok(false),
        }
    }
    // An alias cycle alone is not evidence of this; require the entry anchor.
    Ok(anchored)
}
