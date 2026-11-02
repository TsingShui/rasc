//! Bounded Android binary XML (AXML) reader for `AndroidManifest.xml`.
//!
//! The container is a chunked format: a file header, a string pool, an optional
//! resource map and a flat sequence of namespace/element records. Every length,
//! offset and index is checked against the remaining bytes before use, nesting is
//! bounded by the actual input, and an unsupported record or typed value is a
//! classified error instead of a guessed document.

use thiserror::Error;

/// Explicit manifest bounds. Unset fields impose no numeric cap; structural
/// checks against the actual bytes always apply.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ManifestLimits {
    pub max_nodes: Option<u64>,
    pub max_string_units: Option<u64>,
    pub max_output_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestLimit {
    Nodes,
    StringUnits,
    OutputBytes,
}

impl std::fmt::Display for ManifestLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Nodes => "nodes",
            Self::StringUnits => "string_units",
            Self::OutputBytes => "output_bytes",
        })
    }
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("input is not an Android binary XML document")]
    NotAxml,
    #[error("binary XML chunk is truncated or inconsistent at byte {offset}")]
    InconsistentChunk { offset: usize },
    #[error("binary XML record {kind:#06x} is not supported")]
    UnsupportedChunk { kind: u16 },
    #[error("string pool entry {index} is outside the pool or malformed")]
    InvalidString { index: u32 },
    #[error("element sequence is unbalanced at byte {offset}")]
    Unbalanced { offset: usize },
    #[error("element at byte {offset} repeats attribute {name}")]
    DuplicateAttribute { offset: usize, name: String },
    #[error("typed value {data_type:#04x} of attribute {index} is not supported")]
    UnsupportedValue { index: u32, data_type: u8 },
    #[error("{limit} exceeded: requested {requested}, maximum {maximum}")]
    Budget {
        limit: ManifestLimit,
        requested: u64,
        maximum: u64,
    },
    #[error("manifest value is not representable on this platform")]
    PlatformCapacity,
}

impl ManifestError {
    pub fn kind(&self) -> diana_decomp::FailureKind {
        use diana_decomp::FailureKind as Kind;
        match self {
            Self::Budget { .. } | Self::PlatformCapacity => Kind::BudgetExceeded,
            Self::NotAxml | Self::UnsupportedChunk { .. } | Self::UnsupportedValue { .. } => {
                Kind::Unsupported
            }
            Self::InconsistentChunk { .. }
            | Self::InvalidString { .. }
            | Self::Unbalanced { .. }
            | Self::DuplicateAttribute { .. } => Kind::Malformed,
        }
    }
}

const RES_XML_TYPE: u16 = 0x0003;
const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_XML_RESOURCE_MAP_TYPE: u16 = 0x0180;
const RES_XML_START_NAMESPACE_TYPE: u16 = 0x0100;
const RES_XML_END_NAMESPACE_TYPE: u16 = 0x0101;
const RES_XML_START_ELEMENT_TYPE: u16 = 0x0102;
const RES_XML_END_ELEMENT_TYPE: u16 = 0x0103;

const UTF8_FLAG: u32 = 1 << 8;

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ManifestError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(ManifestError::InconsistentChunk { offset })?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, ManifestError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(ManifestError::InconsistentChunk { offset })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn bounded(offset: usize, size: usize, end: usize) -> Result<usize, ManifestError> {
    let limit = offset
        .checked_add(size)
        .ok_or(ManifestError::InconsistentChunk { offset })?;
    if limit > end {
        return Err(ManifestError::InconsistentChunk { offset });
    }
    Ok(limit)
}

struct StringPool<'a> {
    data: &'a [u8],
    offsets: &'a [u8],
    count: u32,
    utf8: bool,
}

impl<'a> StringPool<'a> {
    fn parse(
        bytes: &'a [u8],
        offset: usize,
        header: usize,
        end: usize,
    ) -> Result<Self, ManifestError> {
        let count = u32_at(bytes, offset + 8)?;
        let flags = u32_at(bytes, offset + 16)?;
        let style_count = u32_at(bytes, offset + 12)?;
        let strings_start = usize::try_from(u32_at(bytes, offset + 20)?)
            .map_err(|_| ManifestError::PlatformCapacity)?;
        let styles_start = u32_at(bytes, offset + 24)?;
        if style_count != 0 && styles_start == 0 {
            return Err(ManifestError::InconsistentChunk { offset });
        }
        if styles_start != 0 {
            bounded(
                offset,
                usize::try_from(styles_start).map_err(|_| ManifestError::PlatformCapacity)?,
                end,
            )?;
        }
        let header = header.max(28);
        let offsets_start = bounded(offset, header, end)?;
        let offsets_len = usize::try_from(count)
            .map_err(|_| ManifestError::PlatformCapacity)?
            .checked_mul(4)
            .ok_or(ManifestError::PlatformCapacity)?;
        let offsets_end = bounded(offsets_start, offsets_len, end)?;
        let data_start = bounded(offset, strings_start, end)?;
        if data_start < offsets_end {
            return Err(ManifestError::InconsistentChunk { offset });
        }
        Ok(Self {
            data: &bytes[data_start..end],
            offsets: &bytes[offsets_start..offsets_end],
            count,
            utf8: flags & UTF8_FLAG != 0,
        })
    }

