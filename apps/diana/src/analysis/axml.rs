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
    #[error("string pool is absent or unreadable at byte {offset}")]
    BrokenStringPool { offset: usize },
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
            | Self::DuplicateAttribute { .. }
            | Self::BrokenStringPool { .. } => Kind::Malformed,
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
const RES_XML_CDATA_TYPE: u16 = 0x0104;

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
/// A decoded manifest and every recoverable defect met while decoding it.
///
/// A warning never removes the whole document: the partial text stays balanced
/// XML and is returned with the classified warning that explains the gap.
#[derive(Debug)]
pub struct DecodedManifest {
    xml: String,
    warnings: Vec<ManifestWarning>,
}

impl DecodedManifest {
    pub fn xml(&self) -> &str {
        &self.xml
    }
    pub fn warnings(&self) -> &[ManifestWarning] {
        &self.warnings
    }
    pub fn is_partial(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// One recoverable defect: the original byte offset and what was left out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestWarning {
    pub offset: usize,
    pub kind: ManifestWarningKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestWarningKind {
    /// A chunk header or extent contradicts the remaining bytes; decoding stops
    /// after closing the elements that were still open.
    TruncatedChunk,
    /// A record kind outside the supported set was skipped.
    UnknownRecord(u16),
    /// A text/CDATA record was skipped; text nodes are not rendered yet.
    SkippedText,
    /// A string index was outside the pool; an empty value was used.
    InvalidString(u32),
    /// A repeated attribute name was dropped, keeping the first value.
    DuplicateAttribute(String),
    /// An attribute name cannot appear in XML; the attribute was dropped.
    InvalidName(String),
    /// A typed value outside the supported set was replaced by its raw data.
    UnsupportedValue(u8),
    /// An end element did not match the open element; the open one was closed.
    UnbalancedElement,
    /// A string pool was absent or unreadable; names could not be resolved.
    BrokenStringPool,
}

/// Decodes one binary `AndroidManifest.xml` document into text XML.
///
/// Only a document that is not binary XML, an exceeded explicit bound or a
/// platform-capacity failure is fatal. Every other defect is recoverable: the
/// reader skips the damaged record, records a warning with its byte offset and
/// closes open elements so the produced text stays balanced XML.
pub fn decode(bytes: &[u8], limits: ManifestLimits) -> Result<DecodedManifest, ManifestError> {
    if u16_at(bytes, 0)? != RES_XML_TYPE {
        return Err(ManifestError::NotAxml);
    }
    let header = usize::from(u16_at(bytes, 2)?);
    let size = usize::try_from(u32_at(bytes, 4)?).map_err(|_| ManifestError::PlatformCapacity)?;
    if header < 8 || header > size {
        return Err(ManifestError::InconsistentChunk { offset: 0 });
    }
    let mut decoder = Decoder {
        bytes,
        budget: Budget {
            limits,
            nodes: 0,
            units: 0,
            output: String::new(),
            _marker: std::marker::PhantomData,
        },
        warnings: Vec::new(),
        pool: None,
        namespaces: Vec::new(),
        open: Vec::new(),
        skip: None,
        raw_depth: 0,
    };
    let mut end = size.min(bytes.len());
    if size > bytes.len() {
        decoder.warning(0, ManifestWarningKind::TruncatedChunk)?;
        end = bytes.len();
    }
    decoder.run(header, end)?;
    decoder.close_open()?;
    Ok(DecodedManifest {
        xml: decoder.budget.output,
        warnings: decoder.warnings,
    })
}

struct Decoder<'a> {
    bytes: &'a [u8],
    budget: Budget<'a>,
    warnings: Vec<ManifestWarning>,
    pool: Option<StringPool<'a>>,
    namespaces: Vec<(String, String)>,
    open: Vec<String>,
    skip: Option<usize>,
    raw_depth: usize,
}

impl Decoder<'_> {
    fn warning(&mut self, offset: usize, kind: ManifestWarningKind) -> Result<(), ManifestError> {
        self.warnings
            .try_reserve(1)
            .map_err(|_| ManifestError::PlatformCapacity)?;
        self.warnings.push(ManifestWarning { offset, kind });
        Ok(())
    }

