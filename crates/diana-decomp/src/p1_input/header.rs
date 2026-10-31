use super::{
    Cursor, InputError, InputProblem, UnsupportedInput, checked_range, count_usize, file_length,
    file_offset, malformed,
};
use crate::pipeline::control::InputControl;
use crate::{ByteLength, ByteOffset, ByteRange, RunContext};

const HEADER_BYTES: ByteLength = ByteLength(0x70);
pub(super) const SUPPORTED_MAP_KINDS: usize = 18;
const TABLE_STRIDES: [ByteLength; 6] = [
    ByteLength(4),
    ByteLength(4),
    ByteLength(12),
    ByteLength(8),
    ByteLength(8),
    ByteLength(32),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableSection {
    pub count: u32,
    pub offset_byte: ByteOffset,
    pub size_byte: ByteLength,
}

/// Checked 035 fixed sections and map envelope, with original integrity fields.
/// This does not certify variable item extents, references or executable code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DexHeader {
    pub checksum: u32,
    pub signature: [u8; 20],
    pub string_ids: TableSection,
    pub type_ids: TableSection,
    pub proto_ids: TableSection,
    pub field_ids: TableSection,
    pub method_ids: TableSection,
    pub class_defs: TableSection,
    pub link_byte_range: ByteRange,
    pub data_byte_range: ByteRange,
    pub map_byte_range: ByteRange,
}

/// Reads a little-endian 035 envelope without copying bytes or authenticating
/// checksum/signature. Returns typed input, resource and cancellation errors.
pub fn read_header(bytes: &[u8], control: &mut RunContext<'_>) -> Result<DexHeader, InputError> {
    read_header_with(bytes, control)
}

pub(crate) fn read_header_with(
    bytes: &[u8],
    control: &mut impl InputControl,
) -> Result<DexHeader, InputError> {
    let mut cursor = Cursor::new(bytes);
    if cursor.read_array::<8>(control)? != *b"dex\n035\0" {
        return Err(InputError::Unsupported {
            reason: UnsupportedInput::Container,
            offset_byte: ByteOffset(0),
        });
    }
    checked_range(ByteLength(bytes.len()), ByteOffset(0), 1, HEADER_BYTES)?;
    let checksum = cursor.read_u32_with(control)?;
    let signature = cursor.read_array(control)?;
    if file_length(cursor.read_u32_with(control)?, ByteOffset(0x20))? != ByteLength(bytes.len()) {
        return Err(malformed(InputProblem::FileSizeMismatch, ByteOffset(0x20)));
    }
    if file_length(cursor.read_u32_with(control)?, ByteOffset(0x24))? != HEADER_BYTES {
        return Err(malformed(InputProblem::InvalidHeaderSize, ByteOffset(0x24)));
    }
    match cursor.read_u32_with(control)? {
        0x1234_5678 => {}
        0x7856_3412 => {
            return Err(InputError::Unsupported {
                reason: UnsupportedInput::ReverseEndian,
                offset_byte: ByteOffset(0x28),
            });
        }
        _ => return Err(malformed(InputProblem::InvalidEndianTag, ByteOffset(0x28))),
    }
    let image_bytes = u64::try_from(bytes.len()).map_err(|_| InputError::PlatformCapacity {
        offset_byte: ByteOffset(0x20),
    })?;
    control.check_image_bytes(image_bytes)?;
    let link_size = count_usize(cursor.read_u32_with(control)?, ByteOffset(0x2c))?;
    let link_offset = file_offset(cursor.read_u32_with(control)?, ByteOffset(0x30))?;
    let link_byte_range = section(
        ByteLength(bytes.len()),
        link_offset,
        link_size,
        ByteLength(1),
        1,
        ByteOffset(0x2c),
    )?;
    let map_offset = file_offset(cursor.read_u32_with(control)?, ByteOffset(0x34))?;
    let mut tables = [TableSection {
        count: 0,
        offset_byte: ByteOffset(0),
        size_byte: ByteLength(0),
    }; 6];
    for (index, table) in tables.iter_mut().enumerate() {
        let field = ByteOffset(0x38 + index * 8);
        let count = cursor.read_u32_with(control)?;
        let offset = file_offset(cursor.read_u32_with(control)?, ByteOffset(field.0 + 4))?;
        let range = section(
            ByteLength(bytes.len()),
            offset,
            count_usize(count, field)?,
            TABLE_STRIDES[index],
            4,
            field,
        )?;
        if matches!(index, 1 | 2) && count > 65535 {
            return Err(malformed(InputProblem::InvalidTableCount, field));
        }
        *table = TableSection {
            count,
            offset_byte: offset,
            size_byte: ByteLength(range.end.0 - range.start.0),
        };
    }
    let data_size = count_usize(cursor.read_u32_with(control)?, ByteOffset(0x68))?;
    let data_offset = file_offset(cursor.read_u32_with(control)?, ByteOffset(0x6c))?;
    let data_byte_range = section(
        ByteLength(bytes.len()),
        data_offset,
        data_size,
        ByteLength(1),
        4,
        ByteOffset(0x68),
    )?;
    check_fixed_overlap(&tables, link_byte_range, data_byte_range)?;
    if !map_offset.0.is_multiple_of(4) {
        return Err(malformed(InputProblem::InvalidAlignment, ByteOffset(0x34)));
    }
    if map_offset < data_byte_range.start || map_offset >= data_byte_range.end {
        return Err(malformed(InputProblem::InvalidMap, ByteOffset(0x34)));
    }
    let map_byte_range = check_map(bytes, map_offset, data_byte_range, &tables, control)?;
    let [
        string_ids,
        type_ids,
        proto_ids,
        field_ids,
        method_ids,
        class_defs,
    ] = tables;
    Ok(DexHeader {
        checksum,
        signature,
        string_ids,
        type_ids,
        proto_ids,
        field_ids,
        method_ids,
        class_defs,
        link_byte_range,
        data_byte_range,
        map_byte_range,
    })
}

