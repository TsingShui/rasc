use crate::context::Context;
use crate::input::{MethodInput, protection::CatchType};
use crate::p2_flow::{BlockId, Op, OpId};
use crate::p3_ssa::SsaBody;
use crate::substrate::owned::{copy_units, push};
use crate::{MethodBudget, RequestError, TypeId};

pub(crate) const THROWABLE: &[u16] =
    &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Throwable;") };

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ExceptionType {
    Nominal(TypeId),
    Throwable,
}
pub(crate) struct ExceptionBinding {
    pub handler: BlockId,
    pub operation: OpId,
    pub kind: Option<ExceptionType>,
}
// Catch binding needs a concrete Throwable superclass chain, not the stricter
// interface-free method-overload directory used by ordinary receiver binding.
pub(crate) fn catch_type(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    name: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    if name.first() != Some(&76) || name.last() != Some(&59) || name.len() < 3 {
        return Ok(false);
    }
    let mut end = name.len() - 1;
    for (index, &unit) in name.iter().enumerate().skip(1) {
        budget.charge_work(1)?;
        if unit == 47 {
            end = index;
            break;
        }
    }
    if !super::qualifier_available(input, catalog, &name[1..end], budget)? {
        return Ok(false);
    }
    let mut current = name;
    let mut seen: Vec<&[u16]> = Vec::new();
    loop {
        budget.charge_work(1)?;
        for previous in &seen {
            budget.charge_work(1)?;
            if crate::context::same_units(previous, current, budget)? {
                return Ok(false);
            }
        }
        let Some(class) = catalog.class(current, budget)? else {
            return Ok(false);
        };
        if class.generic
            || class.enclosing_scope
            || class.access_flags & crate::substrate::flags::PUBLIC == 0
            || class.access_flags
                & (crate::substrate::flags::ENUM
                    | crate::substrate::flags::ANNOTATION
                    | crate::substrate::flags::INTERFACE)
                != 0
        {
            return Ok(false);
        }
        if crate::context::same_units(current, THROWABLE, budget)? {
            return Ok(true);
        }
        push(&mut seen, current, budget)?;
        let Some(parent) = class.parent.as_deref() else {
            return Ok(false);
        };
        current = parent;
    }
}

pub(crate) fn collect(
    input: &MethodInput<'_, '_>,
    catalog: &Context,
    ssa: &SsaBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<ExceptionBinding>, RequestError> {
    let mut result = Vec::new();
    for (index, block) in ssa.bound.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(&first) = block.ops.first() else {
            continue;
        };
        if !matches!(ssa.operation(first)?.op, Op::Exception(_)) {
            continue;
        }
        let handler = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let mut kind = None;
        let mut valid = true;
        for edge in &ssa.exception_edges {
            budget.charge_work(1)?;
            if edge.target != handler {
                continue;
            }
            let incoming = match edge.kind {
                CatchType::Typed(id) => {
                    let name = copy_units(input.type_descriptor(id, budget)?, budget)?;
                    valid &= catch_type(input, catalog, &name, budget)?;
                    ExceptionType::Nominal(id)
                }
                CatchType::All => {
                    valid &= catch_type(input, catalog, THROWABLE, budget)?;
                    ExceptionType::Throwable
                }
            };
            kind = Some(match kind {
                None => incoming,
                Some(old) if old == incoming => old,
                Some(_) => ExceptionType::Throwable,
            });
        }
        if kind == Some(ExceptionType::Throwable) {
            valid &= catch_type(input, catalog, THROWABLE, budget)?;
        }
        push(
            &mut result,
            ExceptionBinding {
                handler,
                operation: first,
                kind: if valid { kind } else { None },
            },
            budget,
        )?;
    }
    Ok(result)
}
