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

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum BuiltinReference {
    Object,
    String,
}
impl BuiltinReference {
    pub(crate) fn descriptor(self) -> &'static [u16] {
        match self {
            Self::Object => &[
                76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
            ],
            Self::String => &[
                76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116, 114, 105, 110, 103, 59,
            ],
        }
    }
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Object => "java.lang.Object",
            Self::String => "java.lang.String",
        }
    }
}