    /// Returns the raw payload of one string: UTF-8 bytes or UTF-16LE units.
    fn payload(&self, index: u32, budget: &mut Budget<'_>) -> Result<&'a [u8], ManifestError> {
        if index >= self.count {
            return Err(ManifestError::InvalidString { index });
        }
        let at = usize::try_from(index)
            .map_err(|_| ManifestError::PlatformCapacity)?
            .checked_mul(4)
            .ok_or(ManifestError::PlatformCapacity)?;
        let relative = usize::try_from(u32_at(self.offsets, at)?)
            .map_err(|_| ManifestError::PlatformCapacity)?;
        let record = self
            .data
            .get(relative..)
            .ok_or(ManifestError::InvalidString { index })?;
        if self.utf8 {
            // The first prefix is the payload byte length; the second is the
            // UTF-16 unit length used for accounting.
            let (byte_length, after_byte_length) = utf8_prefix(record, 0, index)?;
            let (units, payload_start) = utf8_prefix(record, after_byte_length, index)?;
            budget.string_units(u64::from(units))?;
            let length =
                usize::try_from(byte_length).map_err(|_| ManifestError::PlatformCapacity)?;
            record
                .get(payload_start..payload_start + length)
                .ok_or(ManifestError::InvalidString { index })
        } else {
            let units = u32::from(u16_at(record, 0)?);
            budget.string_units(u64::from(units))?;
            let length = usize::try_from(units)
                .map_err(|_| ManifestError::PlatformCapacity)?
                .checked_mul(2)
                .ok_or(ManifestError::PlatformCapacity)?;
            record
                .get(2..2 + length)
                .ok_or(ManifestError::InvalidString { index })
        }
    }

    fn text(&self, index: u32, budget: &mut Budget<'_>) -> Result<String, ManifestError> {
        let payload = self.payload(index, budget)?;
        if self.utf8 {
            Ok(String::from_utf8_lossy(payload).into_owned())
        } else {
            let mut text = String::new();
            for chunk in payload.as_chunks::<2>().0 {
                text.push(
                    char::from_u32(u32::from(u16::from_le_bytes(*chunk))).unwrap_or('\u{fffd}'),
                );
            }
            Ok(text)
        }
    }
}

fn utf8_prefix(record: &[u8], at: usize, index: u32) -> Result<(u32, usize), ManifestError> {
    let first = *record
        .get(at)
        .ok_or(ManifestError::InvalidString { index })?;
    if first & 0x80 == 0 {
        Ok((u32::from(first), at + 1))
    } else {
        let second = *record
            .get(at + 1)
            .ok_or(ManifestError::InvalidString { index })?;
        Ok(((u32::from(first & 0x7f) << 8) | u32::from(second), at + 2))
    }
}

struct Budget<'a> {
    limits: ManifestLimits,
    nodes: u64,
    units: u64,
    output: String,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl Budget<'_> {
    fn nodes(&mut self) -> Result<(), ManifestError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(ManifestError::PlatformCapacity)?;
        if let Some(maximum) = self.limits.max_nodes
            && self.nodes > maximum
        {
            return Err(ManifestError::Budget {
                limit: ManifestLimit::Nodes,
                requested: self.nodes,
                maximum,
            });
        }
        Ok(())
    }

    fn string_units(&mut self, count: u64) -> Result<(), ManifestError> {
        self.units = self
            .units
            .checked_add(count)
            .ok_or(ManifestError::PlatformCapacity)?;
        if let Some(maximum) = self.limits.max_string_units
            && self.units > maximum
        {
            return Err(ManifestError::Budget {
                limit: ManifestLimit::StringUnits,
                requested: self.units,
                maximum,
            });
        }
        Ok(())
    }

    fn write(&mut self, text: &str) -> Result<(), ManifestError> {
        let needed = self
            .output
            .len()
            .checked_add(text.len())
            .ok_or(ManifestError::PlatformCapacity)?;
        if let Some(maximum) = self.limits.max_output_bytes
            && u64::try_from(needed).map_err(|_| ManifestError::PlatformCapacity)? > maximum
        {
            return Err(ManifestError::Budget {
                limit: ManifestLimit::OutputBytes,
                requested: u64::try_from(needed).map_err(|_| ManifestError::PlatformCapacity)?,
                maximum,
            });
        }
        self.output
            .try_reserve(text.len())
            .map_err(|_| ManifestError::PlatformCapacity)?;
        self.output.push_str(text);
        Ok(())
    }

    fn write_escaped(&mut self, text: &str) -> Result<(), ManifestError> {
        let mut escaped = String::new();
        for character in text.chars() {
            match character {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&apos;"),
                other => escaped.push(other),
            }
        }
        self.write(&escaped)
    }

    fn indent(&mut self, depth: usize) -> Result<(), ManifestError> {
        for _ in 0..depth {
            self.write("    ")?;
        }
        Ok(())
    }
}

