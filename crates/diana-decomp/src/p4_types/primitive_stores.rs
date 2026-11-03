use super::normal::{Kind, UseConversion};
use crate::RequestError;
use crate::p2_flow::DeclarationType;
use crate::p3_ssa::{SsaBody, ValueId};

pub(super) fn view(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    bits: &[u8],
    value: ValueId,
    target: DeclarationType,
) -> Result<Option<UseConversion>, RequestError> {
    let at = usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?;
    Ok(match target {
        DeclarationType::Boolean => match kinds[at] {
            Some(Kind::Boolean) => Some(UseConversion::Identity),
            Some(Kind::Int) if bits[at] != 0 && bits[at] & !3 == 0 => {
                Some(UseConversion::Boolean01)
            }
            _ => None,
        },
        DeclarationType::Narrow(kind) => match kinds[at] {
            Some(Kind::Int) => Some(UseConversion::Narrow(kind)),
            _ => None,
        },
        _ => super::numeric::view(ssa, kinds, value, target.kind())?,
    })
}
