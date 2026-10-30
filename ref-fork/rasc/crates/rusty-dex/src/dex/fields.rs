//! Representation of class fields
//!
//! This module decodes fields from a DEX file and returns them in the correct order. Fields must
//! be ordered by the class they belong to, then their name, and finally their type.
//! Each field can represent a static field initialized in the `<cinit>` pseudo-method or a class
//! field that is initialized when the class is instantiated.

use std::io::{Seek, SeekFrom};

use crate::dex::reader::DexReader;
use crate::dex::strings::DexStrings;
use crate::dex::types::DexTypes;
use crate::error::DexError;

/// One row of the DEX `field_ids` table.
#[derive(Debug)]
pub struct FieldRow {
    pub class_idx: u16,
    pub type_idx: u16,
    pub name_idx: u32,
}

/// Representation of the fields in a DEX file. Only the decoded fields are present in the correct
/// order.
#[derive(Debug)]
pub struct DexFields {
    /// Vector of field rows in the correct order
    pub items: Vec<FieldRow>,
}

impl DexFields {
    /// Parse the fields from the DEX file
    ///
    /// Every row is validated against the type and string lists, but the
    /// `Lcls;->name:type` text is rendered on demand.
    pub fn build(
        dex_reader: &mut DexReader,
        offset: u32,
        size: u32,
        types_list: &DexTypes,
        strings_list: &DexStrings,
    ) -> Result<Self, DexError> {
        dex_reader.bytes.seek(SeekFrom::Start(offset.into()))?;

        let mut items = Vec::with_capacity(size as usize);

        for _ in 0..size {
            let class_idx = dex_reader.read_u16()?;
            let type_idx = dex_reader.read_u16()?;
            let name_idx = dex_reader.read_u32()?;

            if types_list.items.get(class_idx as usize).is_none()
                || types_list.items.get(type_idx as usize).is_none()
            {
                return Err(DexError::InvalidTypeIdx);
            }
            if strings_list.strings.get(name_idx as usize).is_none() {
                return Err(DexError::InvalidStringIdx);
            }

            items.push(FieldRow {
                class_idx,
                type_idx,
                name_idx,
            });
        }

        Ok(DexFields { items })
    }

    /// Render field `idx` as `Lcls;->name:type`, the exact text the eager
    /// decoder used to store.
    pub fn render(&self, types: &DexTypes, strings: &DexStrings, idx: u32) -> Option<String> {
        let row = self.items.get(idx as usize)?;
        let mut text = String::new();
        text.push_str(types.descriptor(strings, row.class_idx as u32)?);
        text.push_str("->");
        text.push_str(strings.strings.get(row.name_idx as usize)?.as_str());
        text.push(':');
        text.push_str(types.descriptor(strings, row.type_idx as u32)?);
        Some(text)
    }
}
