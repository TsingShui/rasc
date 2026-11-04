use super::IntRange;
use super::normal::{Kind, UseConversion};
use crate::p2_flow::CallId;
use crate::p3_ssa::{SsaBody, ValueId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

pub(crate) struct TypedUse {
    pub value: ValueId,
    pub conversion: UseConversion,
}
pub(crate) struct CallUses {
    _sealed: (),
    arguments: Vec<TypedUse>,
    pub receiver: Option<TypedReceiver>,
}
pub(crate) enum TypedReceiver {
    Constructor {
        value: ValueId,
        initialization: super::initialization::InitializationId,
    },
    Super {
        value: ValueId,
    },
    Value(TypedUse),
}
impl TypedReceiver {
    pub(crate) fn value(&self) -> ValueId {
        match self {
            Self::Constructor { value, .. } | Self::Super { value } => *value,
            Self::Value(usage) => usage.value,
        }
    }
}
impl CallUses {
    pub(crate) fn arguments(&self) -> &[TypedUse] {
        &self.arguments
    }
}
fn conversion(
    ssa: &SsaBody<'_>,
    zeros: &super::zero::ZeroFacts,
    value: ValueId,
    formal: crate::p2_flow::DeclarationType,
    kinds: &[Option<Kind>],
    bits: &[u8],
    ranges: &[Option<IntRange>],
) -> Result<Option<UseConversion>, RequestError> {
    use crate::p2_flow::DeclarationType;
    let index = usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?;
    if formal.field_offset() == 2 {
        return super::references::view(kinds, zeros, value, formal.kind());
    }
    Ok(match formal {
        DeclarationType::Boolean => match kinds[index] {
            Some(Kind::Boolean) => Some(UseConversion::Identity),
            Some(Kind::Int) if bits[index] != 0 && bits[index] & 4 == 0 => {
                Some(UseConversion::Boolean01)
            }
            _ => None,
        },
        DeclarationType::Narrow(kind) => {
            let range = if bits[index] != 0 && bits[index] & 4 == 0 {
                ranges[index].and_then(IntRange::known01)
            } else {
                ranges[index]
            };
            if kinds[index] == Some(Kind::Int)
                && range.is_some_and(|range| range.within(kind.range()))
            {
                Some(UseConversion::Narrow(kind))
            } else {
                None
            }
        }
        _ => super::numeric::view(ssa, kinds, value, formal.kind())?,
    })
}
pub(crate) fn solve(
    ssa: &SsaBody<'_>,
    zeros: &super::zero::ZeroFacts,
    initializations: &super::initialization::Initializations,
    kinds: &[Option<Kind>],
    bits: &[u8],
    ranges: &[Option<IntRange>],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<CallUses>>, RequestError> {
    let mut uses = Vec::new();
    for (index, site) in ssa.calls.iter().enumerate() {
        budget.checkpoint()?;
        let id = CallId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let bound = ssa.call(id)?;
        let (receiver, values) = if site.kind == crate::p2_flow::InvokeKind::Static {
            (None, bound.inputs.as_slice())
        } else {
            let Some((&value, arguments)) = bound.inputs.split_first() else {
                return Err(RequestError::InternalInvariant(
                    "receiver call lost receiver",
                ));
            };
            let receiver = if site.target == crate::p2_flow::CallTarget::Constructor {
                let initialization = initializations.calls.get(index).copied().flatten().ok_or(
                    RequestError::InternalInvariant("constructor use lost initialization product"),
                )?;
                TypedReceiver::Constructor {
                    value,
                    initialization,
                }
            } else if !initializations.available_at(ssa, value, site.pc)? {
                return Ok(None);
            } else if site.kind == crate::p2_flow::InvokeKind::Super {
                if !is_this(ssa, value, budget)? {
                    return Ok(None);
                }
                TypedReceiver::Super { value }
            } else {
                let Some(conversion) = super::references::read(kinds, zeros, value)? else {
                    return Ok(None);
                };
                TypedReceiver::Value(TypedUse { value, conversion })
            };
            (Some(receiver), arguments)
        };
        let formals = &site.formals;
        if values.len() != formals.len() {
            return Err(RequestError::InternalInvariant(
                "original formal word constraints changed",
            ));
        }
        let mut arguments = Vec::new();
        for (&value, &formal) in values.iter().zip(formals) {
            budget.checkpoint()?;
            if formal.field_offset() == 2 && !initializations.available_at(ssa, value, site.pc)? {
                return Ok(None);
            }
            let Some(conversion) = conversion(ssa, zeros, value, formal, kinds, bits, ranges)?
            else {
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

pub(super) fn is_this(
    ssa: &SsaBody<'_>,
    root: ValueId,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    if !matches!(
        ssa.frame,
        crate::p2_flow::MethodFrame::Instance(_) | crate::p2_flow::MethodFrame::Constructor(_)
    ) {
        return Ok(false);
    }
    let mut seen = Vec::new();
    for _ in &ssa.values {
        budget.checkpoint()?;
        push(&mut seen, false, budget)?;
    }
    let mut phi_indices = Vec::new();
    for _ in &ssa.values {
        budget.checkpoint()?;
        push(&mut phi_indices, None, budget)?;
    }
    for (index, phi) in ssa.phis.iter().enumerate() {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        let index = usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?;
        if seen[index] {
            continue;
        }
        seen[index] = true;
        match ssa.values[index] {
            crate::p3_ssa::Definition::Parameter(0) => anchored = true,
            crate::p3_ssa::Definition::OpResult(id) => {
                let op = ssa.operation(id)?;
                if !matches!(op.op, crate::p2_flow::Op::Move(..)) {
                    return Ok(false);
                }
                push(&mut pending, op.input(0)?, budget)?;
            }
            crate::p3_ssa::Definition::Phi => {
                let phi_index = phi_indices[index]
                    .ok_or(RequestError::InternalInvariant("this origin lost Phi"))?;
                for &(_, source) in &ssa.phis[phi_index].inputs {
                    budget.checkpoint()?;
                    let crate::p3_ssa::Sym::Value(source) = source else {
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
