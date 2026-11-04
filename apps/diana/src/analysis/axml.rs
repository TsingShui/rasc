//! Bounded Android binary XML (AXML) reader for `AndroidManifest.xml`.
//!
//! The container is a chunked format: a file header, a string pool, an optional
//! resource map and a flat sequence of namespace/element records. Every length,
//! offset and index is checked against the remaining bytes before use, nesting is
//! bounded by the actual input, and an unsupported record or typed value is a
//! classified error instead of a guessed document.

use thiserror::Error;

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
    #[error("manifest value is not representable on this platform")]
    PlatformCapacity,
}

impl ManifestError {
    pub fn kind(&self) -> diana_decomp::FailureKind {
        use diana_decomp::FailureKind as Kind;
        match self {
            Self::PlatformCapacity => Kind::ResourceExhausted,
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
const ANDROID_NAMESPACE: &str = "http://schemas.android.com/apk/res/android";
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
    fn payload(&self, index: u32) -> Result<&'a [u8], ManifestError> {
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
            // UTF-16 unit length.
            let (byte_length, after_byte_length) = utf8_prefix(record, 0, index)?;
            let (_, payload_start) = utf8_prefix(record, after_byte_length, index)?;

            let length =
                usize::try_from(byte_length).map_err(|_| ManifestError::PlatformCapacity)?;
            record
                .get(payload_start..payload_start + length)
                .ok_or(ManifestError::InvalidString { index })
        } else {
            let units = u32::from(u16_at(record, 0)?);

            let length = usize::try_from(units)
                .map_err(|_| ManifestError::PlatformCapacity)?
                .checked_mul(2)
                .ok_or(ManifestError::PlatformCapacity)?;
            record
                .get(2..2 + length)
                .ok_or(ManifestError::InvalidString { index })
        }
    }

