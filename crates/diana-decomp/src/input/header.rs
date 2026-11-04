use super::formats::DexVersion;
use super::{
    Cursor, DexContainer, InputError, InputProblem, UnsupportedInput, checked_range, count_usize,
    file_offset, malformed,
};

use crate::{ByteLength, ByteOffset, ByteRange};

pub(super) const SUPPORTED_MAP_KINDS: usize = 20;
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

/// Checked logical fixed sections and map envelope, with original integrity fields.
/// This does not certify variable item extents, references or executable code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DexHeader {
    pub version: DexVersion,
    pub header_offset: ByteOffset,
    pub checksum: u32,
    pub signature: [u8; 20],
    pub string_ids: TableSection,
    pub type_ids: TableSection,
    pub proto_ids: TableSection,
    pub field_ids: TableSection,
    pub method_ids: TableSection,
    pub class_defs: TableSection,
    pub link_byte_range: ByteRange,
    /// The declared data range for 035–040; for 041, the permissible forward
    /// physical range. Individual items still exclude every logical header.
    pub data_byte_range: ByteRange,
    pub map_byte_range: ByteRange,
}

/// Reads the first logical envelope after checking the physical header chain.
/// Original bytes and integrity fields remain unchanged; no authentication is
/// claimed. Returns typed input and resource errors.
pub fn read_header(bytes: &[u8]) -> Result<DexHeader, InputError> {
    let container = DexContainer::open(bytes)?;
    read_member_header(container, ByteOffset(0))
}

