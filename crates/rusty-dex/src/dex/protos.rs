//! Representation of method prototypes
//!
//! This module contains the logic to decode method prototypes from a DEX file.

use std::io::{Seek, SeekFrom};

use crate::dex::reader::DexReader;
use crate::dex::strings::DexStrings;
use crate::dex::types::DexTypes;
use crate::error::DexError;

/// One row of the DEX `proto_ids` table.
///
/// The parameters list is copied as type indices at parse time (cheap u16
/// reads, no names); the prototype text is rendered on demand.
#[derive(Debug)]
pub struct ProtoRow {
    pub return_type_idx: u32,
    pub parameters: Vec<u32>,
}

/// List of decoded prototypes in the DEX files
#[derive(Debug)]
pub struct DexProtos {
    pub items: Vec<ProtoRow>,
}

impl DexProtos {
    /// Parse the prototypes from the reader
    pub fn build(
        dex_reader: &mut DexReader,
        offset: u32,
        size: u32,
        types_list: &DexTypes,
    ) -> Result<Self, DexError> {
        dex_reader.bytes.seek(SeekFrom::Start(offset.into()))?;

        let mut items = Vec::with_capacity(size as usize);

        for _ in 0..size {
            let _shorty_idx = dex_reader.read_u32()?;
            let return_type_idx = dex_reader.read_u32()?;
            let parameters_off = dex_reader.read_u32()?;

            let mut parameters = Vec::new();
            if parameters_off != 0 {
                // Save current stream position
                let current_pos = dex_reader.bytes.position();

                // Decode the parameters
                dex_reader
                    .bytes
                    .seek(SeekFrom::Start(parameters_off.into()))?;

                let params_size = dex_reader.read_u32()?;
                parameters.reserve(params_size as usize);
                for _ in 0..params_size {
                    let type_index = dex_reader.read_u16()?;
                    if types_list.items.get(type_index as usize).is_none() {
                        return Err(DexError::InvalidTypeIdx);
                    }
                    parameters.push(type_index as u32);
                }

                // Go back to the previous position
                dex_reader.bytes.seek(SeekFrom::Start(current_pos))?;
            }
            if types_list.items.get(return_type_idx as usize).is_none() {
                return Err(DexError::InvalidTypeIdx);
            }
            items.push(ProtoRow {
                return_type_idx,
                parameters,
            });
        }

        Ok(DexProtos { items })
    }

    /// Render prototype `idx` as `(` + parameter descriptors + `)` + return
    /// descriptor, the exact text the eager decoder used to store.
    pub fn render(&self, types: &DexTypes, strings: &DexStrings, idx: u32) -> Option<String> {
        let row = self.items.get(idx as usize)?;
        let mut text = String::from("(");
        for parameter in &row.parameters {
            text.push_str(types.descriptor(strings, *parameter)?);
        }
        text.push(')');
        text.push_str(types.descriptor(strings, row.return_type_idx)?);
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_concatenates_parameters_then_return() {
        let strings = DexStrings {
            strings: vec![
                "Ljava/lang/Class;".into(),
                "Ljava/lang/reflect/Field;".into(),
                "V".into(),
            ],
        };
        let types = DexTypes { items: vec![0, 1, 2] };
        let protos = DexProtos {
            items: vec![ProtoRow {
                return_type_idx: 2,
                parameters: vec![0, 1],
            }],
        };

        assert_eq!(
            protos.render(&types, &strings, 0),
            Some("(Ljava/lang/Class;Ljava/lang/reflect/Field;)V".to_string())
        );
        assert_eq!(protos.render(&types, &strings, 1), None);
    }
}