    fn text(&mut self, offset: usize, index: u32) -> Result<String, ManifestError> {
        if let Some(pool) = self.pool.as_ref() {
            match pool.text(index, &mut self.budget) {
                Ok(text) => return Ok(text),
                Err(ManifestError::InvalidString { .. }) => {
                    self.warning(offset, ManifestWarningKind::InvalidString(index))?;
                    return Ok(String::new());
                }
                Err(ManifestError::Budget { .. }) | Err(ManifestError::PlatformCapacity) => {
                    return Err(ManifestError::PlatformCapacity);
                }
                Err(_) => {
                    self.warning(offset, ManifestWarningKind::BrokenStringPool)?;
                    return Ok(String::new());
                }
            }
        }
        self.warning(offset, ManifestWarningKind::BrokenStringPool)?;
        Ok(String::new())
    }

    fn run(&mut self, mut offset: usize, end: usize) -> Result<(), ManifestError> {
        while offset < end {
            if offset + 8 > end {
                self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            }
            let kind = u16_at(self.bytes, offset)?;
            let chunk_header = usize::from(u16_at(self.bytes, offset + 2)?);
            let chunk_size = match usize::try_from(u32_at(self.bytes, offset + 4)?) {
                Ok(size) => size,
                Err(_) => {
                    self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
                    break;
                }
            };
            if chunk_header < 8 || chunk_size < chunk_header {
                self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            }
            let Some(chunk_end) = offset.checked_add(chunk_size) else {
                self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            };
            if chunk_end > end {
                self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            }
            match kind {
                RES_STRING_POOL_TYPE => {
                    match StringPool::parse(self.bytes, offset, chunk_header, chunk_end) {
                        Ok(pool) => self.pool = Some(pool),
                        Err(ManifestError::Budget { .. })
                        | Err(ManifestError::PlatformCapacity) => {
                            return Err(ManifestError::PlatformCapacity);
                        }
                        Err(_) => {
                            self.warning(offset, ManifestWarningKind::BrokenStringPool)?;
                        }
                    }
                }
                RES_XML_RESOURCE_MAP_TYPE => {
                    if !(chunk_end - offset - chunk_header).is_multiple_of(4) {
                        self.warning(offset, ManifestWarningKind::BrokenStringPool)?;
                    }
                }
                RES_XML_START_NAMESPACE_TYPE => {
                    if chunk_header < 16 {
                        self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
                    } else {
                        let prefix = u32_at(self.bytes, offset + 16)?;
                        let uri = u32_at(self.bytes, offset + 20)?;
                        let uri_text = self.text(offset, uri)?;
                        let prefix_text = self.text(offset, prefix)?;
                        self.namespaces
                            .try_reserve(1)
                            .map_err(|_| ManifestError::PlatformCapacity)?;
                        self.namespaces.push((prefix_text, uri_text));
                        self.budget.nodes()?;
                    }
                }
                RES_XML_END_NAMESPACE_TYPE => {
                    self.namespaces.pop();
                }
                RES_XML_START_ELEMENT_TYPE => {
                    self.raw_depth = self.raw_depth.saturating_add(1);
                    self.start_element(offset, chunk_header, chunk_end)?;
                }
                RES_XML_END_ELEMENT_TYPE => {
                    self.end_element(offset, chunk_header)?;
                    self.raw_depth = self.raw_depth.saturating_sub(1);
                    if let Some(depth) = self.skip
                        && self.raw_depth <= depth
                    {
                        self.skip = None;
                    }
                }
                RES_XML_CDATA_TYPE => {
                    self.warning(offset, ManifestWarningKind::SkippedText)?;
                }
                other => {
                    self.warning(offset, ManifestWarningKind::UnknownRecord(other))?;
                }
            }
            offset = chunk_end;
        }
        Ok(())
    }

    fn start_element(
        &mut self,
        offset: usize,
        header: usize,
        end: usize,
    ) -> Result<(), ManifestError> {
        if header < 16 {
            self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
            return Ok(());
        }
        let name_index = u32_at(self.bytes, offset + 20)?;
        let attribute_start = usize::from(u16_at(self.bytes, offset + 24)?);
        let attribute_size = usize::from(u16_at(self.bytes, offset + 26)?);
        let attribute_count = usize::from(u16_at(self.bytes, offset + 28)?);
        let name = self.text(offset, name_index)?;
        if self.skip.is_some() {
            return Ok(());
        }
        if !valid_name(&name) {
            self.warning(offset, ManifestWarningKind::InvalidName(name))?;
            self.skip = Some(self.raw_depth - 1);
            return Ok(());
        }
        self.budget.nodes()?;
        let depth = self.open.len();
        self.budget.indent(depth)?;
        self.budget.write("<")?;
        self.budget.write_escaped(&name)?;
        if self.open.is_empty() {
            for (prefix, uri) in self.namespaces.clone() {
                self.budget.write(" xmlns")?;
                if !prefix.is_empty() {
                    self.budget.write(":")?;
                    self.budget.write_escaped(&prefix)?;
                }
                self.budget.write("=\"")?;
                self.budget.write_escaped(&uri)?;
                self.budget.write("\"")?;
            }
        }
        self.write_attributes(
            offset,
            attribute_start,
            attribute_size,
            attribute_count,
            end,
        )?;
        self.budget.write(">\n")?;
        self.open.push(name);
        Ok(())
    }

