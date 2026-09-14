//! Methods identifiers
//!
//! This module deals with method identifiers. Each DEX file contains a list
//! of identifiers for all methods reffered to in the code. The list is sorted
//! by the defining type (by `type_id` index), method name (by `string_id`
//! index), and method prototype (by `proto_id` index), and cannot contain
//! duplicates.

use std::io::{Seek, SeekFrom};

use crate::dex::protos::DexProtos;
use crate::dex::reader::DexReader;
use crate::dex::strings::DexStrings;
use crate::dex::types::DexTypes;
use crate::error::DexError;

/// One row of the DEX `method_ids` table.
#[derive(Debug)]
pub struct MethodRow {
    pub class_idx: u16,
    pub proto_idx: u16,
    pub name_idx: u32,
}

/// Sorted list of method IDs
#[derive(Debug)]
pub struct DexMethods {
    pub items: Vec<MethodRow>,
}

impl DexMethods {
    /// Build the list of method identifiers from a file
    ///
    /// Every row is validated against the type, prototype and string lists,
    /// but the `Lcls;->name(proto)` text is rendered on demand.
    pub fn build(
        dex_reader: &mut DexReader,
        offset: u32,
        size: u32,
        types_list: &DexTypes,
        protos_list: &DexProtos,
        strings_list: &DexStrings,
    ) -> Result<Self, DexError> {
        dex_reader.bytes.seek(SeekFrom::Start(offset.into()))?;

        let mut items = Vec::with_capacity(size as usize);

        for _ in 0..size {
            let class_idx = dex_reader.read_u16()?;
            let proto_idx = dex_reader.read_u16()?;
            let name_idx = dex_reader.read_u32()?;

            if types_list.items.get(class_idx as usize).is_none() {
                return Err(DexError::InvalidTypeIdx);
            }
            if protos_list.items.get(proto_idx as usize).is_none() {
                return Err(DexError::InvalidTypeIdx);
            }
            if strings_list.strings.get(name_idx as usize).is_none() {
                return Err(DexError::InvalidStringIdx);
            }

            items.push(MethodRow {
                class_idx,
                proto_idx,
                name_idx,
            });
        }

        Ok(DexMethods { items })
    }

    /// Render method `idx` as `Lcls;->name(proto)`, the exact text the eager
    /// decoder used to store.
    pub fn render(
        &self,
        types: &DexTypes,
        protos: &DexProtos,
        strings: &DexStrings,
        idx: u32,
    ) -> Option<String> {
        let row = self.items.get(idx as usize)?;
        let mut text = String::new();
        text.push_str(types.descriptor(strings, row.class_idx as u32)?);
        text.push_str("->");
        text.push_str(strings.strings.get(row.name_idx as usize)?.as_str());
        text.push_str(&protos.render(types, strings, row.proto_idx as u32)?);
        Some(text)
    }
}
