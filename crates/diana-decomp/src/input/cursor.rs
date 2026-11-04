use super::{InputError, InputProblem, malformed};
use crate::{ByteLength, ByteOffset, ByteRange};

/// Checks count multiplication and end addition before borrowing an input range.
pub fn checked_range(
    input_length: ByteLength,
    offset: ByteOffset,
    count: usize,
    stride: ByteLength,
) -> Result<ByteRange, InputError> {
    let size = count
        .checked_mul(stride.0)
        .ok_or_else(|| malformed(InputProblem::RangeOverflow, offset))?;
    let end = offset
        .checked_advance(ByteLength(size))
        .ok_or_else(|| malformed(InputProblem::RangeOverflow, offset))?;
    if end.0 > input_length.0 {
        return Err(malformed(InputProblem::Truncated, offset));
    }
    Ok(ByteRange { start: offset, end })
}

/// Bounded shared reader. Missing input never becomes a default value.
pub struct Cursor<'a> {
    bytes: &'a [u8],
    position: ByteOffset,
    end: ByteOffset,
}

impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: ByteOffset(0),
            end: ByteOffset(bytes.len()),
        }
    }
    pub(super) fn at(bytes: &'a [u8], offset: ByteOffset) -> Result<Self, InputError> {
        Self::bounded(bytes, offset, ByteOffset(bytes.len()))
    }
    pub(crate) fn bounded(
        bytes: &'a [u8],
        start: ByteOffset,
        end: ByteOffset,
    ) -> Result<Self, InputError> {
        let size = end
            .checked_distance_from(start)
            .ok_or_else(|| malformed(InputProblem::RangeOverflow, start))?;
        checked_range(ByteLength(bytes.len()), start, 1, size)?;
        Ok(Self {
            bytes,
            position: start,
            end,
        })
    }
    pub(crate) fn position_byte(&self) -> ByteOffset {
        self.position
    }
    pub(crate) fn remaining_byte(&self) -> ByteLength {
        ByteLength(self.end.0 - self.position.0)
    }
    pub(super) fn skip(
        &mut self,
        count: usize,
        stride: ByteLength,
    ) -> Result<ByteRange, InputError> {
        let range = checked_range(ByteLength(self.end.0), self.position, count, stride)?;
        self.position = range.end;
        Ok(range)
    }
    pub fn read_u16(&mut self) -> Result<u16, InputError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }
    pub fn read_u32(&mut self) -> Result<u32, InputError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }
    pub fn read_uleb32(&mut self) -> Result<u32, InputError> {
        let start = self.position;
        let mut value = 0u64;
        for index in 0..5 {
            let [byte] = self.read_array()?;
            if index == 4 && byte > 0x0f {
                return Err(malformed(InputProblem::InvalidLeb32, start));
            }
            value |= u64::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                return u32::try_from(value)
                    .map_err(|_| malformed(InputProblem::InvalidLeb32, start));
            }
        }
        Err(malformed(InputProblem::InvalidLeb32, start))
    }
    pub fn read_sleb32(&mut self) -> Result<i32, InputError> {
        let start = self.position;
        let mut value = 0u64;
        for index in 0..5 {
            let [byte] = self.read_array()?;
            let payload = byte & 0x7f;
            if index == 4 && (byte & 0x80 != 0 || (0x08..0x78).contains(&payload)) {
                return Err(malformed(InputProblem::InvalidLeb32, start));
            }
            value |= u64::from(payload) << (index * 7);
            if byte & 0x80 == 0 {
                if byte & 0x40 != 0 {
                    value |= u64::MAX << ((index + 1) * 7);
                }
                let signed = i64::from_le_bytes(value.to_le_bytes());
                return i32::try_from(signed)
                    .map_err(|_| malformed(InputProblem::InvalidLeb32, start));
            }
        }
        Err(malformed(InputProblem::InvalidLeb32, start))
    }
    pub(crate) fn read_array<const N: usize>(&mut self) -> Result<[u8; N], InputError> {
        let range = checked_range(ByteLength(self.end.0), self.position, 1, ByteLength(N))?;
        let mut value = [0; N];
        value.copy_from_slice(&self.bytes[range.as_range()]);
        self.position = range.end;
        Ok(value)
    }
}