    fn write_attributes(
        &mut self,
        offset: usize,
        attribute_start: usize,
        attribute_size: usize,
        attribute_count: usize,
        end: usize,
    ) -> Result<(), ManifestError> {
        if attribute_count != 0 && attribute_size < 20 {
            self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
            return Ok(());
        }
        let Some(base) = offset
            .checked_add(16)
            .and_then(|start| start.checked_add(attribute_start))
        else {
            self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
            return Ok(());
        };
        let mut seen: Vec<String> = Vec::new();
        for index in 0..attribute_count {
            let Some(at) = index
                .checked_mul(attribute_size)
                .and_then(|step| base.checked_add(step))
            else {
                self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            };
            if at.checked_add(20).is_none_or(|limit| limit > end) {
                self.warning(at, ManifestWarningKind::TruncatedChunk)?;
                break;
            }
            let namespace = self.namespace_uri(at)?;
            let attribute_name = u32_at(self.bytes, at + 4)?;
            let raw = u32_at(self.bytes, at + 8)?;
            let data_type = self.bytes[at + 15];
            let data = u32_at(self.bytes, at + 16)?;
            let label = self.text(at, attribute_name)?;
            let mut qualified = String::new();
            if let Some(prefix) = namespace {
                qualified.push_str(&prefix);
                qualified.push(':');
            }
            qualified.push_str(&label);
            if !valid_namespaced_name(&qualified) {
                self.warning(at, ManifestWarningKind::InvalidName(qualified))?;
                continue;
            }
            if seen.iter().any(|previous| previous == &qualified) {
                self.warning(at, ManifestWarningKind::DuplicateAttribute(qualified))?;
                continue;
            }
            let value = match attribute_value(
                self.pool.as_ref(),
                raw,
                data_type,
                data,
                u32::try_from(index).unwrap_or(u32::MAX),
                &mut self.budget,
            ) {
                Ok(value) => value,
                Err(ManifestError::UnsupportedValue { data_type, .. }) => {
                    self.warning(at, ManifestWarningKind::UnsupportedValue(data_type))?;
                    if raw != u32::MAX {
                        self.text(at, raw)?
                    } else {
                        String::new()
                    }
                }
                Err(other) => return Err(other),
            };
            self.budget.nodes()?;
            self.budget.write(" ")?;
            self.budget.write_escaped(&qualified)?;
            self.budget.write("=\"")?;
            self.budget.write_escaped(&value)?;
            self.budget.write("\"")?;
            seen.try_reserve(1)
                .map_err(|_| ManifestError::PlatformCapacity)?;
            seen.push(qualified);
        }
        Ok(())
    }

    fn namespace_uri(&mut self, at: usize) -> Result<Option<String>, ManifestError> {
        let index = u32_at(self.bytes, at)?;
        if index == u32::MAX {
            return Ok(None);
        }
        let uri = self.text(at, index)?;
        let prefix = self
            .namespaces
            .iter()
            .rev()
            .find(|(_, candidate)| *candidate == uri)
            .map(|(prefix, _)| prefix.clone());
        Ok(prefix.filter(|prefix| !prefix.is_empty()))
    }

    fn end_element(&mut self, offset: usize, header: usize) -> Result<(), ManifestError> {
        if header < 16 {
            self.warning(offset, ManifestWarningKind::TruncatedChunk)?;
            return Ok(());
        }
        if self.skip.is_some() {
            return Ok(());
        }
        let name = self.text(offset, u32_at(self.bytes, offset + 20)?)?;
        let Some(opened) = self.open.pop() else {
            self.warning(offset, ManifestWarningKind::UnbalancedElement)?;
            return Ok(());
        };
        if opened != name {
            self.warning(offset, ManifestWarningKind::UnbalancedElement)?;
        }
        self.budget.nodes()?;
        let depth = self.open.len();
        self.budget.indent(depth)?;
        self.budget.write("</")?;
        self.budget.write_escaped(&opened)?;
        self.budget.write(">\n")?;
        Ok(())
    }

