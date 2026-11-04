use crate::RequestError;
use crate::input::{DexString, Prototype};
use crate::pipeline::control::InputControl;

pub(crate) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    control: &mut impl InputControl,
) -> Result<(), RequestError> {
    let needed = values
        .len()
        .checked_add(additional)
        .ok_or(RequestError::PlatformCapacity)?;
    if needed <= values.capacity() {
        return Ok(());
    }
    let size = size_of::<T>();
    if size == 0 {
        return Err(RequestError::InternalInvariant(
            "owned result has zero-sized records",
        ));
    }
    let maximum = (isize::MAX as usize) / size;
    if needed > maximum {
        return Err(RequestError::PlatformCapacity);
    }
    let capacity = values
        .capacity()
        .checked_mul(2)
        .filter(|&n| n <= maximum)
        .unwrap_or(needed)
        .max(needed);
    control.checkpoint()?;
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(RequestError::Allocation)?;
    if values.capacity() != capacity {
        return Err(RequestError::InternalInvariant(
            "owned result reservation changed capacity",
        ));
    }
    control.checkpoint()?;
    Ok(())
}

pub(crate) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    control: &mut impl InputControl,
) -> Result<(), RequestError> {
    reserve(values, 1, control)?;
    values.push(value);
    Ok(())
}

pub(crate) fn copy_units(
    value: DexString<'_>,
    control: &mut impl InputControl,
) -> Result<Vec<u16>, RequestError> {
    let mut result = Vec::new();
    reserve(
        &mut result,
        usize::try_from(value.utf16_len().0).map_err(|_| RequestError::PlatformCapacity)?,
        control,
    )?;
    let mut units = value.reader()?;
    while let Some(unit) = units.next(control)? {
        result.push(unit);
    }
    Ok(result)
}

pub(crate) fn copy_proto(
    value: &Prototype<'_, '_>,
    control: &mut impl InputControl,
) -> Result<Vec<u16>, RequestError> {
    let mut result = Vec::new();
    reserve(&mut result, 1, control)?;
    result.push(u16::from(b'('));
    value.visit_parameter_types(control, |name, control| -> Result<(), RequestError> {
        reserve(
            &mut result,
            usize::try_from(name.utf16_len().0).map_err(|_| RequestError::PlatformCapacity)?,
            control,
        )?;
        let mut units = name.reader()?;
        while let Some(unit) = units.next(control)? {
            result.push(unit);
        }
        Ok(())
    })?;
    reserve(&mut result, 1, control)?;
    result.push(u16::from(b')'));
    reserve(
        &mut result,
        usize::try_from(value.return_type.utf16_len().0)
            .map_err(|_| RequestError::PlatformCapacity)?,
        control,
    )?;
    let mut units = value.return_type.reader()?;
    while let Some(unit) = units.next(control)? {
        result.push(unit);
    }
    Ok(result)
}

/// Converts an ASCII source literal to exact UTF-16 units at compile time.
pub(crate) const fn ascii_units<const N: usize>(bytes: [u8; N]) -> [u16; N] {
    let mut units = [0; N];
    let mut index = 0;
    while index < N {
        units[index] = bytes[index] as u16;
        index += 1;
    }
    units
}
