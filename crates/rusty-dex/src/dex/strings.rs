#![allow(dead_code)]

//! String identifiers.
//!
//! Upstream decoded every string to UTF-16 while parsing, which costs ~15 ms on a
//! 10 MiB DEX with 76k strings even though a single-class decompile reads a few
//! hundred of them. rasc's fork keeps each string as a byte range into the image
//! and decodes on first use, caching per string; see PATCHES.md.
#![allow(missing_docs, reason = "internal")]

use std::io::BufRead;
use std::io::{Seek, SeekFrom};
use std::sync::OnceLock;

use crate::dex::reader::{DexBytes, DexReader};
use crate::error::DexError;

/// Index into the DEX `string_ids` table.
pub type StringIdx = u32;

/// One DEX string.
///
/// Either bounds into the DEX image (the parse path) or an already decoded
/// UTF-16 buffer (the `From<String>` path used by tests and helpers). The
/// MUTF-8 bytes are decoded the first time `as_str`/`utf16` is called and the
/// result is cached, so a string no caller looks at is never decoded.
#[derive(Debug, Clone)]
pub struct DexString {
    /// `Some` when this string still lives in the DEX image.
    source: Option<StringSource>,
    utf16: OnceLock<Vec<u16>>,
    lossy: OnceLock<String>,
}

#[derive(Debug, Clone)]
struct StringSource {
    bytes: DexBytes,
    /// First MUTF-8 byte (after the uleb128 UTF-16 length).
    start: u32,
    /// One past the last MUTF-8 byte (the NUL terminator is not included).
    end: u32,
}

impl DexString {
    pub fn from_utf16(utf16: Vec<u16>) -> Self {
        let utf16_cell = OnceLock::new();
        let _ = utf16_cell.set(utf16);
        Self {
            source: None,
            utf16: utf16_cell,
            lossy: OnceLock::new(),
        }
    }

    /// A string that is decoded on demand from the DEX image.
    pub(crate) fn from_source(bytes: DexBytes, start: u32, end: u32) -> Self {
        Self {
            source: Some(StringSource { bytes, start, end }),
            utf16: OnceLock::new(),
            lossy: OnceLock::new(),
        }
    }

    pub fn as_str(&self) -> &str {
        self.lossy
            .get_or_init(|| String::from_utf16_lossy(self.utf16()))
    }

    pub fn utf16(&self) -> &[u16] {
        self.utf16.get_or_init(|| {
            let Some(source) = &self.source else {
                return Vec::new();
            };
            let raw = &source.bytes.as_ref()[source.start as usize..source.end as usize];
            // The bytes are validated by `DexStrings::build` (well-formedness and
            // the UTF-16 length check), so a failure here means the image changed
            // under us; degrade to a lossy view rather than panicking.
            crate::mutf8::decode(raw).unwrap_or_else(|_| raw.iter().map(|byte| u16::from(*byte)).collect())
        })
    }
}

impl PartialEq for DexString {
    fn eq(&self, other: &Self) -> bool {
        self.utf16() == other.utf16()
    }
}

impl Eq for DexString {}

impl PartialOrd for DexString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DexString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.utf16().cmp(other.utf16())
    }
}

impl std::hash::Hash for DexString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.utf16().hash(state);
    }
}

impl From<String> for DexString {
    fn from(value: String) -> Self {
        Self::from_utf16(value.encode_utf16().collect())
    }
}

