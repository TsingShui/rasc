use super::formats::DexVersion;
use super::{
    Cursor, InputError, InputProblem, ProgramIndex, UnsupportedInput, checked_range, file_length,
    file_offset, malformed,
};
use crate::pipeline::control::InputControl;
use crate::{ByteLength, ByteOffset, ByteRange, RunContext};

/// A checked physical header chain borrowing the original input. Construction
/// checks every header and the physical image limit, without allocating or
/// validating the logical members' maps, declarations or code.
#[derive(Clone, Copy, Debug)]
pub struct DexContainer<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) version: DexVersion,
}

impl<'a> DexContainer<'a> {
    /// Recognizes supported little-endian DEX versions and validates the whole
    /// header chain. Errors retain original physical offsets. Work checks
    /// cancellation per read; no logical member copies or input-byte sums occur.
    pub fn open(bytes: &'a [u8], control: &mut RunContext<'_>) -> Result<Self, InputError> {
        Self::open_with(bytes, control)
    }

    pub(crate) fn open_with(
        bytes: &'a [u8],
        control: &mut impl InputControl,
    ) -> Result<Self, InputError> {
        let mut cursor = Cursor::new(bytes);
        let magic = cursor.read_array::<8>(control)?;
        let version = match &magic {
            b"dex\n035\0" => DexVersion::V035,
            b"dex\n037\0" => DexVersion::V037,
            b"dex\n038\0" => DexVersion::V038,
            b"dex\n039\0" => DexVersion::V039,
            b"dex\n040\0" => DexVersion::V040,
            b"dex\n041\0" => DexVersion::V041,
            _ => {
                return Err(InputError::Unsupported {
                    reason: UnsupportedInput::Container,
                    offset_byte: ByteOffset(0),
                });
            }
        };
        let container = Self { bytes, version };
        checked_range(
            ByteLength(bytes.len()),
            ByteOffset(0),
            1,
            container.header_size(),
        )?;
        control.check_image_bytes(u64::try_from(bytes.len()).map_err(|_| {
            InputError::PlatformCapacity {
                offset_byte: ByteOffset(0x20),
            }
        })?)?;
        let mut header = ByteOffset(0);
        loop {
            let range = checked_range(ByteLength(bytes.len()), header, 1, container.header_size())?;
            let mut cursor = Cursor::bounded(bytes, header, range.end)?;
            if cursor.read_array::<8>(control)? != magic {
                return Err(malformed(InputProblem::InvalidHeaderChain, header));
            }
            cursor.skip(1, ByteLength(24), control)?;
            let step = file_length(cursor.read_u32_with(control)?, ByteOffset(header.0 + 0x20))?;
            if file_length(cursor.read_u32_with(control)?, ByteOffset(header.0 + 0x24))?
                != container.header_size()
            {
                return Err(malformed(
                    InputProblem::InvalidHeaderSize,
                    ByteOffset(header.0 + 0x24),
                ));
            }
            match cursor.read_u32_with(control)? {
                0x1234_5678 => {}
                0x7856_3412 => {
                    return Err(InputError::Unsupported {
                        reason: UnsupportedInput::ReverseEndian,
                        offset_byte: ByteOffset(header.0 + 0x28),
                    });
                }
                _ => {
                    return Err(malformed(
                        InputProblem::InvalidEndianTag,
                        ByteOffset(header.0 + 0x28),
                    ));
                }
            }
            if version != DexVersion::V041 {
                if step != ByteLength(bytes.len()) {
                    return Err(malformed(InputProblem::FileSizeMismatch, ByteOffset(0x20)));
                }
                break;
            }
            cursor.skip(1, ByteLength(0x44), control)?;
            let size = file_length(cursor.read_u32_with(control)?, ByteOffset(header.0 + 0x70))?;
            let recorded =
                file_offset(cursor.read_u32_with(control)?, ByteOffset(header.0 + 0x74))?;
            if size != ByteLength(bytes.len())
                || recorded != header
                || step < container.header_size()
            {
                return Err(malformed(InputProblem::InvalidHeaderChain, header));
            }
            let next = header
                .checked_advance(step)
                .ok_or_else(|| malformed(InputProblem::RangeOverflow, header))?;
            if next.0 > bytes.len() || (next.0 != bytes.len() && !next.0.is_multiple_of(4)) {
                return Err(malformed(InputProblem::InvalidHeaderChain, header));
            }
            if next.0 == bytes.len() {
                break;
            }
            header = next;
        }
        Ok(container)
    }

