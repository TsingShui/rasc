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

pub(crate) struct TextBuffer {
    text: String,
}
impl TextBuffer {
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.text.len()
    }
    pub fn push(
        &mut self,
        text: &str,
        control: &mut impl InputControl,
    ) -> Result<(), RequestError> {
        let needed = self
            .text
            .len()
            .checked_add(text.len())
            .ok_or(RequestError::PlatformCapacity)?;
        control.checkpoint()?;
        if needed > isize::MAX as usize {
            return Err(RequestError::PlatformCapacity);
        }
        if needed > self.text.capacity() {
            let capacity = self
                .text
                .capacity()
                .checked_mul(2)
                .filter(|&n| n <= isize::MAX as usize)
                .unwrap_or(needed)
                .max(needed);
            control.checkpoint()?;
            self.text
                .try_reserve_exact(capacity - self.text.len())
                .map_err(RequestError::Allocation)?;
            if self.text.capacity() != capacity {
                return Err(RequestError::InternalInvariant(
                    "source reservation changed capacity",
                ));
            }
        }
        let mut remaining = text;
        while !remaining.is_empty() {
            control.checkpoint()?;
            let mut end = remaining.len().min(65536);
            while !remaining.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&remaining[..end]);
            remaining = &remaining[end..];
        }
        Ok(())
    }
    pub fn character(
        &mut self,
        value: char,
        control: &mut impl InputControl,
    ) -> Result<(), RequestError> {
        let mut bytes = [0; 4];
        self.push(value.encode_utf8(&mut bytes), control)
    }
    pub fn unsigned(
        &mut self,
        mut value: u64,
        control: &mut impl InputControl,
    ) -> Result<(), RequestError> {
        let mut bytes = [0u8; 20];
        let mut start = bytes.len();
        loop {
            start -= 1;
            bytes[start] = b'0'
                + u8::try_from(value % 10)
                    .map_err(|_| RequestError::InternalInvariant("decimal digit exceeds u8"))?;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        let text = std::str::from_utf8(&bytes[start..])
            .map_err(|_| RequestError::InternalInvariant("decimal digits are not UTF8"))?;
        self.push(text, control)
    }
    pub fn arguments(
        &mut self,
        arguments: std::fmt::Arguments<'_>,
        control: &mut impl InputControl,
    ) -> Result<(), RequestError> {
        struct Writer<'a, C> {
            output: &'a mut TextBuffer,
            control: &'a mut C,
            error: Option<RequestError>,
        }
        impl<C: InputControl> std::fmt::Write for Writer<'_, C> {
            fn write_str(&mut self, text: &str) -> std::fmt::Result {
                match self.output.push(text, self.control) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        self.error = Some(error);
                        Err(std::fmt::Error)
                    }
                }
            }
        }
        let mut writer = Writer {
            output: self,
            control,
            error: None,
        };
        match std::fmt::Write::write_fmt(&mut writer, arguments) {
            Ok(()) => Ok(()),
            Err(_) => Err(writer.error.unwrap_or(RequestError::InternalInvariant(
                "diagnostic formatting failed without an output error",
            ))),
        }
    }
    pub fn finish(self) -> String {
        self.text
    }
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
