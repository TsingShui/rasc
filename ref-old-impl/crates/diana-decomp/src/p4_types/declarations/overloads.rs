use crate::context::Context;
use crate::input::{InputClassDeclaration, InputMethodDeclaration};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Primitive {
    Boolean,
    Byte,
    Short,
    Char,
    Int,
    Long,
    Float,
    Double,
}
#[derive(Clone, Copy)]
enum Formal<'a> {
    Primitive(Primitive),
    Reference(&'a [u16]),
}
struct Prototype<'a> {
    parameters: Vec<Formal<'a>>,
    result: Option<Formal<'a>>,
}
fn formal<'a>(
    units: &'a [u16],
    at: &mut usize,
    budget: &mut RunContext<'_>,
) -> Result<Option<Formal<'a>>, RequestError> {
    budget.checkpoint()?;
    let start = *at;
    while units.get(*at) == Some(&91) {
        budget.checkpoint()?;
        *at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    }
    let array = *at != start;
    let Some(&unit) = units.get(*at) else {
        return Ok(None);
    };
    *at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    let primitive = match unit {
        90 => Some(Primitive::Boolean),
        66 => Some(Primitive::Byte),
        83 => Some(Primitive::Short),
        67 => Some(Primitive::Char),
        73 => Some(Primitive::Int),
        74 => Some(Primitive::Long),
        70 => Some(Primitive::Float),
        68 => Some(Primitive::Double),
        _ => None,
    };
    if let Some(primitive) = primitive {
        return Ok(Some(if array {
            Formal::Reference(&units[start..*at])
        } else {
            Formal::Primitive(primitive)
        }));
    }
    if unit != 76 {
        return Ok(None);
    }
    let name = *at;
    while let Some(&unit) = units.get(*at) {
        budget.checkpoint()?;
        if unit == 59 {
            break;
        }
        if matches!(unit, 0 | 40 | 41 | 46 | 91) {
            return Ok(None);
        }
        *at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    }
    if *at == name || units.get(*at) != Some(&59) {
        return Ok(None);
    }
    *at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    Ok(Some(Formal::Reference(&units[start..*at])))
}
fn parameters<'a>(
    proto: &'a [u16],
    budget: &mut RunContext<'_>,
) -> Result<Option<Prototype<'a>>, RequestError> {
    if proto.first() != Some(&40) {
        return Ok(None);
    }
    let mut at = 1usize;
    let mut result = Vec::new();
    while proto.get(at) != Some(&41) {
        budget.checkpoint()?;
        let Some(kind) = formal(proto, &mut at, budget)? else {
            return Ok(None);
        };
        push(&mut result, kind, budget)?;
    }
    at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    let returned = if proto.get(at) == Some(&86) {
        at = at.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        None
    } else {
        let Some(kind) = formal(proto, &mut at, budget)? else {
            return Ok(None);
        };
        Some(kind)
    };
    Ok(if at == proto.len() {
        Some(Prototype {
            parameters: result,
            result: returned,
        })
    } else {
        None
    })
}
fn same(a: Formal<'_>, b: Formal<'_>, budget: &mut RunContext<'_>) -> Result<bool, RequestError> {
    Ok(match (a, b) {
        (Formal::Primitive(a), Formal::Primitive(b)) => a == b,
        (Formal::Reference(a), Formal::Reference(b)) => crate::context::same_units(a, b, budget)?,
        _ => false,
    })
}
fn same_parameters(
    a: &[Formal<'_>],
    b: &[Formal<'_>],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (&a, &b) in a.iter().zip(b) {
        budget.checkpoint()?;
        if !same(a, b, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn widens(
    catalog: &Context,
    from: Formal<'_>,
    to: Formal<'_>,
    budget: &mut RunContext<'_>,
) -> Result<Option<bool>, RequestError> {
    if same(from, to, budget)? {
        return Ok(Some(true));
    }
    use Primitive::*;
    Ok(match (from, to) {
        (Formal::Primitive(from), Formal::Primitive(to)) => Some(matches!(
            (from, to),
            (Byte, Short | Int | Long | Float | Double)
                | (Short | Char, Int | Long | Float | Double)
                | (Int, Long | Float | Double)
                | (Long, Float | Double)
                | (Float, Double)
        )),
        (Formal::Reference(from), Formal::Reference(to)) => {
            if from.first() == Some(&91) {
                if crate::context::same_units(
                    to,
                    crate::substrate::primitive_types::BuiltinReference::Object.descriptor(),
                    budget,
                )? {
                    Some(true)
                } else {
                    None
                }
            } else if to.first() == Some(&91) {
                Some(false)
            } else {
                super::hierarchy::subtype(catalog, from, to, budget)?
            }
        }
        _ => Some(false),
    })
}
fn returns(
    catalog: &Context,
    from: Option<Formal<'_>>,
    to: Option<Formal<'_>>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    match (from, to) {
        (None, None) => Ok(true),
        (Some(a), Some(b)) if same(a, b, budget)? => Ok(true),
        (Some(a @ Formal::Reference(_)), Some(b @ Formal::Reference(_))) => {
            Ok(widens(catalog, a, b, budget)? == Some(true))
        }
        _ => Ok(false),
    }
}

#[derive(Clone, Copy)]
pub(super) enum Scope<'a> {
    Constructor {
        same_class: bool,
    },
    Static,
    EnclosingStatic {
        same_class: bool,
    },
    Receiver {
        source: &'a InputClassDeclaration,
        direct: bool,
    },
}
// A bridge is hidden from source overload selection only with a same-class
// implementation and an inherited declaration proving the covariant override.
fn covariant_bridge(
    catalog: &Context,
    owner: &InputClassDeclaration,
    method: &InputMethodDeclaration,
    classes: &[&InputClassDeclaration],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    if method.access_flags & (crate::substrate::flags::SYNTHETIC | crate::substrate::flags::BRIDGE)
        != (crate::substrate::flags::SYNTHETIC | crate::substrate::flags::BRIDGE)
        || method.access_flags
            & (2 | crate::substrate::flags::PROTECTED
                | crate::substrate::flags::STATIC
                | crate::substrate::flags::VARARGS)
            != 0
        || method.generic
        || owner.generic
    {
        return Ok(false);
    }
    let Some(bridge) = parameters(&method.prototype, budget)? else {
        return Ok(false);
    };
    let mut implementations = 0usize;
    for candidate in &owner.methods {
        budget.checkpoint()?;
        if candidate.access_flags
            & (2 | crate::substrate::flags::PROTECTED
                | crate::substrate::flags::STATIC
                | (crate::substrate::flags::SYNTHETIC | crate::substrate::flags::BRIDGE))
            != 0
            || candidate.access_flags & crate::substrate::flags::PUBLIC == 0
            || candidate.generic
            || !crate::context::same_units(&candidate.name, &method.name, budget)?
        {
            continue;
        }
        let Some(proto) = parameters(&candidate.prototype, budget)? else {
            return Ok(false);
        };
        if same_parameters(&bridge.parameters, &proto.parameters, budget)?
            && returns(catalog, proto.result, bridge.result, budget)?
        {
            implementations = implementations
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
        }
    }
    if implementations != 1 {
        return Ok(false);
    }
    for ancestor in classes {
        budget.checkpoint()?;
        if std::ptr::eq(*ancestor, owner)
            || super::hierarchy::subtype(catalog, &owner.descriptor, &ancestor.descriptor, budget)?
                != Some(true)
        {
            continue;
        }
        for declaration in &ancestor.methods {
            budget.checkpoint()?;
            if declaration.access_flags
                & (1 | crate::substrate::flags::PRIVATE
                    | crate::substrate::flags::STATIC
                    | crate::substrate::flags::BRIDGE)
                == crate::substrate::flags::PUBLIC
                && !declaration.generic
                && !ancestor.generic
                && crate::context::same_units(&declaration.name, &method.name, budget)?
                && crate::context::same_units(&declaration.prototype, &method.prototype, budget)?
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
/// Exact target-formal source types make the nongeneric target at least as
/// specific as every widening-applicable different vector. Equal vectors need
/// independent override/bridge evidence; they are not discarded by arrival order.
#[allow(dead_code)]
pub(super) fn unique_target(
    catalog: &Context,
    class: &InputClassDeclaration,
    caller: &[u16],
    name: &[u16],
    target: &[u16],
    scope: Scope<'_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let _span = crate::pipeline::phase_span("p4_overloads");
    if !matches!(
        class.coverage,
        crate::input::DeclarationCoverage::AllDeclarations
    ) {
        return Ok(false);
    }
    let Some(fixed) = parameters(target, budget)? else {
        return Ok(false);
    };
    let mut selected = None;
    for method in &class.methods {
        budget.checkpoint()?;
        if crate::context::same_units(&method.name, name, budget)?
            && crate::context::same_units(&method.prototype, target, budget)?
            && selected.replace(method).is_some()
        {
            return Ok(false);
        }
    }
    let Some(selected) = selected else {
        return Ok(false);
    };
    if !super::raw_signature::object_identity(selected, budget)?
        || selected.access_flags
            & (crate::substrate::flags::BRIDGE | crate::substrate::flags::VARARGS)
            != 0
    {
        return Ok(false);
    }
    let is_static = matches!(scope, Scope::Static | Scope::EnclosingStatic { .. });
    let same_class = matches!(
        scope,
        Scope::Constructor { same_class: true }
            | Scope::Receiver { direct: true, .. }
            | Scope::EnclosingStatic { same_class: true }
    );
    let hierarchy;
    let declared = [class];
    let classes = match scope {
        Scope::Receiver { source, .. } => {
            let Some(found) = super::hierarchy::collect(catalog, source, budget)? else {
                return Ok(false);
            };
            hierarchy = found;
            hierarchy.declarations()
        }
        Scope::Constructor { .. } => {
            if fixed.result.is_some()
                || super::hierarchy::collect(catalog, class, budget)?.is_none()
            {
                return Ok(false);
            }
            &declared
        }
        Scope::Static | Scope::EnclosingStatic { .. } => &declared,
    };
    let mut owner_present = false;
    for owner in classes {
        budget.checkpoint()?;
        owner_present |= std::ptr::eq(*owner, class);
        for (position, method) in owner.methods.iter().enumerate() {
            budget.checkpoint()?;
            if !crate::context::same_units(&method.name, name, budget)? {
                continue;
            }
            if !super::contracts::member_directory(owner, caller, budget)? {
                return Ok(false);
            }
            let Some(candidate) = parameters(&method.prototype, budget)? else {
                return Ok(false);
            };
            if candidate.parameters.len() != fixed.parameters.len() {
                continue;
            }
            let mut applicable = true;
            for (&from, &to) in fixed.parameters.iter().zip(&candidate.parameters) {
                budget.checkpoint()?;
                match widens(catalog, from, to, budget)? {
                    Some(true) => {}
                    Some(false) => {
                        applicable = false;
                        break;
                    }
                    None => return Ok(false),
                }
            }
            if !applicable {
                continue;
            }
            if method.access_flags & crate::substrate::flags::BRIDGE != 0 {
                if !matches!(scope, Scope::Receiver { .. })
                    || !covariant_bridge(catalog, owner, method, classes, budget)?
                {
                    return Ok(false);
                }
                continue;
            }
            if method.generic
                && (!std::ptr::eq(method, selected)
                    || !super::raw_signature::object_identity(method, budget)?)
                || method.access_flags & crate::substrate::flags::VARARGS != 0
                || (method.access_flags & crate::substrate::flags::STATIC != 0) != is_static
                || method.access_flags & crate::substrate::flags::PUBLIC == 0
                    && !(same_class && std::ptr::eq(*owner, class)
                        || matches!(scope, Scope::EnclosingStatic { .. })
                            && method.access_flags
                                & (crate::substrate::flags::PUBLIC
                                    | crate::substrate::flags::PRIVATE
                                    | crate::substrate::flags::PROTECTED)
                                == 0)
            {
                return Ok(false);
            }
            for earlier in &owner.methods[..position] {
                budget.checkpoint()?;
                if earlier.access_flags & crate::substrate::flags::BRIDGE != 0
                    || !crate::context::same_units(&earlier.name, name, budget)?
                {
                    continue;
                }
                let Some(proto) = parameters(&earlier.prototype, budget)? else {
                    return Ok(false);
                };
                if same_parameters(&candidate.parameters, &proto.parameters, budget)? {
                    return Ok(false);
                }
            }
            if !same_parameters(&fixed.parameters, &candidate.parameters, budget)? {
                continue;
            }
            if std::ptr::eq(method, selected) {
                continue;
            }
            if !matches!(scope, Scope::Receiver { .. })
                || std::ptr::eq(*owner, class)
                || method.access_flags & (2 | crate::substrate::flags::STATIC | 0x10) != 0
            {
                return Ok(false);
            }
            if super::hierarchy::subtype(catalog, &class.descriptor, &owner.descriptor, budget)?
                == Some(true)
            {
                if !returns(catalog, fixed.result, candidate.result, budget)? {
                    return Ok(false);
                }
            } else if super::hierarchy::subtype(
                catalog,
                &owner.descriptor,
                &class.descriptor,
                budget,
            )? == Some(true)
            {
                if selected.access_flags & (2 | crate::substrate::flags::STATIC | 0x10) != 0
                    || !returns(catalog, candidate.result, fixed.result, budget)?
                {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }
    }
    Ok(owner_present)
}
