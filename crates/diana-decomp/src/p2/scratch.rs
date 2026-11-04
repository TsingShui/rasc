//! Fallible scratch allocation and interruptible in-place ordering for P2.

use super::FlowError;
use crate::RunContext;

pub(super) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    control: &mut RunContext<'_>,
) -> Result<(), FlowError> {
    crate::substrate::owned::push(values, value, control).map_err(super::build::request_error)
}

pub(super) fn filled<T: Copy>(
    count: usize,
    value: T,
    control: &mut RunContext<'_>,
) -> Result<Vec<T>, FlowError> {
    let mut values = Vec::new();
    crate::substrate::owned::reserve(&mut values, count, control)
        .map_err(super::build::request_error)?;
    for _ in 0..count {
        control.checkpoint()?;
        values.push(value);
    }
    Ok(values)
}

// Heapsort needs no allocation and provides cancellation checkpoints within
// the ordering work, rather than only before/after a potentially large sort.
pub(super) fn sort_by_key<T, K: Ord>(
    values: &mut [T],
    key: impl Fn(&T) -> K,
    control: &mut RunContext<'_>,
) -> Result<(), FlowError> {
    for root in (0..values.len() / 2).rev() {
        sift(values, root, values.len(), &key, control)?;
    }
    for end in (1..values.len()).rev() {
        control.checkpoint()?;
        values.swap(0, end);
        sift(values, 0, end, &key, control)?;
    }
    control.checkpoint()?;
    Ok(())
}

fn sift<T, K: Ord>(
    values: &mut [T],
    mut root: usize,
    end: usize,
    key: &impl Fn(&T) -> K,
    control: &mut RunContext<'_>,
) -> Result<(), FlowError> {
    // root < end/2 (rounded up) proves 2*root+1 is within end and cannot overflow.
    while root < end / 2 {
        control.checkpoint()?;
        let left = root * 2 + 1;
        let right = left + 1;
        let child = if right < end && key(&values[right]) > key(&values[left]) {
            right
        } else {
            left
        };
        if key(&values[root]) >= key(&values[child]) {
            return Ok(());
        }
        values.swap(root, child);
        root = child;
    }
    Ok(())
}
