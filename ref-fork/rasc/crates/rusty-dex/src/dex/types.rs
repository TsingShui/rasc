//! DEX types
//!
//! Types that are used in the bytecode are stored as strings. The DEX header then contains a list
//! of offset of these strings. This module decodes these offset to make them available later on.

use std::io::{Seek, SeekFrom};

use crate::dex::reader::DexReader;
use crate::dex::strings::DexStrings;
use crate::error::DexError;

/// Index into the DEX `type_ids` table.
pub type TypeIdx = u32;

/// List of types defined in the DEX file.
///
/// A `type_ids` row is just a `string_idx`; the descriptor text is rendered on
/// demand through [`DexTypes::descriptor`] so an unread type costs one u32 at
/// parse time.
#[derive(Debug)]
pub struct DexTypes {
    pub items: Vec<u32>,
}

impl DexTypes {
    /// Parse the types from a DEX reader
    ///
    /// Every row is validated against `strings_list` (the index must name a
    /// real string), but the strings themselves are not materialized here.
    pub fn build(
        dex_reader: &mut DexReader,
        offset: u32,
        size: u32,
        strings_list: &DexStrings,
    ) -> Result<Self, DexError> {
        dex_reader.bytes.seek(SeekFrom::Start(offset.into()))?;

        let mut items = Vec::with_capacity(size as usize);

        for _ in 0..size {
            let string_index = dex_reader.read_u32()?;
            if strings_list.strings.get(string_index as usize).is_none() {
                return Err(DexError::InvalidStringIdx);
            }
            items.push(string_index);
        }

        Ok(DexTypes { items })
    }

    /// Render the descriptor named by type index `idx`, borrowing the lazy
    /// string cache in `strings`.
    pub fn descriptor<'a>(&self, strings: &'a DexStrings, idx: u32) -> Option<&'a str> {
        let string_index = *self.items.get(idx as usize)?;
        Some(strings.strings.get(string_index as usize)?.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dex_types_empty() {
        let dex_data = vec![
            0x64, 0x65, 0x78, 0x0a, 0x30, 0x33, 0x35, 0x00, 0x00, 0x00, // DEX magic
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // endianness tag
        ];

        let mut dex_reader = DexReader::build(dex_data).unwrap();
        let strings_list = DexStrings {
            strings: Vec::new(),
        };

        let dex_types = DexTypes::build(&mut dex_reader, 0, 0, &strings_list).unwrap();

        assert_eq!(dex_types.items.len(), 0);
    }

    #[test]
    fn test_build_dex_types() {
        let dex_data = vec![
            0x64, 0x65, 0x78, 0x0a, 0x30, 0x33, 0x35, 0x00, 0x00, 0x00, // DEX magic
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // endianness tag
            0x00, 0x00, 0x00, 0x00, // type 0 offset
            0x01, 0x00, 0x00, 0x00, // type 1 offset
            0x02, 0x00, 0x00, 0x00, // type 2 offset
            0x03, 0x00, 0x00, 0x00, // type 3 offset
        ];

        let mut dex_reader = DexReader::build(dex_data).unwrap();
        let strings_list = DexStrings {
            strings: vec![
                "Type0".into(),
                "Type1".into(),
                "Type2".into(),
                "Type3".into(),
            ],
        };

        let dex_types = DexTypes::build(&mut dex_reader, 50, 4, &strings_list).unwrap();

        assert_eq!(dex_types.items.len(), 4);
        assert_eq!(dex_types.items, vec![0, 1, 2, 3]);
        assert_eq!(dex_types.descriptor(&strings_list, 0), Some("Type0"));
        assert_eq!(dex_types.descriptor(&strings_list, 1), Some("Type1"));
        assert_eq!(dex_types.descriptor(&strings_list, 2), Some("Type2"));
        assert_eq!(dex_types.descriptor(&strings_list, 3), Some("Type3"));
        assert_eq!(dex_types.descriptor(&strings_list, 4), None);
    }

    #[test]
    fn test_build_dex_types_duplicates() {
        let dex_data = vec![
            0x64, 0x65, 0x78, 0x0a, 0x30, 0x33, 0x35, 0x00, 0x00, 0x00, // DEX magic
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // endianness tag
            0x00, 0x00, 0x00, 0x00, // type 0 offset
            0x01, 0x00, 0x00, 0x00, // type 1 offset
            0x02, 0x00, 0x00, 0x00, // type 1 duplicate offset
        ];

        let mut dex_reader = DexReader::build(dex_data).unwrap();
        let strings_list = DexStrings {
            strings: vec!["Type0".into(), "Type1".into(), "Type1".into()],
        };

        let dex_types = DexTypes::build(&mut dex_reader, 50, 2, &strings_list).unwrap();

        assert_eq!(dex_types.items.len(), 2);
        assert_eq!(dex_types.items, vec![0, 1]);
        assert_eq!(dex_types.descriptor(&strings_list, 0), Some("Type0"));
        assert_eq!(dex_types.descriptor(&strings_list, 1), Some("Type1"));
    }
}