/// Decodes one binary `AndroidManifest.xml` document into text XML.
///
/// The reader rejects a non-AXML container, a truncated or inconsistent chunk, an
/// unknown record type, an unsupported typed value and an unbalanced element
/// sequence. Elements and attributes are emitted with their original names;
/// attributes in a known Android namespace use that namespace's prefix.
pub fn decode(bytes: &[u8], limits: ManifestLimits) -> Result<String, ManifestError> {
    if u16_at(bytes, 0)? != RES_XML_TYPE {
        return Err(ManifestError::NotAxml);
    }
    let header = usize::from(u16_at(bytes, 2)?);
    let size = usize::try_from(u32_at(bytes, 4)?).map_err(|_| ManifestError::PlatformCapacity)?;
    let end = bounded(0, size, bytes.len())?;
    if header < 8 || header > size {
        return Err(ManifestError::InconsistentChunk { offset: 0 });
    }
    let mut budget = Budget {
        limits,
        nodes: 0,
        units: 0,
        output: String::new(),
        _marker: std::marker::PhantomData,
    };
    let mut offset = header;
    let mut pool: Option<StringPool<'_>> = None;
    let mut namespaces: Vec<(u32, String)> = Vec::new();
    let mut open: Vec<String> = Vec::new();
    let mut root_seen = false;
    while offset < end {
        let kind = u16_at(bytes, offset)?;
        let chunk_header = usize::from(u16_at(bytes, offset + 2)?);
        let chunk_size = usize::try_from(u32_at(bytes, offset + 4)?)
            .map_err(|_| ManifestError::PlatformCapacity)?;
        if chunk_header < 8 || chunk_size < chunk_header {
            return Err(ManifestError::InconsistentChunk { offset });
        }
        let chunk_end = bounded(offset, chunk_size, end)?;
        match kind {
            RES_STRING_POOL_TYPE => {
                if pool.is_some() {
                    return Err(ManifestError::InconsistentChunk { offset });
                }
                pool = Some(StringPool::parse(bytes, offset, chunk_header, chunk_end)?);
            }
            RES_XML_RESOURCE_MAP_TYPE => {
                // Validated for shape only: attribute names come from the string
                // pool, so a guessed resource-id table cannot mislabel them.
                if (chunk_end - offset - chunk_header) % 4 != 0 {
                    return Err(ManifestError::InconsistentChunk { offset });
                }
            }
            RES_XML_START_NAMESPACE_TYPE => {
                if chunk_header < 16 {
                    return Err(ManifestError::InconsistentChunk { offset });
                }
                let pool = pool
                    .as_ref()
                    .ok_or(ManifestError::InconsistentChunk { offset })?;
                let prefix = u32_at(bytes, offset + 16)?;
                let uri = u32_at(bytes, offset + 20)?;
                namespaces.push((prefix, pool.text(uri, &mut budget)?));
                budget.nodes()?;
            }
            RES_XML_END_NAMESPACE_TYPE => {
                namespaces.pop();
            }
            RES_XML_START_ELEMENT_TYPE => {
                if root_seen && open.is_empty() {
                    return Err(ManifestError::Unbalanced { offset });
                }
                let Some(active) = pool.as_ref() else {
                    return Err(ManifestError::InconsistentChunk { offset });
                };
                start_element(
                    bytes,
                    offset,
                    chunk_header,
                    chunk_end,
                    active,
                    &namespaces,
                    &mut budget,
                    &mut open,
                )?;
                root_seen = true;
            }
            RES_XML_END_ELEMENT_TYPE => {
                if chunk_header < 16 {
                    return Err(ManifestError::InconsistentChunk { offset });
                }
                let Some(active) = pool.as_ref() else {
                    return Err(ManifestError::InconsistentChunk { offset });
                };
                let name = active.text(u32_at(bytes, offset + 20)?, &mut budget)?;
                let Some(opened) = open.pop() else {
                    return Err(ManifestError::Unbalanced { offset });
                };
                if opened != name {
                    return Err(ManifestError::Unbalanced { offset });
                }
                budget.nodes()?;
                let depth = open.len();
                budget.indent(depth)?;
                budget.write("</")?;
                budget.write_escaped(&name)?;
                budget.write(">\n")?;
            }
            other => return Err(ManifestError::UnsupportedChunk { kind: other }),
        }
        offset = chunk_end;
    }
    if !open.is_empty() {
        return Err(ManifestError::Unbalanced { offset });
    }
    Ok(budget.output)
}