pub(super) fn read_member_header(
    container: DexContainer<'_>,
    header_offset: ByteOffset,
) -> Result<DexHeader, InputError> {
    let bytes = container.bytes;
    let header_end = header_offset
        .checked_advance(container.header_size())
        .ok_or_else(|| malformed(InputProblem::RangeOverflow, header_offset))?;
    let mut cursor = Cursor::bounded(bytes, header_offset, header_end)?;
    cursor.read_array::<8>()?;
    let checksum = cursor.read_u32()?;
    let signature = cursor.read_array()?;
    cursor.skip(3, ByteLength(4))?;
    let link_size = count_usize(cursor.read_u32()?, ByteOffset(header_offset.0 + 0x2c))?;
    let link_offset = file_offset(cursor.read_u32()?, ByteOffset(header_offset.0 + 0x30))?;
    let link_byte_range = section(
        ByteLength(bytes.len()),
        link_offset,
        link_size,
        ByteLength(1),
        1,
        ByteOffset(header_offset.0 + 0x2c),
        header_end,
    )?;
    container.check_data_range(link_byte_range)?;
    let map_offset = file_offset(cursor.read_u32()?, ByteOffset(header_offset.0 + 0x34))?;
    let mut tables = [TableSection {
        count: 0,
        offset_byte: ByteOffset(0),
        size_byte: ByteLength(0),
    }; 6];
    for (index, table) in tables.iter_mut().enumerate() {
        let field = ByteOffset(header_offset.0 + 0x38 + index * 8);
        let count = cursor.read_u32()?;
        let offset = file_offset(cursor.read_u32()?, ByteOffset(field.0 + 4))?;
        let range = section(
            ByteLength(bytes.len()),
            offset,
            count_usize(count, field)?,
            TABLE_STRIDES[index],
            4,
            field,
            header_end,
        )?;
        container.check_data_range(range)?;
        if matches!(index, 1 | 2) && count > 65535 {
            return Err(malformed(InputProblem::InvalidTableCount, field));
        }
        *table = TableSection {
            count,
            offset_byte: offset,
            size_byte: ByteLength(range.end.0 - range.start.0),
        };
    }
    let data_size = cursor.read_u32()?;
    let data_raw = cursor.read_u32()?;
    let native_container = container.version == DexVersion::V041;
    let data_byte_range = if native_container {
        ByteRange {
            start: header_end,
            end: ByteOffset(bytes.len()),
        }
    } else {
        section(
            ByteLength(bytes.len()),
            file_offset(data_raw, ByteOffset(header_offset.0 + 0x6c))?,
            count_usize(data_size, ByteOffset(header_offset.0 + 0x68))?,
            ByteLength(1),
            4,
            ByteOffset(header_offset.0 + 0x68),
            header_end,
        )?
    };
    check_fixed_overlap(
        &tables,
        link_byte_range,
        (!native_container).then_some(data_byte_range),
    )?;
    if !map_offset.0.is_multiple_of(4) {
        return Err(malformed(
            InputProblem::InvalidAlignment,
            ByteOffset(header_offset.0 + 0x34),
        ));
    }
    if map_offset < data_byte_range.start || map_offset >= data_byte_range.end {
        return Err(malformed(
            InputProblem::InvalidMap,
            ByteOffset(header_offset.0 + 0x34),
        ));
    }
    let map_byte_range = check_map(
        container,
        header_offset,
        map_offset,
        data_byte_range,
        &tables,
    )?;
    check_fixed_overlap(&tables, link_byte_range, Some(map_byte_range))?;
    let [
        string_ids,
        type_ids,
        proto_ids,
        field_ids,
        method_ids,
        class_defs,
    ] = tables;
    Ok(DexHeader {
        version: container.version,
        header_offset,
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
    minimum: ByteOffset,
) -> Result<ByteRange, InputError> {
    if (count == 0) != (offset == ByteOffset(0)) {
        return Err(malformed(InputProblem::InvalidSectionPair, field));
    }
    if !offset.0.is_multiple_of(alignment) {
        return Err(malformed(InputProblem::InvalidAlignment, field));
    }
    let range = checked_range(input_length, offset, count, stride)?;
    if count != 0 && offset < minimum {
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
    data: Option<ByteRange>,
) -> Result<(), InputError> {
    if data.is_some_and(|data| overlaps(link, data)) {
        return Err(malformed(InputProblem::OverlappingSections, link.start));
    }
    for (index, table) in tables.iter().enumerate() {
        let range = table_range(*table)?;
        if overlaps(range, link) || data.is_some_and(|data| overlaps(range, data)) {
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
    container: DexContainer<'_>,
    header_offset: ByteOffset,
    offset: ByteOffset,
    data: ByteRange,
    tables: &[TableSection; 6],
) -> Result<ByteRange, InputError> {
    let prefix = checked_range(ByteLength(data.end.0), offset, 1, ByteLength(4))?;
    let mut cursor = Cursor::at(container.bytes, offset)?;
    let count = cursor.read_u32()?;
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
    container.check_data_range(map_range)?;

    let mut seen = 0u32;
    let mut previous_offset = None;
    let mut dynamic_end = None;
    for _ in 0..count {
        let record = cursor.position_byte();
        let kind = cursor.read_u16()?;
        let unused = cursor.read_u16()?;
        let size = cursor.read_u32()?;
        let item_offset = file_offset(cursor.read_u32()?, ByteOffset(record.0 + 8))?;
        if matches!(kind, 7 | 8) && container.version < DexVersion::V038 {
            return Err(InputError::Unsupported {
                reason: UnsupportedInput::MapType(kind),
                offset_byte: record,
            });
        }
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
        if dynamic_end.is_some_and(|end| item_offset < end) {
            return Err(malformed(InputProblem::OverlappingSections, item_offset));
        }
        seen |= bit;
        previous_offset = Some(item_offset);
        if matches!(kind, 7 | 8) {
            let range = checked_range(
                ByteLength(data.end.0),
                item_offset,
                count_usize(size, record)?,
                ByteLength(if kind == 7 { 4 } else { 8 }),
            )?;
            container.check_data_range(range)?;
            dynamic_end = Some(range.end);
            if overlaps(range, map_range) {
                return Err(malformed(InputProblem::OverlappingSections, item_offset));
            }
            for table in tables {
                if overlaps(range, table_range(*table)?) {
                    return Err(malformed(InputProblem::OverlappingSections, item_offset));
                }
            }
        }
        match kind {
            0 => {
                if size != 1 || item_offset != header_offset {
                    return Err(malformed(InputProblem::InvalidMap, record));
                }
            }
            1..=6 => {
                let expected = tables[usize::from(kind - 1)];
                if size != expected.count || item_offset != expected.offset_byte {
                    return Err(malformed(InputProblem::InvalidMap, record));
                }
            }
            7 | 8 => {}
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
                container.item_end(item_offset)?;
                for table in tables {
                    let range = table_range(*table)?;
                    if item_offset >= range.start && item_offset < range.end {
                        return Err(malformed(InputProblem::OverlappingSections, item_offset));
                    }
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
        7..=8 => (18 + u32::from(kind - 7), 4),
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