    fn text(&self, index: u32) -> Result<String, ManifestError> {
        let payload = self.payload(index)?;
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

fn describe(kind: &ManifestWarningKind) -> String {
    match kind {
        ManifestWarningKind::TruncatedChunk => "chunk truncated; decoding stopped".to_owned(),
        ManifestWarningKind::UnknownRecord(kind) => {
            format!("record {kind:#06x} not supported; skipped")
        }
        ManifestWarningKind::SkippedText => {
            "text record skipped (text nodes are not rendered)".to_owned()
        }
        ManifestWarningKind::InvalidString(index) => {
            format!("string {index} outside the pool; empty value used")
        }
        ManifestWarningKind::DuplicateAttribute(name) => {
            format!("attribute {name} repeated; first value kept")
        }
        ManifestWarningKind::RenamedName { original, emitted } => {
            format!("name {original:?} rewritten as {emitted:?}")
        }
        ManifestWarningKind::ResolvedAttribute { resource, emitted } => {
            format!("attribute {resource:#010x} resolved as android:{emitted}")
        }
        ManifestWarningKind::IdentifiedAttribute { resource } => {
            format!("attribute id {resource:#010x} has no known name")
        }
        ManifestWarningKind::UnsupportedValue(kind) => {
            format!("typed value {kind:#04x} unsupported; raw value used")
        }
        ManifestWarningKind::InvalidName(name) => {
            format!("element name {name:?} cannot appear in XML; subtree skipped")
        }
        ManifestWarningKind::UnbalancedElement => "unbalanced element closed".to_owned(),
        ManifestWarningKind::BrokenStringPool => "string pool unreadable".to_owned(),
    }
}

struct XmlOutput {
    output: String,
}

impl XmlOutput {
    fn write(&mut self, text: &str) -> Result<(), ManifestError> {
        let needed = self
            .output
            .len()
            .checked_add(text.len())
            .ok_or(ManifestError::PlatformCapacity)?;

        self.output
            .try_reserve(needed - self.output.len())
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

impl ManifestWarningKind {
    /// Human-readable explanation used by text output and JSON details.
    pub fn describe(&self) -> String {
        describe(self)
    }

    /// Stable code used by the JSON result and by diagnostics.
    pub fn code(&self) -> &'static str {
        match self {
            Self::TruncatedChunk => "truncated_chunk",
            Self::UnknownRecord(_) => "unknown_record",
            Self::SkippedText => "text_unavailable",
            Self::InvalidString(_) => "invalid_string",
            Self::DuplicateAttribute(_) => "duplicate_attribute",
            Self::RenamedName { .. } => "renamed_name",
            Self::ResolvedAttribute { .. } => "resolved_attribute",
            Self::IdentifiedAttribute { .. } => "identified_attribute",
            Self::UnsupportedValue(_) => "unsupported_value",
            Self::UnbalancedElement => "unbalanced_element",
            Self::BrokenStringPool => "broken_string_pool",
            Self::InvalidName(_) => "invalid_name",
        }
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
    /// A text/CDATA record could not be resolved; its string was empty or invalid.
    SkippedText,
    /// A string index was outside the pool; an empty value was used.
    InvalidString(u32),
    /// A repeated attribute name was dropped, keeping the first value.
    DuplicateAttribute(String),
    /// An attribute or element name was rewritten to a legal XML name. The value
    /// is kept, so the document stays usable and the change stays explicit.
    RenamedName { original: String, emitted: String },
    /// The attribute name string was empty, so the public resource id identifies
    /// the attribute; the emitted name comes from the Android attribute table.
    ResolvedAttribute { resource: u32, emitted: String },
    /// The attribute name string was empty and the resource id is not a known
    /// Android attribute; the id itself is emitted instead of a guessed name.
    IdentifiedAttribute { resource: u32 },
    /// An element name could not be represented; its subtree was skipped.
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
pub fn decode(bytes: &[u8]) -> Result<DecodedManifest, ManifestError> {
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
        output: XmlOutput {
            output: String::new(),
        },
        warnings: Vec::new(),
        pool: None,
        namespaces: Vec::new(),
        resource_map: Vec::new(),
        needs_android_namespace: false,
        declared_android_namespace: false,
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
    let mut xml = decoder.output.output;
    if decoder.needs_android_namespace && !decoder.declared_android_namespace {
        // Attributes named through the Android table need the bound prefix, or the
        // text is not parseable XML.
        if let Some(line_end) = xml.find('\n')
            && let Some(position) = xml[..line_end].rfind('>')
        {
            xml.insert_str(position, &format!(" xmlns:android=\"{ANDROID_NAMESPACE}\""));
        }
    }
    Ok(DecodedManifest {
        xml,
        warnings: decoder.warnings,
    })
}

struct Decoder<'a> {
    bytes: &'a [u8],
    output: XmlOutput,
    warnings: Vec<ManifestWarning>,
    pool: Option<StringPool<'a>>,
    namespaces: Vec<(String, String)>,
    resource_map: Vec<u32>,
    needs_android_namespace: bool,
    declared_android_namespace: bool,
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

    /// Writes a warning where it occurred and records it for JSON/stderr.
    fn comment(&mut self, offset: usize, kind: ManifestWarningKind) -> Result<(), ManifestError> {
        let depth = self.open.len();
        let mut text = format!("diana: byte {offset}: {}", describe(&kind));
        if text.contains("--") {
            text = text.replace("--", "- -");
        }
        self.output.indent(depth)?;
        self.output.write("<!-- ")?;
        self.output.write(&text)?;
        self.output.write(" -->\n")?;
        self.warning(offset, kind)
    }

    /// Inserts buffered element-level warnings before the element start tag.
    fn insert_comments(
        &mut self,
        at: usize,
        depth: usize,
        pending: Vec<(usize, ManifestWarningKind)>,
    ) -> Result<(), ManifestError> {
        let mut comments = String::new();
        for (offset, kind) in pending {
            let mut text = format!("diana: byte {offset}: {}", describe(&kind));
            if text.contains("--") {
                text = text.replace("--", "- -");
            }
            for _ in 0..depth {
                comments.push_str("    ");
            }
            comments.push_str("<!-- ");
            comments.push_str(&text);
            comments.push_str(" -->\n");
            self.warning(offset, kind)?;
        }
        let needed = self
            .output
            .output
            .len()
            .checked_add(comments.len())
            .ok_or(ManifestError::PlatformCapacity)?;

        self.output
            .output
            .try_reserve(needed - self.output.output.len())
            .map_err(|_| ManifestError::PlatformCapacity)?;
        self.output.output.insert_str(at, &comments);
        Ok(())
    }

    fn text(&mut self, offset: usize, index: u32) -> Result<String, ManifestError> {
        if let Some(pool) = self.pool.as_ref() {
            match pool.text(index) {
                Ok(text) => return Ok(text),
                Err(ManifestError::InvalidString { .. }) => {
                    self.warning(offset, ManifestWarningKind::InvalidString(index))?;
                    return Ok(String::new());
                }
                Err(ManifestError::PlatformCapacity) => {
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
                self.comment(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            }
            let kind = u16_at(self.bytes, offset)?;
            let chunk_header = usize::from(u16_at(self.bytes, offset + 2)?);
            let chunk_size = match usize::try_from(u32_at(self.bytes, offset + 4)?) {
                Ok(size) => size,
                Err(_) => {
                    self.comment(offset, ManifestWarningKind::TruncatedChunk)?;
                    break;
                }
            };
            if chunk_header < 8 || chunk_size < chunk_header {
                self.comment(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            }
            let Some(chunk_end) = offset.checked_add(chunk_size) else {
                self.comment(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            };
            if chunk_end > end {
                self.comment(offset, ManifestWarningKind::TruncatedChunk)?;
                break;
            }
            match kind {
                RES_STRING_POOL_TYPE => {
                    match StringPool::parse(self.bytes, offset, chunk_header, chunk_end) {
                        Ok(pool) => self.pool = Some(pool),
                        Err(ManifestError::PlatformCapacity) => {
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
                    } else {
                        let start = offset + chunk_header;
                        for at in (start..chunk_end).step_by(4) {
                            let value = u32_at(self.bytes, at)?;
                            self.resource_map
                                .try_reserve(1)
                                .map_err(|_| ManifestError::PlatformCapacity)?;
                            self.resource_map.push(value);
                        }
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
                    if self.skip.is_none() {
                        // ResXMLTree_cdataExt: the text is a string-pool index at
                        // +16; style spans are not rendered yet.
                        let index = u32_at(self.bytes, offset + 16)?;
                        let text = self.text(offset, index)?;
                        if !text.is_empty() {
                            let depth = self.open.len();
                            self.output.indent(depth)?;
                            self.output.write_escaped(&text)?;
                            self.output.write("\n")?;
                        }
                    }
                }
                other => {
                    if self.skip.is_none() {
                        self.comment(offset, ManifestWarningKind::UnknownRecord(other))?;
                    } else {
                        self.warning(offset, ManifestWarningKind::UnknownRecord(other))?;
                    }
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

        let depth = self.open.len();
        self.output.indent(depth)?;
        self.output.write("<")?;
        self.output.write_escaped(&name)?;
        if self.open.is_empty() {
            for (prefix, uri) in self.namespaces.clone() {
                if uri.is_empty() {
                    // An empty namespace declaration would undeclare the default
                    // namespace and carries no information here.
                    continue;
                }
                if prefix == "android" || uri == ANDROID_NAMESPACE {
                    self.declared_android_namespace = true;
                }
                self.output.write(" xmlns")?;
                if !prefix.is_empty() {
                    self.output.write(":")?;
                    self.output.write_escaped(&prefix)?;
                }
                self.output.write("=\"")?;
                self.output.write_escaped(&uri)?;
                self.output.write("\"")?;
            }
        }
        let start = self.output.output.len();
        let mut pending: Vec<(usize, ManifestWarningKind)> = Vec::new();
        self.write_attributes(
            offset,
            attribute_start,
            attribute_size,
            attribute_count,
            end,
            &mut pending,
        )?;
        self.output.write(">\n")?;
        if !pending.is_empty() {
            self.insert_comments(start, depth, pending)?;
        }
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
        pending: &mut Vec<(usize, ManifestWarningKind)>,
    ) -> Result<(), ManifestError> {
        if attribute_count != 0 && attribute_size < 20 {
            pending.push((offset, ManifestWarningKind::TruncatedChunk));
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
                pending.push((offset, ManifestWarningKind::TruncatedChunk));
                break;
            };
            if at.checked_add(20).is_none_or(|limit| limit > end) {
                pending.push((at, ManifestWarningKind::TruncatedChunk));
                break;
            }
            let namespace = self.namespace_uri(at)?;
            let attribute_name = u32_at(self.bytes, at + 4)?;
            let raw = u32_at(self.bytes, at + 8)?;
            let data_type = self.bytes[at + 15];
            let data = u32_at(self.bytes, at + 16)?;
            let (label, synthetic_android) = self.attribute_label(at, attribute_name)?;
            let mut qualified = String::new();
            if let Some(prefix) = namespace {
                qualified.push_str(&prefix);
                qualified.push(':');
            } else if synthetic_android {
                qualified.push_str("android:");
                self.needs_android_namespace = true;
            }
            qualified.push_str(&label);
            if !valid_namespaced_name(&qualified) {
                let emitted = sanitize_name(&qualified);
                pending.push((
                    at,
                    ManifestWarningKind::RenamedName {
                        original: qualified,
                        emitted: emitted.clone(),
                    },
                ));
                qualified = emitted;
            }
            if seen.iter().any(|previous| previous == &qualified) {
                pending.push((at, ManifestWarningKind::DuplicateAttribute(qualified)));
                continue;
            }
            let value = match attribute_value(
                self.pool.as_ref(),
                raw,
                data_type,
                data,
                u32::try_from(index).unwrap_or(u32::MAX),
            ) {
                Ok(value) => value,
                Err(ManifestError::UnsupportedValue { data_type, .. }) => {
                    pending.push((at, ManifestWarningKind::UnsupportedValue(data_type)));
                    if raw != u32::MAX {
                        self.text(at, raw)?
                    } else {
                        String::new()
                    }
                }
                Err(other) => return Err(other),
            };

            self.output.write(" ")?;
            self.output.write_escaped(&qualified)?;
            self.output.write("=\"")?;
            self.output.write_escaped(&value)?;
            self.output.write("\"")?;
            seen.try_reserve(1)
                .map_err(|_| ManifestError::PlatformCapacity)?;
            seen.push(qualified);
        }
        Ok(())
    }

    /// Resolves one attribute name. A non-empty legal string-pool name wins; an
    /// empty or illegal one is resolved through the resource map, first to the
    /// public Android attribute name and otherwise to the id itself.
    fn attribute_label(
        &mut self,
        at: usize,
        name_index: u32,
    ) -> Result<(String, bool), ManifestError> {
        let text = self.text(at, name_index)?;
        if valid_name(&text) {
            return Ok((text, false));
        }
        let resource = usize::try_from(name_index)
            .ok()
            .and_then(|index| self.resource_map.get(index).copied());
        if let Some(resource) = resource {
            if let Some(name) = super::attr_names::android_attribute(resource) {
                self.warning(
                    at,
                    ManifestWarningKind::ResolvedAttribute {
                        resource,
                        emitted: name.to_owned(),
                    },
                )?;
                return Ok((name.to_owned(), true));
            }
            self.warning(at, ManifestWarningKind::IdentifiedAttribute { resource })?;
            return Ok((format!("attr_{resource:#010x}"), false));
        }
        Ok((text, false))
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

        let depth = self.open.len();
        self.output.indent(depth)?;
        self.output.write("</")?;
        self.output.write_escaped(&opened)?;
        self.output.write(">\n")?;
        Ok(())
    }

    fn close_open(&mut self) -> Result<(), ManifestError> {
        while let Some(name) = self.open.pop() {
            let depth = self.open.len();
            self.output.indent(depth)?;
            self.output
                .write("<!-- diana: missing records; this element was closed here -->\n")?;
            self.warning(0, ManifestWarningKind::UnbalancedElement)?;

            let depth = self.open.len();
            self.output.indent(depth)?;
            self.output.write("</")?;
            self.output.write_escaped(&name)?;
            self.output.write(">\n")?;
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

/// Rewrites one name into the XML name production without dropping the value.
fn sanitize_name(name: &str) -> String {
    let mut emitted = String::with_capacity(name.len());
    for (position, character) in name.chars().enumerate() {
        let accepted = if position == 0 {
            character.is_alphabetic() || character == '_'
        } else {
            character.is_alphanumeric() || matches!(character, '_' | '-' | '.')
        };
        emitted.push(if accepted { character } else { '_' });
    }
    if emitted.is_empty()
        || emitted
            .chars()
            .next()
            .is_some_and(|first| !(first.is_alphabetic() || first == '_'))
    {
        emitted.insert(0, '_');
    }
    emitted
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
) -> Result<String, ManifestError> {
    let value = match data_type {
        0x00 => String::new(),
        0x01 | 0x07 | 0x08 => format!("@{data:#010x}"),
        0x02 => format!("?{data:#010x}"),
        0x03 => match pool {
            Some(pool) => pool.text(data)?,
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