#[allow(clippy::too_many_arguments)]
fn start_element(
    bytes: &[u8],
    offset: usize,
    header: usize,
    end: usize,
    pool: &StringPool<'_>,
    namespaces: &[(u32, String)],
    budget: &mut Budget<'_>,
    open: &mut Vec<String>,
) -> Result<(), ManifestError> {
    if header < 16 {
        return Err(ManifestError::InconsistentChunk { offset });
    }
    let name_index = u32_at(bytes, offset + 20)?;
    let attribute_start = usize::from(u16_at(bytes, offset + 24)?);
    let attribute_size = usize::from(u16_at(bytes, offset + 26)?);
    let attribute_count = usize::from(u16_at(bytes, offset + 28)?);
    let name = pool.text(name_index, budget)?;
    if attribute_count != 0 && attribute_size < 20 {
        return Err(ManifestError::InconsistentChunk { offset });
    }
    budget.nodes()?;
    let depth = open.len();
    budget.indent(depth)?;
    budget.write("<")?;
    budget.write_escaped(&name)?;
    if open.is_empty() {
        // Every namespace used by an attribute must be declared in the output, or
        // the text is not parseable XML.
        for (prefix_index, uri) in namespaces {
            let prefix = pool.text(*prefix_index, budget)?;
            let uri = uri.clone();
            budget.write(" xmlns")?;
            if !prefix.is_empty() {
                budget.write(":")?;
                budget.write_escaped(&prefix)?;
            }
            budget.write("=\"")?;
            budget.write_escaped(&uri)?;
            budget.write("\"")?;
        }
    }
    // `attributeStart` is relative to the attribute extension that follows the
    // 16-byte node header, not to the chunk start.
    let base = bounded(offset + 16, attribute_start, end)?;
    let mut seen: Vec<String> = Vec::new();
    for index in 0..attribute_count {
        let step = index
            .checked_mul(attribute_size)
            .ok_or(ManifestError::PlatformCapacity)?;
        let at = bounded(base, step, end)?;
        bounded(at, 20, end)?;
        let namespace = u32_at(bytes, at)?;
        let attribute_name = u32_at(bytes, at + 4)?;
        let raw = u32_at(bytes, at + 8)?;
        let data_type = *bytes
            .get(at + 15)
            .ok_or(ManifestError::InconsistentChunk { offset: at })?;
        let data = u32_at(bytes, at + 16)?;
        budget.nodes()?;
        let label = attribute_label(pool, attribute_name, budget)?;
        let mut prefix = String::new();
        if namespace != u32::MAX {
            let uri = pool.text(namespace, budget)?;
            if let Some((prefix_index, _)) =
                namespaces.iter().rev().find(|(_, uri_at)| *uri_at == uri)
            {
                prefix = pool.text(*prefix_index, budget)?;
            }
        }
        let mut qualified = String::new();
        if !prefix.is_empty() {
            qualified.push_str(&prefix);
            qualified.push(':');
        }
        qualified.push_str(&label);
        if seen.iter().any(|previous| previous == &qualified) {
            return Err(ManifestError::DuplicateAttribute {
                offset,
                name: qualified,
            });
        }
        budget.write(" ")?;
        budget.write_escaped(&qualified)?;
        seen.push(qualified);
        budget.write("=\"")?;
        let value = attribute_value(
            pool,
            raw,
            data_type,
            data,
            u32::try_from(index).unwrap_or(u32::MAX),
            budget,
        )?;
        budget.write_escaped(&value)?;
        budget.write("\"")?;
    }
    budget.write(">\n")?;
    open.push(name);
    Ok(())
}

fn attribute_label(
    pool: &StringPool<'_>,
    name: u32,
    budget: &mut Budget<'_>,
) -> Result<String, ManifestError> {
    // The original attribute name is the string-pool entry; the resource map is
    // not needed to name it and a guessed id table would mislabel attributes.
    pool.text(name, budget)
}

