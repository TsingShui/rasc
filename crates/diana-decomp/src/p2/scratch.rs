//! Fallible P2 scratch allocation.

use super::FlowError;

pub(super) fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), FlowError> {
    crate::substrate::owned::push(values, value).map_err(super::build::request_error)
}

pub(super) fn filled<T: Copy>(count: usize, value: T) -> Result<Vec<T>, FlowError> {
    let mut values = Vec::new();
    crate::substrate::owned::reserve(&mut values, count).map_err(super::build::request_error)?;
    values.resize(count, value);
    Ok(values)
}
