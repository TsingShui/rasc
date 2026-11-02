use super::normal::{Kind, UseConversion};
use crate::RequestError;
use crate::p2_flow::normal::DeclarationType;
use crate::p3_ssa::normal::{SsaBody, ValueId};

pub(super) fn store_view(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    source: ValueId,
    target: DeclarationType,
) -> Result<Option<UseConversion>, RequestError> {
    if target.field_offset() != 2 {
        return Ok(None);
    }
    if ssa.is_zero(source)? {
        return Ok(Some(UseConversion::NullAssignment));
    }
    let at = usize::try_from(source.0).map_err(|_| RequestError::PlatformCapacity)?;
    Ok(match kinds[at] {
        Some(kind) if kind == target.kind() => Some(UseConversion::Identity),
        Some(
            Kind::String
            | Kind::Nominal(_)
            | Kind::ReferenceArray(_)
            | Kind::ProjectedNominal(_)
            | Kind::PrimitiveArray(_)
            | Kind::ObjectArray,
        ) if target.kind() == Kind::Object => Some(UseConversion::UpcastObject),
        _ => None,
    })
}
