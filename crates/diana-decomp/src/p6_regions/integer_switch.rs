use crate::{RequestError, RunContext};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum SwitchForm {
    Cases,
    Comparisons,
}
pub(crate) fn choose<T>(
    cases: &[(i32, T)],
    budget: &mut RunContext<'_>,
) -> Result<SwitchForm, RequestError> {
    let mut bounds = None;
    for &(key, _) in cases {
        budget.checkpoint()?;
        bounds = Some(match bounds {
            None => (key, key),
            Some((low, high)) => (i32::min(low, key), i32::max(high, key)),
        });
    }
    let range = match bounds {
        None => 0,
        Some((low, high)) => i64::from(high)
            .checked_sub(i64::from(low))
            .and_then(|value| value.checked_add(1))
            .ok_or(RequestError::PlatformCapacity)?,
    };
    let threshold = i64::try_from(cases.len())
        .map_err(|_| RequestError::PlatformCapacity)?
        .checked_mul(4)
        .and_then(|value| value.checked_add(16))
        .ok_or(RequestError::PlatformCapacity)?;
    Ok(if range <= threshold {
        SwitchForm::Cases
    } else {
        SwitchForm::Comparisons
    })
}