impl From<&str> for DexString {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl std::fmt::Display for DexString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// List of strings of a DEX file
#[derive(Debug)]
pub struct DexStrings {
    pub strings: Vec<DexString>,
}

impl DexStrings {
    /// Parse all strings from a DEX file.
    ///
    /// The bytes of every string are located and validated here (well-formedness
    /// and the declared UTF-16 length), but decoding is left to first use.
    pub fn build(dex_reader: &mut DexReader, offset: u32, size: u32) -> Result<Self, DexError> {
        // Move to start of map list
        dex_reader.bytes.seek(SeekFrom::Start(offset.into()))?;

        let source = dex_reader.bytes.get_ref().clone();
        let mut strings = Vec::with_capacity(size as usize);
        let mut raw_string = Vec::new();

        for _ in 0..size {
            let string_offset = dex_reader.read_u32()?;
            let current_offset = dex_reader.bytes.position();

            dex_reader
                .bytes
                .seek(SeekFrom::Start(string_offset.into()))?;

            let (utf16_size, header_len) = dex_reader.read_uleb128()?;
            raw_string.clear();
            dex_reader.bytes.read_until(0, &mut raw_string)?;
            if raw_string.pop() != Some(0) {
                return Err(DexError::InvalidMutf8("[MUTF-8] missing null terminator"));
            }
            let actual = crate::mutf8::decode_len(&raw_string).map_err(DexError::InvalidMutf8)?;
            if actual != utf16_size as usize {
                return Err(DexError::InvalidStringLength {
                    expected: utf16_size,
                    actual,
                });
            }
            let start = u32::try_from(string_offset)
                .ok()
                .and_then(|start| start.checked_add(u32::try_from(header_len).ok()?))
                .ok_or(DexError::InvalidStringIdx)?;
            let end = start
                .checked_add(u32::try_from(raw_string.len()).map_err(|_| DexError::InvalidStringIdx)?)
                .ok_or(DexError::InvalidStringIdx)?;
            strings.push(DexString::from_source(source.clone(), start, end));

            dex_reader.bytes.seek(SeekFrom::Start(current_offset))?;
        }

        Ok(DexStrings { strings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_with_empty_strings() {
        let data = vec![
            0x64, 0x65, 0x78, 0x0a, 0x30, 0x33, 0x35, 0x00, 0x00, 0x00, // DEX magic
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // endianness tag
        ];
        let mut dex_reader = DexReader::build(data).unwrap();
        let dex_strings = DexStrings::build(&mut dex_reader, 44, 0).unwrap();

        assert_eq!(dex_strings.strings.len(), 0);
    }

    #[test]
    fn test_build_with_non_empty_strings() {
        let data = vec![
            0x64, 0x65, 0x78, 0x0a, 0x30, 0x33, 0x35, 0x00, 0x00, 0x00, // DEX magic
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // endianness tag
            // offsets
            0x3E, 0x00, 0x00, 0x00, 0x46, 0x00, 0x00, 0x00, 0x68, 0x00, 0x00, 0x00,
            // strings size and data
            0x06, b'H', b'e', b'l', b'l', b'o', b'!', 0x00, // string #0 value
            0x20, b'T', b'h', b'i', b's', b' ', b'i', b's', b' ', b'a', b' ', b't', b'e', b's',
            b't', b'.', b' ', b'\"', b'A', b'B', b'C', b'D', b'\"', b' ', b'i', b'n', b' ', b'M',
            b'U', b'T', b'F', b'-', b'8', 0x00, // string #1 value
            0x00, 0x00,
        ];

        let mut dex_reader = DexReader::build(data).unwrap();
        let dex_strings = DexStrings::build(&mut dex_reader, 50, 3).unwrap();

        assert_eq!(dex_strings.strings.len(), 3);
        assert_eq!(dex_strings.strings[0].as_str(), "Hello!");
        assert_eq!(
            dex_strings.strings[1].as_str(),
            "This is a test. \"ABCD\" in MUTF-8"
        );
        assert_eq!(dex_strings.strings[2].as_str(), "");
    }

    #[test]
    fn test_build_with_invalid_string() {
        let data = vec![
            0x64, 0x65, 0x78, 0x0a, 0x30, 0x33, 0x35, 0x00, 0x00, 0x00, // DEX magic
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // nothing
            0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // endianness tag
            // offsets
            0x36, 0x00, 0x00, 0x00, // string size and data
            0x02, 0xc3, 0x00, // incomplete MUTF-8 two-byte sequence
        ];

        let mut dex_reader = DexReader::build(data).unwrap();
        assert!(matches!(
            DexStrings::build(&mut dex_reader, 50, 1),
            Err(DexError::InvalidMutf8(_))
        ));
    }
}
