use super::catalog::ProgramIndex;
use super::strings::TypeShape;
use super::{Cursor, InputError, InputProblem, file_offset, malformed};
use crate::pipeline::control::InputControl;
use crate::{
    ByteLength, ByteOffset, ByteRange, CodeUnitCount, CodeUnitOffset, RegisterWordCount, TypeId,
};

#[derive(Clone, Debug)]
pub enum MethodBody<'a> {
    DeclarationOnly,
    Code(RawMethodView<'a>),
}

/// Range-checked original facts, constructed only by this reader. Instruction
/// boundaries, references and control-flow semantics require later validation.
#[derive(Clone, Debug)]
pub struct RawMethodView<'a> {
    pub(super) source_bytes: &'a [u8],
    pub(super) header_offset: ByteOffset,
    pub(super) version: super::formats::DexVersion,
    pub(super) method_id: crate::MethodId,
    registers_size: u16,
    ins_size: u16,
    outs_size: u16,
    code_units: CodeUnitCount,
    instructions: &'a [u8],
    code_byte_range: ByteRange,
    try_byte_range: ByteRange,
    handler_byte_range: ByteRange,
    debug_info_offset_byte: Option<ByteOffset>,
}

impl<'a> RawMethodView<'a> {
    pub(crate) fn source_bytes(&self) -> &'a [u8] {
        self.source_bytes
    }

    pub(crate) fn version(&self) -> super::formats::DexVersion {
        self.version
    }

    pub fn registers_size(&self) -> u16 {
        self.registers_size
    }
    pub fn ins_size(&self) -> u16 {
        self.ins_size
    }
    pub fn outs_size(&self) -> u16 {
        self.outs_size
    }
    pub fn code_units(&self) -> CodeUnitCount {
        self.code_units
    }
    pub fn instructions(&self) -> &'a [u8] {
        self.instructions
    }
    pub fn code_byte_range(&self) -> ByteRange {
        self.code_byte_range
    }
    pub fn try_byte_range(&self) -> ByteRange {
        self.try_byte_range
    }
    pub fn handler_byte_range(&self) -> ByteRange {
        self.handler_byte_range
    }
    pub fn debug_info_offset_byte(&self) -> Option<ByteOffset> {
        self.debug_info_offset_byte
    }
}

pub(super) fn read_code<'a>(
    index: &ProgramIndex<'a>,
    code_offset: Option<ByteOffset>,
    method_id: crate::MethodId,
    access_flags: u32,
    expected_words: Option<RegisterWordCount>,
    control: &mut impl InputControl,
) -> Result<MethodBody<'a>, InputError> {
    let offset = match (
        code_offset,
        access_flags & (crate::substrate::flags::NATIVE | crate::substrate::flags::ABSTRACT) != 0,
    ) {
        (None, true) => return Ok(MethodBody::DeclarationOnly),
        (Some(offset), false) => offset,
        (None, false) => return Err(malformed(InputProblem::InvalidCode, ByteOffset(0))),
        (Some(offset), true) => return Err(malformed(InputProblem::InvalidCode, offset)),
    };
    let mut cursor = index.item(0x2001, offset, control)?;
    let registers_size = cursor.read_u16_with(control)?;
    let ins_size = cursor.read_u16_with(control)?;
    let outs_size = cursor.read_u16_with(control)?;
    let tries_size = cursor.read_u16_with(control)?;
    let debug_offset = file_offset(
        cursor.read_u32_with(control)?,
        ByteOffset(cursor.position_byte().0 - 4),
    )?;
    let code_units = CodeUnitCount(cursor.read_u32_with(control)?);
    let byte_length = code_units
        .checked_byte_length()
        .ok_or_else(|| malformed(InputProblem::RangeOverflow, ByteOffset(offset.0 + 12)))?;
    let instructions = cursor.skip(1, byte_length, control)?;
    if ins_size > registers_size
        || expected_words.is_some_and(|words| words.0 != u64::from(ins_size))
    {
        return Err(malformed(
            InputProblem::InvalidCode,
            ByteOffset(offset.0 + 2),
        ));
    }
    if debug_offset != ByteOffset(0) {
        index.item(0x2003, debug_offset, control)?;
    }
    if tries_size != 0 && !code_units.0.is_multiple_of(2) && cursor.read_u16_with(control)? != 0 {
        return Err(malformed(
            InputProblem::InvalidCode,
            ByteOffset(cursor.position_byte().0 - 2),
        ));
    }
    let tries = cursor.skip(usize::from(tries_size), ByteLength(8), control)?;
    let handler_start = cursor.position_byte();
    if tries_size != 0 {
        check_handlers(index, &mut cursor, tries, code_units, control)?;
    }
    Ok(MethodBody::Code(RawMethodView {
        source_bytes: index.bytes,
        header_offset: index.header.header_offset,
        version: index.header.version,
        method_id,
        registers_size,
        ins_size,
        outs_size,
        code_units,
        instructions: &index.bytes[instructions.as_range()],
        code_byte_range: ByteRange {
            start: offset,
            end: cursor.position_byte(),
        },
        try_byte_range: tries,
        handler_byte_range: ByteRange {
            start: handler_start,
            end: cursor.position_byte(),
        },
        debug_info_offset_byte: (debug_offset != ByteOffset(0)).then_some(debug_offset),
    }))
}