    fn close_open(&mut self) -> Result<(), ManifestError> {
        while let Some(name) = self.open.pop() {
            self.warning(0, ManifestWarningKind::UnbalancedElement)?;
            self.budget.nodes()?;
            let depth = self.open.len();
            self.budget.indent(depth)?;
            self.budget.write("</")?;
            self.budget.write_escaped(&name)?;
            self.budget.write(">\n")?;
        }
        Ok(())
    }
}

fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_')
        && characters
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

fn valid_namespaced_name(name: &str) -> bool {
    match name.split_once(':') {
        Some((prefix, local)) => valid_name(prefix) && valid_name(local),
        None => valid_name(name),
    }
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
    pool: Option<&StringPool<'_>>,
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
        0x03 => match pool {
            Some(pool) => pool.text(data, budget)?,
            None => return Err(ManifestError::BrokenStringPool { offset: 0 }),
        },
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
mod tests {
    use super::{DecodedManifest, ManifestLimits, ManifestWarningKind, decode};

    fn chunk(kind: u16, header: u16, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&header.to_le_bytes());
        out.extend_from_slice(&u32::try_from(8 + body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn pool(strings: &[&str], utf8: bool, styles: bool) -> Vec<u8> {
        let mut offsets = Vec::new();
        let mut data = Vec::new();
        for entry in strings {
            offsets.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
            if utf8 {
                let bytes = entry.as_bytes();
                data.push(u8::try_from(bytes.len()).unwrap());
                data.push(u8::try_from(entry.encode_utf16().count()).unwrap());
                data.extend_from_slice(bytes);
                data.push(0);
            } else {
                let units: Vec<u16> = entry.encode_utf16().collect();
                data.extend_from_slice(&u16::try_from(units.len()).unwrap().to_le_bytes());
                for unit in units {
                    data.extend_from_slice(&unit.to_le_bytes());
                }
                data.extend_from_slice(&[0, 0]);
            }
        }
        let strings_start = 28 + offsets.len();
        let mut body = Vec::new();
        body.extend_from_slice(&u32::try_from(strings.len()).unwrap().to_le_bytes());
        body.extend_from_slice(&u32::try_from(usize::from(styles)).unwrap().to_le_bytes());
        body.extend_from_slice(&(if utf8 { 1u32 << 8 } else { 0 }).to_le_bytes());
        body.extend_from_slice(&u32::try_from(strings_start).unwrap().to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&offsets);
        body.extend_from_slice(&data);
        let mut out = chunk(0x0001, 28, &body);
        if styles {
            let style_start = strings_start + data.len();
            out[24..28].copy_from_slice(&u32::try_from(style_start).unwrap().to_le_bytes());
            out.extend_from_slice(&[0u8; 4]);
            let size = u32::try_from(out.len()).unwrap();
            out[4..8].copy_from_slice(&size.to_le_bytes());
        }
        out
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
        attribute_raw(namespace, name, u32::MAX, data_type, data)
    }

    fn attribute_raw(namespace: u32, name: u32, raw: u32, data_type: u8, data: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&namespace.to_le_bytes());
        out.extend_from_slice(&name.to_le_bytes());
        out.extend_from_slice(&raw.to_le_bytes());
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

    fn end(name: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        body.extend_from_slice(&name.to_le_bytes());
        chunk(0x0103, 16, &body)
    }

    fn document(strings: &[&str], utf8: bool, styles: bool) -> Vec<u8> {
        let mut body = pool(strings, utf8, styles);
        body.extend_from_slice(&namespace(0x0100, 3, 4));
        body.extend_from_slice(&element(1, &[attribute(4, 0, 0x03, 2)]));
        body.extend_from_slice(&end(1));
        body.extend_from_slice(&namespace(0x0101, 3, 4));
        chunk(0x0003, 8, &body)
    }

    fn warnings(decoded: &DecodedManifest) -> Vec<&ManifestWarningKind> {
        decoded
            .warnings()
            .iter()
            .map(|warning| &warning.kind)
            .collect()
    }

    fn balanced(xml: &str) -> bool {
        let opens =
            xml.matches('<').count() - xml.matches("</").count() - xml.matches("/>").count();
        let closes = xml.matches("</").count();
        opens == closes
    }

    #[test]
    fn h03_axml_decodes_elements_attributes_and_namespaces() {
        let bytes = document(
            &[
                "name",
                "manifest",
                "pkg",
                "android",
                "http://schemas.android.com/apk/res/android",
            ],
            false,
            false,
        );
        let decoded = decode(&bytes, ManifestLimits::default()).unwrap();
        assert!(decoded.warnings().is_empty());
        assert_eq!(
            decoded.xml(),
            "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\" android:name=\"pkg\">\n</manifest>\n"
        );
    }

    #[test]
    fn h03_axml_keeps_a_balanced_partial_document_with_warnings() {
        let bytes = document(
            &["name", "manifest", "pkg", "android", "http://x"],
            false,
            false,
        );
        // Truncate inside the end-element record.
        let cut = &bytes[..bytes.len() - 10];
        let decoded = decode(cut, ManifestLimits::default()).unwrap();
        assert!(decoded.is_partial());
        assert!(balanced(decoded.xml()), "{}", decoded.xml());
        assert!(matches!(
            warnings(&decoded)[0],
            ManifestWarningKind::TruncatedChunk | ManifestWarningKind::UnbalancedElement
        ));

        let mut unknown = document(
            &["name", "manifest", "pkg", "android", "http://x"],
            false,
            false,
        );
        let tail = unknown.len();
        unknown[tail - 24..tail - 22].copy_from_slice(&0x0999u16.to_le_bytes());
        let decoded = decode(&unknown, ManifestLimits::default()).unwrap();
        assert!(warnings(&decoded).contains(&&ManifestWarningKind::UnknownRecord(0x0999)));
        assert!(balanced(decoded.xml()));

        // Text nodes are skipped with a warning instead of failing the document.
        let mut text = document(
            &["name", "manifest", "pkg", "android", "http://x"],
            false,
            false,
        );
        let body = chunk(0x0104, 16, &[0u8; 12]);
        let insert = text.len() - 24;
        text.splice(insert..insert, body);
        let size = u32::try_from(text.len()).unwrap();
        text[4..8].copy_from_slice(&size.to_le_bytes());
        let decoded = decode(&text, ManifestLimits::default()).unwrap();
        assert!(warnings(&decoded).contains(&&ManifestWarningKind::SkippedText));
        assert!(!decoded.xml().is_empty());

        // A repeated attribute keeps the first value and reports the second.
        let mut duplicate = pool(&["root", "a", "b"], false, false);
        duplicate.extend_from_slice(&element(
            0,
            &[
                attribute(u32::MAX, 1, 0x03, 2),
                attribute(u32::MAX, 1, 0x03, 3),
            ],
        ));
        duplicate.extend_from_slice(&end(0));
        let duplicate = chunk(0x0003, 8, &duplicate);
        let decoded = decode(&duplicate, ManifestLimits::default()).unwrap();
        assert_eq!(decoded.xml(), "<root a=\"b\">\n</root>\n");
        assert!(matches!(
            warnings(&decoded)[0],
            ManifestWarningKind::DuplicateAttribute(name) if name == "a"
        ));
    }

    #[test]
    fn h03_axml_reports_bad_names_strings_and_values_as_warnings() {
        // An attribute name that cannot appear in XML is dropped.
        let mut body = pool(&["root", "bad name", "v"], false, false);
        body.extend_from_slice(&element(0, &[attribute(u32::MAX, 1, 0x03, 2)]));
        body.extend_from_slice(&end(0));
        let decoded = decode(&chunk(0x0003, 8, &body), ManifestLimits::default()).unwrap();
        assert_eq!(decoded.xml(), "<root>\n</root>\n");
        assert!(matches!(
            warnings(&decoded)[0],
            ManifestWarningKind::InvalidName(name) if name == "bad name"
        ));

        // An out-of-pool name renders as an empty name plus a warning.
        let mut body = pool(&["root"], false, false);
        body.extend_from_slice(&element(0, &[]));
        body.extend_from_slice(&end(0));
        let mut bad = chunk(0x0003, 8, &body);
        let first = bad
            .windows(2)
            .position(|w| w == 0x0102u16.to_le_bytes())
            .unwrap();
        bad[first + 20..first + 24].copy_from_slice(&99u32.to_le_bytes());
        let decoded = decode(&bad, ManifestLimits::default()).unwrap();
        assert!(matches!(
            warnings(&decoded)[0],
            ManifestWarningKind::InvalidString(99)
        ));

        // An unsupported typed value falls back to the raw string when present.
        let mut body = pool(&["root", "value", "raw"], false, false);
        body.extend_from_slice(&element(0, &[attribute_raw(u32::MAX, 1, 2, 0x7f, 0)]));
        body.extend_from_slice(&end(0));
        let decoded = decode(&chunk(0x0003, 8, &body), ManifestLimits::default()).unwrap();
        assert!(decoded.xml().contains("value=\"raw\""), "{}", decoded.xml());
        assert!(matches!(
            warnings(&decoded)[0],
            ManifestWarningKind::UnsupportedValue(0x7f)
        ));
    }

    #[test]
    fn h03_axml_reads_utf8_pools_style_pools_and_complex_values() {
        let strings = [
            "name", "root", "child", "tools", "http://t", "android", "http://a", "value", "deep",
            "深度", "count", "width", "ratio",
        ];
        let mut body = pool(&strings, true, true);
        body.extend_from_slice(&namespace(0x0100, 3, 4));
        body.extend_from_slice(&namespace(0x0100, 5, 6));
        body.extend_from_slice(&element(
            1,
            &[
                attribute(6, 0, 0x03, 7),
                attribute(4, 0, 0x12, 1),
                attribute(u32::MAX, 9, 0x1c, 0xff00_1234),
                attribute(u32::MAX, 10, 0x10, 63),
                attribute(u32::MAX, 11, 0x05, 0x0080_0021),
                attribute(u32::MAX, 12, 0x06, 0x0080_0021),
            ],
        ));
        body.extend_from_slice(&element(2, &[attribute(u32::MAX, 8, 0x11, 0x10)]));
        body.extend_from_slice(&end(2));
        body.extend_from_slice(&end(1));
        body.extend_from_slice(&namespace(0x0101, 5, 6));
        body.extend_from_slice(&namespace(0x0101, 3, 4));
        let decoded = decode(&chunk(0x0003, 8, &body), ManifestLimits::default()).unwrap();
        assert!(decoded.warnings().is_empty(), "{:?}", decoded.warnings());
        let xml = decoded.xml();
        assert!(xml.starts_with("<root xmlns:tools=\"http://t\" xmlns:android=\"http://a\" android:name=\"value\" tools:name=\"true\" 深度=\"#ff001234\" count=\"63\" width=\"1dip\" ratio=\"100%p\">"), "{xml}");
        assert!(xml.contains("<child deep=\"0x00000010\">"), "{xml}");
    }

    #[test]
    fn h03_axml_rejects_non_axml_and_enforces_explicit_bounds() {
        let bytes = document(
            &["name", "manifest", "pkg", "android", "http://x"],
            false,
            false,
        );
        let mut not_axml = bytes.clone();
        not_axml[0] = 0x01;
        assert!(matches!(
            decode(&not_axml, ManifestLimits::default()),
            Err(super::ManifestError::NotAxml)
        ));
        assert!(matches!(
            decode(
                &bytes,
                ManifestLimits {
                    max_nodes: Some(1),
                    ..ManifestLimits::default()
                }
            ),
            Err(super::ManifestError::Budget { .. })
        ));
        assert!(matches!(
            decode(
                &bytes,
                ManifestLimits {
                    max_output_bytes: Some(4),
                    ..ManifestLimits::default()
                }
            ),
            Err(super::ManifestError::Budget { .. })
        ));
    }

    /// Fixed-seed mutations must never panic or hang, and every result is either a
    /// decoded document (possibly partial) or a classified error.
    #[test]
    fn h03_axml_seeded_mutations_never_panic_or_hang() {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let mut accepted = 0u32;
        for (utf8, styles) in [(false, false), (true, true)] {
            let original = document(
                &["name", "manifest", "pkg", "android", "http://x"],
                utf8,
                styles,
            );
            let mut seed = 0x9e37_79b9u32;
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
                    Ok(Ok(decoded)) => {
                        accepted += 1;
                        assert!(
                            balanced(decoded.xml()),
                            "unbalanced output at byte {position}"
                        );
                    }
                    Ok(Err(_)) => {}
                    Err(_) => panic!("mutation at byte {position} panicked"),
                }
            }
        }
        std::panic::set_hook(previous);
        assert!(accepted > 0);
    }
}
