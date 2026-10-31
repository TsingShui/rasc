use crate::substrate::owned::reserve;
use crate::{ExternalClassDeclaration, MethodBudget, RequestError};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Formal {
    Boolean,
    Byte,
    Short,
    Char,
    Int,
    Long,
    Float,
    Double,
    Object,
    String,
}
fn formal(
    units: &[u16],
    at: &mut usize,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Formal>, RequestError> {
    budget.charge_work(1)?;
    let Some(&unit) = units.get(*at) else {
        return Ok(None);
    };
    *at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    Ok(Some(match unit {
        90 => Formal::Boolean,
        66 => Formal::Byte,
        83 => Formal::Short,
        67 => Formal::Char,
        73 => Formal::Int,
        74 => Formal::Long,
        70 => Formal::Float,
        68 => Formal::Double,
        76 => {
            let start = *at - 1;
            while units.get(*at).is_some_and(|&unit| unit != 59) {
                budget.charge_work(1)?;
                *at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            }
            if units.get(*at) != Some(&59) {
                return Ok(None);
            }
            *at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            let name = &units[start..*at];
            use crate::p5_types::BuiltinReference;
            if crate::p0_knowledge::same_units(name, BuiltinReference::Object.descriptor(), budget)?
            {
                Formal::Object
            } else if crate::p0_knowledge::same_units(
                name,
                BuiltinReference::String.descriptor(),
                budget,
            )? {
                Formal::String
            } else {
                return Ok(None);
            }
        }
        _ => return Ok(None),
    }))
}
fn parameters(
    proto: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Formal>>, RequestError> {
    if proto.first() != Some(&40) {
        return Ok(None);
    }
    let mut at = 1usize;
    let mut out = Vec::new();
    while proto.get(at) != Some(&41) {
        budget.charge_work(1)?;
        let Some(kind) = formal(proto, &mut at, budget)? else {
            return Ok(None);
        };
        reserve(&mut out, 1, budget)?;
        out.push(kind);
    }
    at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    if proto.get(at) == Some(&86) {
        at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    } else if formal(proto, &mut at, budget)?.is_none() {
        return Ok(None);
    }
    Ok(if at == proto.len() { Some(out) } else { None })
}
fn widens(from: Formal, to: Formal) -> bool {
    use Formal::*;
    from == to
        || matches!(
            (from, to),
            (Byte, Short | Int | Long | Float | Double)
                | (Short | Char, Int | Long | Float | Double)
                | (Int, Long | Float | Double)
                | (Long, Float | Double)
                | (Float, Double)
                | (String, Object)
        )
}
/// The actual source vector is fixed to the original target formal vector.
/// Nongeneric fixed-arity applicability therefore also establishes that target's
/// specificity against each applicable different vector; equal vectors collide.
pub(super) fn unique_target(
    class: &ExternalClassDeclaration,
    name: &[u16],
    target: &[u16],
    is_static: bool,
    same_class: bool,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let Some(fixed) = parameters(target, budget)? else {
        return Ok(false);
    };
    let mut exact = 0usize;
    for method in &class.methods {
        budget.charge_work(1)?;
        if !crate::p0_knowledge::same_units(&method.name, name, budget)? {
            continue;
        }
        if (method.access_flags & 1 == 0 && !same_class)
            || (method.access_flags & 8 != 0) != is_static
            || method.access_flags & (0x40 | 0x80) != 0
            || method.generic
        {
            return Ok(false);
        }
        let Some(candidate) = parameters(&method.prototype, budget)? else {
            return Ok(false);
        };
        if crate::p0_knowledge::same_units(&method.prototype, target, budget)? {
            exact = exact.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            continue;
        }
        if candidate.len() != fixed.len() {
            continue;
        }
        let mut applicable = true;
        let mut reverse = true;
        let mut identical = true;
        for (&from, &to) in fixed.iter().zip(&candidate) {
            budget.charge_work(1)?;
            applicable &= widens(from, to);
            reverse &= widens(to, from);
            identical &= from == to;
        }
        if identical || (applicable && reverse) {
            return Ok(false);
        }
    }
    Ok(exact == 1)
}