/// AOSP `complexToFloat`: a signed 24-bit mantissa scaled by one of the four
/// radix multipliers `{1, 1<<7, 1<<15, 1<<23}`.
fn complex_float(data: u32) -> f32 {
    let mantissa = ((data as i32) >> 8) as f32;
    let radix = match (data >> 4) & 0x3 {
        0 => 1.0,
        1 => 1.0 / 128.0,
        2 => 1.0 / 32_768.0,
        _ => 1.0 / 8_388_608.0,
    };
    mantissa * radix
}

fn complex_unit(data: u32) -> &'static str {
    match data & 0xF {
        0 => "px",
        1 => "dip",
        2 => "sp",
        3 => "pt",
        4 => "in",
        5 => "mm",
        _ => "",
    }
}

fn attribute_value(
    pool: &StringPool<'_>,
    raw: u32,
    data_type: u8,
    data: u32,
    index: u32,
    budget: &mut Budget<'_>,
) -> Result<String, ManifestError> {
    let value = match data_type {
        0x00 => String::new(),
        0x01 | 0x07 | 0x08 => format!("@{data:#010x}"),
        0x02 => format!("?{data:#010x}"),
        0x03 => pool.text(data, budget)?,
        0x04 => f32::from_bits(data).to_string(),
        0x05 => format!("{}{}", complex_float(data), complex_unit(data)),
        0x06 => {
            let unit = if data & 0xF == 1 { "%p" } else { "%" };
            format!("{}{unit}", complex_float(data) * 100.0)
        }
        0x10 => i32::from_le_bytes(data.to_le_bytes()).to_string(),
        0x11 => format!("{data:#010x}"),
        0x12 => if data == 0 { "false" } else { "true" }.to_owned(),
        0x1c..=0x1f => format!("#{data:08x}"),
        other => {
            return Err(ManifestError::UnsupportedValue {
                index,
                data_type: other,
            });
        }
    };
    let _ = raw;
    Ok(value)
}

#[cfg(test)]
mod mutation_tests {
    use super::{ManifestError, ManifestLimits, decode};

    fn chunk(kind: u16, header: u16, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&header.to_le_bytes());
        out.extend_from_slice(&u32::try_from(8 + body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn pool(strings: &[&str], styles: bool) -> Vec<u8> {
        let mut offsets = Vec::new();
        let mut data = Vec::new();
        for entry in strings {
            offsets.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
            let units: Vec<u16> = entry.encode_utf16().collect();
            data.extend_from_slice(&u16::try_from(units.len()).unwrap().to_le_bytes());
            for unit in units {
                data.extend_from_slice(&unit.to_le_bytes());
            }
            data.extend_from_slice(&[0, 0]);
        }
        let strings_start = 28 + offsets.len();
        let mut body = Vec::new();
        body.extend_from_slice(&u32::try_from(strings.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&u32::try_from(usize::from(styles)).unwrap().to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&u32::try_from(strings_start).unwrap().to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&offsets);
        body.extend_from_slice(&data);
        let mut chunk = chunk(0x0001, 28, &body);
        if styles {
            // Style spans are outside the decoded text and only need bounded shape.
            let style_start = strings_start + data.len();
            chunk[24..28].copy_from_slice(&u32::try_from(style_start).unwrap().to_le_bytes());
            chunk.extend_from_slice(&[0u8; 4]);
            let size = u32::try_from(chunk.len()).unwrap();
            chunk[4..8].copy_from_slice(&size.to_le_bytes());
        }
        chunk
    }

    fn element(name: u32, attributes: &[(u32, u32)]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        body.extend_from_slice(&20u16.to_le_bytes());
        body.extend_from_slice(&20u16.to_le_bytes());
        body.extend_from_slice(&u16::try_from(attributes.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        for &(attribute, value) in attributes {
            body.extend_from_slice(&u32::MAX.to_le_bytes());
            body.extend_from_slice(&attribute.to_le_bytes());
            body.extend_from_slice(&u32::MAX.to_le_bytes());
            body.extend_from_slice(&8u16.to_le_bytes());
            body.push(0);
            body.push(0x10);
            body.extend_from_slice(&value.to_le_bytes());
        }
        chunk(0x0102, 16, &body)
    }

    fn end(name: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        chunk(0x0103, 16, &body)
    }

    fn document(styles: bool) -> Vec<u8> {
        let mut body = pool(&["root", "child", "value", "deep"], styles);
        body.extend_from_slice(&chunk(0x0180, 8, &0x0101_0015u32.to_le_bytes()));
        body.extend_from_slice(&element(0, &[(2, 7), (3, 9)]));
        body.extend_from_slice(&element(1, &[(2, 1)]));
        body.extend_from_slice(&end(1));
        body.extend_from_slice(&end(0));
        chunk(0x0003, 8, &body)
    }

    /// A damaged AXML document must end in a classified error, never a panic or a
    /// hang. The seed and the offset range are fixed so the case is reproducible.
    #[test]
    fn h03_axml_seeded_mutations_never_panic_or_hang() {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        for styles in [false, true] {
            let original = document(styles);
            let mut seed = 0x9e37_79b9u32;
            let mut accepted = 0u32;
            let mut rejected = 0u32;
            for _ in 0..512 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let mut bytes = original.clone();
                let position = usize::try_from(seed).unwrap() % bytes.len();
                let value = u8::try_from(seed >> 24).unwrap();
                bytes[position] ^= value | 1;
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    decode(&bytes, ManifestLimits::default())
                }));
                match outcome {
                    Ok(Ok(_)) => accepted += 1,
                    Ok(Err(error)) => {
                        let _: ManifestError = error;
                        rejected += 1;
                    }
                    Err(_) => panic!("mutation at byte {position} panicked"),
                }
            }
            assert_eq!(accepted + rejected, 512);
            assert!(rejected > 0, "no mutation was rejected");
        }
        std::panic::set_hook(previous);
    }

    #[test]
    fn h03_axml_rejects_text_records_and_keeps_style_pools_bounded() {
        // A style-aware pool decodes normally; style spans do not alter the text.
        let styled = document(true);
        assert!(decode(&styled, ManifestLimits::default()).is_ok());
        // A text/CDATA record is a classified unsupported record.
        let mut body = pool(&["root"], false);
        body.extend_from_slice(&element(0, &[]));
        body.extend_from_slice(&chunk(0x0104, 16, &[0u8; 12]));
        body.extend_from_slice(&end(0));
        let text = chunk(0x0003, 8, &body);
        assert!(matches!(
            decode(&text, ManifestLimits::default()),
            Err(ManifestError::UnsupportedChunk { kind: 0x0104 })
        ));
    }
}

#[cfg(test)]
mod utf8_tests {
    use super::{ManifestError, ManifestLimits, decode};