    /// Selects an actual logical header in this checked container. Original
    /// offsets and table namespaces are retained. Invalid selection or map data
    /// is a typed input error; resources and cancellation stop the operation.
    pub fn program(
        &self,
        header_offset: ByteOffset,
        control: &mut RunContext<'_>,
    ) -> Result<ProgramIndex<'a>, InputError> {
        let mut header = ByteOffset(0);
        loop {
            control.charge_work(1)?;
            if header == header_offset {
                return ProgramIndex::from_container(*self, header, control);
            }
            let next = self.next_header(header, control)?;
            if next.0 >= self.bytes.len() || next > header_offset {
                return Err(malformed(InputProblem::InvalidReference, header_offset));
            }
            header = next;
        }
    }

    /// Original logical header offsets in physical order.
    ///
    /// A 041 container can hold several logical DEX members inside one physical
    /// entry; an ordinary container yields exactly one offset at zero. The chain
    /// was validated by `open`, so this only walks the recorded steps and charges
    /// the request ledger; it allocates the offset list and no member copy.
    pub fn logical_headers(
        &self,
        control: &mut RunContext<'_>,
    ) -> Result<Vec<ByteOffset>, InputError> {
        let mut headers = Vec::new();
        let mut header = ByteOffset(0);
        loop {
            control.charge_work(1)?;
            headers.try_reserve(1).map_err(|_| InputError::Allocation {
                offset_byte: header,
            })?;
            headers.push(header);
            let next = self.next_header(header, control)?;
            if next.0 >= self.bytes.len() {
                break;
            }
            header = next;
        }
        Ok(headers)
    }

    pub(super) fn header_size(&self) -> ByteLength {
        ByteLength(if self.version == DexVersion::V041 {
            0x78
        } else {
            0x70
        })
    }

    fn next_header(
        &self,
        header: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<ByteOffset, InputError> {
        if self.version != DexVersion::V041 {
            return Ok(ByteOffset(self.bytes.len()));
        }
        let mut cursor = Cursor::at(self.bytes, ByteOffset(header.0 + 0x20))?;
        let step = file_length(cursor.read_u32_with(control)?, cursor.position_byte())?;
        header
            .checked_advance(step)
            .ok_or_else(|| malformed(InputProblem::RangeOverflow, header))
    }

    /// Caps one original item's read at the next header, even if its section
    /// spans several logical members. Shared data after that header stays usable
    /// through its own original offset; a header itself is never item data.
    pub(super) fn item_end(
        &self,
        offset: ByteOffset,
        control: &mut impl InputControl,
    ) -> Result<ByteOffset, InputError> {
        let mut header = ByteOffset(0);
        loop {
            control.charge_work(1)?;
            if offset < header {
                return Ok(header);
            }
            let end = header
                .checked_advance(self.header_size())
                .ok_or_else(|| malformed(InputProblem::RangeOverflow, header))?;
            if offset < end {
                return Err(malformed(InputProblem::OverlappingSections, offset));
            }
            let next = self.next_header(header, control)?;
            if next.0 == self.bytes.len() {
                return Ok(next);
            }
            header = next;
        }
    }

    pub(super) fn check_data_range(
        &self,
        range: ByteRange,
        control: &mut impl InputControl,
    ) -> Result<(), InputError> {
        if range.start != range.end && range.end > self.item_end(range.start, control)? {
            return Err(malformed(InputProblem::OverlappingSections, range.start));
        }
        Ok(())
    }
}