fn check_handlers(
    index: &ProgramIndex<'_>,
    cursor: &mut Cursor<'_>,
    tries: ByteRange,
    code_units: CodeUnitCount,
    control: &mut impl InputControl,
) -> Result<(), InputError> {
    let list_start = cursor.position_byte();
    let count = cursor.read_uleb32_with(control)?;
    if u64::from(count) > cursor.remaining_byte().0 as u64 {
        return Err(malformed(InputProblem::Truncated, cursor.position_byte()));
    }
    control.charge_metadata(u64::from(count))?;
    // Membership covers the format's u16 relative-byte-offset domain.
    let mut starts = [0u64; 1024];
    for _ in 0..count {
        let relative = cursor
            .position_byte()
            .checked_distance_from(list_start)
            .ok_or_else(|| malformed(InputProblem::RangeOverflow, cursor.position_byte()))?;
        if relative.0 <= usize::from(u16::MAX) {
            starts[relative.0 / 64] |= 1u64 << (relative.0 % 64);
        }
        let size = cursor.read_sleb32_with(control)?;
        let typed = i64::from(size).unsigned_abs();
        let minimum = typed * 2 + u64::from(size <= 0);
        if minimum > cursor.remaining_byte().0 as u64 {
            return Err(malformed(InputProblem::Truncated, cursor.position_byte()));
        }
        control.charge_metadata(typed)?;
        for _ in 0..typed {
            let type_idx = TypeId(cursor.read_uleb32_with(control)?);
            let (_, shape) = index.type_descriptor(type_idx, control)?;
            if !matches!(shape, TypeShape::Class) {
                return Err(malformed(
                    InputProblem::InvalidHandler,
                    cursor.position_byte(),
                ));
            }
            if CodeUnitOffset(cursor.read_uleb32_with(control)?).0 >= code_units.0 {
                return Err(malformed(
                    InputProblem::InvalidHandler,
                    cursor.position_byte(),
                ));
            }
        }
        if size <= 0 && CodeUnitOffset(cursor.read_uleb32_with(control)?).0 >= code_units.0 {
            return Err(malformed(
                InputProblem::InvalidHandler,
                cursor.position_byte(),
            ));
        }
    }
    let mut records = Cursor::bounded(index.bytes, tries.start, tries.end)?;
    let mut previous_end = CodeUnitOffset(0);
    while records.remaining_byte() != ByteLength(0) {
        control.charge_metadata(1)?;
        let start = CodeUnitOffset(records.read_u32_with(control)?);
        let length = CodeUnitCount(u32::from(records.read_u16_with(control)?));
        let offset = usize::from(records.read_u16_with(control)?);
        let end = start.checked_advance(length).ok_or_else(|| {
            malformed(
                InputProblem::RangeOverflow,
                ByteOffset(records.position_byte().0 - 8),
            )
        })?;
        if length == CodeUnitCount(0)
            || start < previous_end
            || end.0 > code_units.0
            || starts[offset / 64] & (1u64 << (offset % 64)) == 0
        {
            return Err(malformed(
                InputProblem::InvalidHandler,
                ByteOffset(records.position_byte().0 - 8),
            ));
        }
        previous_end = end;
    }
    Ok(())
}