fn section(
    input_length: ByteLength,
    offset: ByteOffset,
    count: usize,
    stride: ByteLength,
    alignment: usize,
    field: ByteOffset,
) -> Result<ByteRange, InputError> {
    if (count == 0) != (offset == ByteOffset(0)) {
        return Err(malformed(InputProblem::InvalidSectionPair, field));
    }
    if !offset.0.is_multiple_of(alignment) {
        return Err(malformed(InputProblem::InvalidAlignment, field));
    }
    let range = checked_range(input_length, offset, count, stride)?;
    if count != 0 && offset.0 < HEADER_BYTES.0 {
        return Err(malformed(InputProblem::OverlappingSections, field));
    }
    Ok(range)
}

fn overlaps(a: ByteRange, b: ByteRange) -> bool {
    a.start != a.end && b.start != b.end && a.start < b.end && b.start < a.end
}

fn table_range(table: TableSection) -> Result<ByteRange, InputError> {
    let end = table
        .offset_byte
        .checked_advance(table.size_byte)
        .ok_or_else(|| malformed(InputProblem::RangeOverflow, table.offset_byte))?;
    Ok(ByteRange {
        start: table.offset_byte,
        end,
    })
}

fn check_fixed_overlap(
    tables: &[TableSection; 6],
    link: ByteRange,
    data: ByteRange,
) -> Result<(), InputError> {
    if overlaps(link, data) {
        return Err(malformed(InputProblem::OverlappingSections, link.start));
    }
    for (index, table) in tables.iter().enumerate() {
        let range = table_range(*table)?;
        if overlaps(range, link) || overlaps(range, data) {
            return Err(malformed(InputProblem::OverlappingSections, range.start));
        }
        for other in &tables[..index] {
            if overlaps(range, table_range(*other)?) {
                return Err(malformed(InputProblem::OverlappingSections, range.start));
            }
        }
    }
    Ok(())
}

fn check_map(
    bytes: &[u8],
    offset: ByteOffset,
    data: ByteRange,
    tables: &[TableSection; 6],
    control: &mut impl InputControl,
) -> Result<ByteRange, InputError> {
    let prefix = checked_range(ByteLength(data.end.0), offset, 1, ByteLength(4))?;
    let mut cursor = Cursor::at(bytes, offset)?;
    let count = cursor.read_u32_with(control)?;
    let entries = checked_range(
        ByteLength(data.end.0),
        prefix.end,
        count_usize(count, offset)?,
        ByteLength(12),
    )?;
    let map_range = ByteRange {
        start: offset,
        end: entries.end,
    };
    control.charge_metadata(u64::from(count))?;
    let mut seen = 0u32;
    let mut previous_offset = None;
    for _ in 0..count {
        let record = cursor.position_byte();
        let kind = cursor.read_u16_with(control)?;
        let unused = cursor.read_u16_with(control)?;
        let size = cursor.read_u32_with(control)?;
        let item_offset = file_offset(cursor.read_u32_with(control)?, ByteOffset(record.0 + 8))?;
        let (bit, alignment) = map_kind(kind).ok_or(InputError::Unsupported {
            reason: UnsupportedInput::MapType(kind),
            offset_byte: record,
        })?;
        if seen & bit != 0
            || unused != 0
            || size == 0
            || previous_offset.is_some_and(|previous| item_offset <= previous)
        {
            return Err(malformed(InputProblem::InvalidMap, record));
        }
        if !item_offset.0.is_multiple_of(alignment) {
            return Err(malformed(InputProblem::InvalidAlignment, record));
        }
        seen |= bit;
        previous_offset = Some(item_offset);
        match kind {
            0 => {
                if size != 1 || item_offset != ByteOffset(0) {
                    return Err(malformed(InputProblem::InvalidMap, record));
                }
            }
            1..=6 => {
                let expected = tables[usize::from(kind - 1)];
                if size != expected.count || item_offset != expected.offset_byte {
                    return Err(malformed(InputProblem::InvalidMap, record));
                }
            }
            0x1000 => {
                if size != 1 || item_offset != offset {
                    return Err(malformed(InputProblem::InvalidMap, record));
                }
            }
            _ => {
                if item_offset < data.start
                    || item_offset >= data.end
                    || (item_offset >= map_range.start && item_offset < map_range.end)
                {
                    return Err(malformed(InputProblem::InvalidMap, record));
                }
                // Minimum admission only; the item parser establishes actual extents.
                checked_range(
                    ByteLength(data.end.0),
                    item_offset,
                    count_usize(size, ByteOffset(record.0 + 4))?,
                    ByteLength(1),
                )?;
            }
        }
    }
    let mut required = (1 << 0) | (1 << 7);
    for (index, table) in tables.iter().enumerate() {
        if table.count != 0 {
            required |= 1 << (index + 1);
        }
    }
    if seen & required != required {
        return Err(malformed(InputProblem::InvalidMap, offset));
    }
    Ok(map_range)
}

pub(super) fn map_kind(kind: u16) -> Option<(u32, usize)> {
    let (bit, alignment) = match kind {
        0..=6 => (u32::from(kind), 4),
        0x1000..=0x1003 => (7 + u32::from(kind - 0x1000), 4),
        0x2000..=0x2006 => (
            11 + u32::from(kind - 0x2000),
            if matches!(kind, 0x2001 | 0x2006) {
                4
            } else {
                1
            },
        ),
        _ => return None,
    };
    Some((1 << bit, alignment))
}
