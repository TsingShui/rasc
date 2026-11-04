use super::normal::{Kind, UseConversion};
use super::zero::ZeroFacts;
use crate::RequestError;
use crate::p3_ssa::ValueId;

/// Reads a reference value or an independently proven null literal. This does
/// not establish its subtype, accessibility or initialization state.
pub(super) fn read(
    kinds: &[Option<Kind>],
    zeros: &ZeroFacts,
    source: ValueId,
) -> Result<Option<UseConversion>, RequestError> {
    if zeros.contains(source)? {
        return Ok(Some(UseConversion::NullAssignment));
    }
    let at = usize::try_from(source.0).map_err(|_| RequestError::PlatformCapacity)?;
    let kind = kinds.get(at).ok_or(RequestError::InternalInvariant(
        "foreign reference use value",
    ))?;
    Ok(kind
        .filter(|kind| super::reference_roots::reference(*kind))
        .map(|_| UseConversion::Identity))
}

/// Selects a value-preserving source view, not a subtype or access proof. An
/// unknown source binding keeps the original reference without inserting a cast.
pub(super) fn view(
    kinds: &[Option<Kind>],
    zeros: &ZeroFacts,
    source: ValueId,
    target: Kind,
) -> Result<Option<UseConversion>, RequestError> {
    if !super::reference_roots::reference(target) {
        return Ok(None);
    }
    let Some(conversion) = read(kinds, zeros, source)? else {
        return Ok(None);
    };
    let at = usize::try_from(source.0).map_err(|_| RequestError::PlatformCapacity)?;
    Ok(Some(
        if conversion == UseConversion::Identity
            && target == Kind::Object
            && kinds[at] != Some(Kind::Object)
        {
            UseConversion::UpcastObject
        } else {
            conversion
        },
    ))
}