    fn chunk(kind: u16, header: u16, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&header.to_le_bytes());
        out.extend_from_slice(&u32::try_from(8 + body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn utf8_pool(strings: &[&str]) -> Vec<u8> {
        let mut offsets = Vec::new();
        let mut data = Vec::new();
        for entry in strings {
            offsets.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
            let bytes = entry.as_bytes();
            data.push(u8::try_from(bytes.len()).unwrap());
            data.push(u8::try_from(entry.encode_utf16().count()).unwrap());
            data.extend_from_slice(bytes);
            data.push(0);
        }
        let strings_start = 28 + offsets.len();
        let mut body = Vec::new();
        body.extend_from_slice(&u32::try_from(strings.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&(1u32 << 8).to_le_bytes());
        body.extend_from_slice(&u32::try_from(strings_start).unwrap().to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&offsets);
        body.extend_from_slice(&data);
        chunk(0x0001, 28, &body)
    }

    fn namespace(kind: u16, prefix: u32, uri: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&prefix.to_le_bytes());
        body.extend_from_slice(&uri.to_le_bytes());
        chunk(kind, 16, &body)
    }

    fn attribute(namespace: u32, name: u32, data_type: u8, data: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&namespace.to_le_bytes());
        out.extend_from_slice(&name.to_le_bytes());
        out.extend_from_slice(&u32::MAX.to_le_bytes());
        out.extend_from_slice(&8u16.to_le_bytes());
        out.push(0);
        out.push(data_type);
        out.extend_from_slice(&data.to_le_bytes());
        out
    }

    fn element(name: u32, attributes: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        body.extend_from_slice(&20u16.to_le_bytes());
        body.extend_from_slice(&20u16.to_le_bytes());
        body.extend_from_slice(&u16::try_from(attributes.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        for attribute in attributes {
            body.extend_from_slice(attribute);
        }
        chunk(0x0102, 16, &body)
    }

    fn end_element(name: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        chunk(0x0103, 16, &body)
    }

    #[test]
    fn h03_axml_reads_a_utf8_pool_with_two_namespaces() {
        // 0:name 1:root 2:child 3:tools 4:toolsUri 5:android 6:androidUri
        // 7:value 8:deep 9:unicode label
        let strings = [
            "name",
            "root",
            "child",
            "tools",
            "http://schemas.android.com/tools",
            "android",
            "http://schemas.android.com/apk/res/android",
            "value",
            "deep",
            "深度",
            "count",
        ];
        let mut body = utf8_pool(&strings);
        body.extend_from_slice(&namespace(0x0100, 3, 4));
        body.extend_from_slice(&namespace(0x0100, 5, 6));
        let attributes = vec![
            attribute(6, 0, 0x03, 7),
            attribute(4, 0, 0x12, 1),
            attribute(u32::MAX, 9, 0x1c, 0xff00_1234),
            attribute(u32::MAX, 10, 0x10, 63),
        ];
        body.extend_from_slice(&element(1, &attributes));
        for _ in 0..32 {
            body.extend_from_slice(&element(2, &[attribute(u32::MAX, 8, 0x11, 0x10)]));
        }
        for _ in 0..32 {
            body.extend_from_slice(&end_element(2));
        }
        body.extend_from_slice(&end_element(1));
        body.extend_from_slice(&namespace(0x0101, 5, 6));
        body.extend_from_slice(&namespace(0x0101, 3, 4));
        let document = chunk(0x0003, 8, &body);
        let xml = decode(&document, ManifestLimits::default()).unwrap();
        assert!(
            xml.starts_with(
                "<root xmlns:tools=\"http://schemas.android.com/tools\" xmlns:android=\"http://schemas.android.com/apk/res/android\" android:name=\"value\" tools:name=\"true\" 深度=\"#ff001234\""
            ),
            "{xml}"
        );
        assert!(xml.contains("深度=\"#ff001234\""), "{xml}");
        assert!(xml.contains("count=\"63\""), "{xml}");
        assert_eq!(xml.matches("<child ").count(), 32);
        assert_eq!(xml.matches("</child>").count(), 32);
        assert!(xml.ends_with("</root>\n"));
    }

    #[test]
    fn h03_axml_rejects_a_repeated_attribute_name() {
        let strings = ["root", "name", "a", "b"];
        let mut body = utf8_pool(&strings);
        body.extend_from_slice(&element(
            0,
            &[
                attribute(u32::MAX, 1, 0x03, 2),
                attribute(u32::MAX, 1, 0x03, 3),
            ],
        ));
        body.extend_from_slice(&end_element(0));
        let document = chunk(0x0003, 8, &body);
        assert!(matches!(
            decode(&document, ManifestLimits::default()),
            Err(ManifestError::DuplicateAttribute { .. })
        ));
    }

    #[test]
    fn h03_axml_rejects_a_damaged_utf8_pool_entry() {
        let strings = ["root", "child"];
        let mut body = utf8_pool(&strings);
        body.extend_from_slice(&element(0, &[]));
        body.extend_from_slice(&end_element(0));
        let mut document = chunk(0x0003, 8, &body);
        // Point the first string offset past the pool data.
        let pool_offsets = 8 + 28;
        document[pool_offsets..pool_offsets + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            decode(&document, ManifestLimits::default()),
            Err(ManifestError::InvalidString { .. })
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{ManifestError, ManifestLimits, decode};

    fn chunk(kind: u16, header: u16, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&header.to_le_bytes());
        out.extend_from_slice(&u32::try_from(8 + body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn string_pool(strings: &[&str]) -> Vec<u8> {
        let mut offsets = Vec::new();
        let mut data = Vec::new();
        for entry in strings {
            offsets.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
            let units: Vec<u16> = entry.encode_utf16().collect();
            data.extend_from_slice(&u16::try_from(units.len()).unwrap().to_le_bytes());
            for unit in units {
                data.extend_from_slice(&unit.to_le_bytes());
            }
            data.extend_from_slice(&[0, 0]);
        }
        let strings_start = 28 + offsets.len();
        let mut body = Vec::new();
        body.extend_from_slice(&u32::try_from(strings.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&u32::try_from(strings_start).unwrap().to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&offsets);
        body.extend_from_slice(&data);
        chunk(0x0001, 28, &body)
    }

    fn namespace(kind: u16) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&3u32.to_le_bytes());
        body.extend_from_slice(&4u32.to_le_bytes());
        chunk(kind, 16, &body)
    }

    fn attribute(namespace: u32, name: u32, data_type: u8, data: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&namespace.to_le_bytes());
        out.extend_from_slice(&name.to_le_bytes());
        out.extend_from_slice(&u32::MAX.to_le_bytes());
        out.extend_from_slice(&8u16.to_le_bytes());
        out.push(0);
        out.push(data_type);
        out.extend_from_slice(&data.to_le_bytes());
        out
    }

    fn start_element(name: u32, attributes: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        body.extend_from_slice(&20u16.to_le_bytes());
        body.extend_from_slice(&20u16.to_le_bytes());
        body.extend_from_slice(&u16::try_from(attributes.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        for attribute in attributes {
            body.extend_from_slice(attribute);
        }
        chunk(0x0102, 16, &body)
    }

    fn end_element(name: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        chunk(0x0103, 16, &body)
    }

    fn fixture(package: &str) -> Vec<u8> {
        let strings = [
            "versionCode",
            "package",
            "manifest",
            "android",
            "http://schemas.android.com/apk/res/android",
            package,
        ];
        let mut body = string_pool(&strings);
        let mut resource_map = Vec::new();
        resource_map.extend_from_slice(&0x0101_0015u32.to_le_bytes());
        body.extend_from_slice(&chunk(0x0180, 8, &resource_map));
        body.extend_from_slice(&namespace(0x0100));
        body.extend_from_slice(&start_element(
            2,
            &[attribute(4, 0, 0x10, 7), attribute(u32::MAX, 1, 0x03, 5)],
        ));
        body.extend_from_slice(&end_element(2));
        body.extend_from_slice(&namespace(0x0101));
        chunk(0x0003, 8, &body)
    }

    #[test]
    fn h03_axml_decodes_elements_attributes_and_android_ids() {
        let bytes = fixture("pkg");
        let xml = decode(&bytes, ManifestLimits::default()).unwrap();
        assert_eq!(
            xml,
            "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\" android:versionCode=\"7\" package=\"pkg\">\n</manifest>\n"
        );
    }

    #[test]
    fn h03_axml_rejects_damaged_or_unsupported_documents() {
        let good = fixture("pkg");
        assert!(matches!(
            decode(&good[..good.len() - 1], ManifestLimits::default()),
            Err(ManifestError::InconsistentChunk { .. })
        ));
        let mut not_axml = good.clone();
        not_axml[0] = 0x01;
        assert!(matches!(
            decode(&not_axml, ManifestLimits::default()),
            Err(ManifestError::NotAxml)
        ));
        // Replace the trailing end-namespace record with an unknown kind.
        let mut unsupported = good.clone();
        let at = unsupported.len() - 24;
        unsupported[at..at + 2].copy_from_slice(&0x0999u16.to_le_bytes());
        assert!(matches!(
            decode(&unsupported, ManifestLimits::default()),
            Err(ManifestError::UnsupportedChunk { kind: 0x0999 })
        ));
        let mut truncated = good.clone();
        truncated[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            decode(&truncated, ManifestLimits::default()),
            Err(ManifestError::InconsistentChunk { .. })
        ));
        // A string index outside the pool is rejected, not rendered empty.
        let mut bad_string = good.clone();
        let first = bad_string
            .windows(2)
            .position(|window| window == 0x0102u16.to_le_bytes())
            .unwrap();
        bad_string[first + 20..first + 24].copy_from_slice(&99u32.to_le_bytes());
        assert!(matches!(
            decode(&bad_string, ManifestLimits::default()),
            Err(ManifestError::InvalidString { .. })
        ));
    }

    #[test]
    fn h03_axml_formats_dimension_and_fraction_from_complex_values() {
        // 0x00800021: mantissa 0x8000 with radix 2 (1/32768) and unit dip.
        // 0x00800021: the same mantissa/radix with fraction unit %p.
        let mut body = string_pool(&["root", "width", "ratio"]);
        body.extend_from_slice(&start_element(
            0,
            &[
                attribute(u32::MAX, 1, 0x05, 0x0080_0021),
                attribute(u32::MAX, 2, 0x06, 0x0080_0021),
            ],
        ));
        body.extend_from_slice(&end_element(0));
        let document = chunk(0x0003, 8, &body);
        let xml = decode(&document, ManifestLimits::default()).unwrap();
        assert!(xml.contains("width=\"1dip\""), "{xml}");
        assert!(xml.contains("ratio=\"100%p\""), "{xml}");
    }

    #[test]
    fn h03_axml_node_and_output_bounds_are_explicit() {
        let bytes = fixture("pkg");
        assert!(matches!(
            decode(
                &bytes,
                ManifestLimits {
                    max_nodes: Some(1),
                    ..ManifestLimits::default()
                }
            ),
            Err(ManifestError::Budget { .. })
        ));
        assert!(matches!(
            decode(
                &bytes,
                ManifestLimits {
                    max_output_bytes: Some(4),
                    ..ManifestLimits::default()
                }
            ),
            Err(ManifestError::Budget { .. })
        ));
        assert!(matches!(
            decode(
                &bytes,
                ManifestLimits {
                    max_string_units: Some(1),
                    ..ManifestLimits::default()
                }
            ),
            Err(ManifestError::Budget { .. })
        ));
    }
}
