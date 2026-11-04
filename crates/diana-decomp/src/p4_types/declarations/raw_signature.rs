use crate::RequestError;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::push;

pub(crate) fn erases_to(
    signature: &[u16],
    descriptor: &[u16],
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    if signature.first() == Some(&60) {
        let mut colon = None;
        for (index, &unit) in signature.iter().enumerate().skip(1) {
            control.checkpoint()?;
            if unit == 58 {
                colon = Some(index);
                break;
            }
            if !(unit == 95 || (65..=90).contains(&unit) || (97..=122).contains(&unit)) {
                return Ok(false);
            }
        }
        let Some(colon) = colon else { return Ok(false) };
        if colon == 1 {
            return Ok(false);
        }
        let bound = const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Object;") };
        let end = colon
            .checked_add(1)
            .and_then(|at| at.checked_add(bound.len()))
            .ok_or(RequestError::PlatformCapacity)?;
        if signature.get(colon + 1..end) != Some(bound.as_slice())
            || signature.get(end) != Some(&62)
        {
            return Ok(false);
        }
        let variable = &signature[1..colon];
        let mut erased = Vec::new();
        let mut at = end + 1;
        while at < signature.len() {
            control.checkpoint()?;
            if signature[at] == 84 {
                let finish = at
                    .checked_add(1)
                    .and_then(|at| at.checked_add(variable.len()))
                    .ok_or(RequestError::PlatformCapacity)?;
                if signature.get(at + 1..finish) != Some(variable)
                    || signature.get(finish) != Some(&59)
                {
                    return Ok(false);
                }
                for &unit in &bound {
                    push(&mut erased, unit, control)?;
                }
                at = finish + 1;
            } else {
                if !matches!(signature[at], 40 | 41) {
                    return Ok(false);
                }
                push(&mut erased, signature[at], control)?;
                at += 1;
            }
        }
        return Ok(erased == descriptor);
    }
    let mut erased = Vec::new();
    let mut depth = 0usize;
    let mut reference = false;
    let mut argument = false;
    let mut arguments = 0usize;
    for &unit in signature {
        control.checkpoint()?;
        if unit == 60 {
            if !reference {
                return Ok(false);
            }
            if depth != 0 {
                return Ok(false);
            }
            depth = 1;
            arguments = 0;
        } else if unit == 62 {
            if depth != 1 || argument || arguments == 0 {
                return Ok(false);
            }
            depth = 0;
        } else if depth != 0 {
            if !argument {
                if unit != 76 {
                    return Ok(false);
                }
                argument = true;
            } else if unit == 59 {
                argument = false;
                arguments = arguments
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
            } else if !(unit == 47
                || unit == 36
                || unit == 95
                || (48..=57).contains(&unit)
                || (65..=90).contains(&unit)
                || (97..=122).contains(&unit))
            {
                return Ok(false);
            }
        } else if depth == 0 {
            if unit == 84 && !reference {
                return Ok(false);
            }
            if unit == 46 || unit == 94 {
                return Ok(false);
            }
            push(&mut erased, unit, control)?;
            if unit == 76 {
                reference = true;
            }
            if unit == 59 {
                reference = false;
            }
        }
    }
    Ok(depth == 0 && !reference && erased == descriptor)
}

pub(super) fn raw_class(
    class: &crate::input::InputClassDeclaration,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    if !class.generic {
        return Ok(true);
    }
    let Some(signature) = &class.signature else {
        return Ok(false);
    };
    let prefix = const { crate::substrate::owned::ascii_units(*b"<T:Ljava/lang/Object;>") };
    let Some(parent) = &class.parent else {
        return Ok(false);
    };
    if signature.get(..prefix.len()) != Some(prefix.as_slice())
        || signature.get(prefix.len()..) != Some(parent.as_slice())
        || !class.interfaces.is_empty()
    {
        return Ok(false);
    }
    for method in &class.methods {
        control.checkpoint()?;
        if method.generic || method.signature.is_some() {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn object_identity(
    method: &crate::input::InputMethodDeclaration,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    if !method.generic {
        return Ok(true);
    }
    if method.access_flags
        & (1 | crate::substrate::flags::STATIC
            | crate::substrate::flags::BRIDGE
            | crate::substrate::flags::VARARGS)
        != (1 | crate::substrate::flags::STATIC)
    {
        return Ok(false);
    }
    let Some(signature) = &method.signature else {
        return Ok(false);
    };
    if method.prototype.as_slice()
        != const { crate::substrate::owned::ascii_units(*b"(Ljava/lang/Object;)Ljava/lang/Object;") }
    {
        return Ok(false);
    }
    Ok(signature.first() == Some(&60) && erases_to(signature, &method.prototype, control)?)
}

pub(crate) struct RawSignature {
    units: Vec<u16>,
}
impl RawSignature {
    pub(crate) fn units(&self) -> &[u16] {
        &self.units
    }
}

pub(crate) fn collect(
    _permit: &crate::engine::SchedulePermit,
    input: &crate::input::MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    control: &mut impl InputControl,
) -> Result<Option<RawSignature>, RequestError> {
    collect_impl(input, catalog, control)
}
fn collect_impl(
    input: &crate::input::MethodSource<'_, '_>,
    _catalog: &crate::context::Context,
    control: &mut impl InputControl,
) -> Result<Option<RawSignature>, RequestError> {
    let Some(signature) = crate::input::signature::method(input, control)? else {
        return Ok(None);
    };
    let mut units = Vec::new();
    for &unit in signature.units() {
        push(&mut units, unit, control)?;
    }
    Ok(Some(RawSignature { units }))
}
