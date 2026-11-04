use std::fmt;

use serde::{Serialize, Serializer};

use crate::input::catalog::{ClassDataShape, MethodGroup};
use crate::substrate::flags::{ClassAccessFlags, FieldAccessFlags, MethodAccessFlags};
use crate::{
    ByteOffset, ClassDefId, ClassKey, FieldId, FieldStorage, MethodId, OriginalAnnotations,
    RequestError, TypeId,
};

/// A locally read fact. Absence is distinct from damage and deliberate non-reading.
#[derive(Debug)]
pub enum Fact<T> {
    Present(T),
    Absent,
    NotRead,
    Unavailable(RequestError),
}

impl<T> Fact<T> {
    pub fn is_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
    pub fn unavailable(&self) -> Option<&RequestError> {
        match self {
            Self::Unavailable(error) => Some(error),
            _ => None,
        }
    }
}

impl<T: Serialize> Serialize for Fact<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut record = serializer.serialize_struct("Fact", 2)?;
        match self {
            Self::Present(value) => {
                record.serialize_field("state", "present")?;
                record.serialize_field("value", value)?;
            }
            Self::Absent => record.serialize_field("state", "absent")?,
            Self::NotRead => record.serialize_field("state", "not_read")?,
            Self::Unavailable(error) => {
                record.serialize_field("state", "unavailable")?;
                record.serialize_field("error", &ErrorView(error))?;
            }
        }
        record.end()
    }
}

pub(super) struct ErrorView<'a>(pub &'a RequestError);
impl Serialize for ErrorView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut record = serializer.serialize_struct("Error", 2)?;
        record.serialize_field("kind", kind_name(self.0.kind()))?;
        record.serialize_field("detail", &DisplayValue(self.0))?;
        record.end()
    }
}

pub(super) fn kind_name(kind: crate::FailureKind) -> &'static str {
    match kind {
        crate::FailureKind::Malformed => "malformed",
        crate::FailureKind::Unsupported => "unsupported",
        crate::FailureKind::ResourceExhausted => "resource_exhausted",
        crate::FailureKind::InternalInvariant => "internal_invariant",
        crate::FailureKind::Cancelled => "cancelled",
    }
}

pub(super) struct DisplayValue<T>(pub T);
impl<T: fmt::Display> Serialize for DisplayValue<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

/// Exact original UTF-16. Display escapes isolated surrogates; units retain identity.
#[derive(Clone, Debug)]
pub struct OriginalText(Vec<u16>);
impl OriginalText {
    pub(crate) fn new(units: Vec<u16>) -> Self {
        Self(units)
    }
    pub fn units(&self) -> &[u16] {
        &self.0
    }
}
impl fmt::Display for OriginalText {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        for scalar in char::decode_utf16(self.0.iter().copied()) {
            match scalar {
                Ok(character) => write!(output, "{character}")?,
                Err(error) => write!(output, "\\u{:04X}", error.unpaired_surrogate())?,
            }
        }
        Ok(())
    }
}
impl Serialize for OriginalText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut record = serializer.serialize_struct("OriginalText", 1)?;
        record.serialize_field("text", &DisplayValue(self))?;
        record.end()
    }
}

/// The original access word with the per-kind bit names the owning collection
/// read and the remaining bits. Semantic queries are derived from `raw` through
/// the shared per-kind flag types, so this struct never duplicates them.
#[derive(Serialize)]
pub struct OriginalFlags {
    raw: u32,
    names: Vec<&'static str>,
    unknown_bits: u32,
}

impl OriginalFlags {
    /// The original flag word. Semantic queries are derived from it through the
    /// shared per-kind flag type, not by matching this whole value.
    pub fn raw(&self) -> u32 {
        self.raw
    }
}

impl OriginalFlags {
    pub(super) fn new(raw: u32, names: Vec<&'static str>, unknown_bits: u32) -> Self {
        Self {
            raw,
            names,
            unknown_bits,
        }
    }
}

/// Original class identity and declaration. `descriptor` keeps the exact input
/// units; the access word keeps its raw value beside per-kind semantic queries.
#[derive(Serialize)]
pub struct ClassDeclaration {
    pub type_index: u32,
    pub descriptor: OriginalText,
    pub access_flags: OriginalFlags,
    pub superclass: Fact<OriginalText>,
    pub interfaces: Fact<Vec<Fact<OriginalText>>>,
    pub source_file: Fact<OriginalText>,
}

#[derive(Serialize)]
pub struct FieldDeclaration {
    pub name: OriginalText,
    pub descriptor: OriginalText,
}

/// Static preparation bytes exactly as encoded, with the decoded scalar when the
/// format and tag determine one. Implicit and explicit defaults stay distinct.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InitialValue {
    ImplicitDefault,
    Encoded {
        tag: u8,
        value_arg: u8,
        range_bytes: [usize; 2],
        bytes: Vec<u8>,
        scalar: Option<ScalarValue>,
    },
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ScalarValue {
    Signed(i64),
    Unsigned(u64),
    FloatBits(u32),
    DoubleBits(u64),
    Boolean(bool),
    Null,
    String(OriginalText),
}

/// The original method declaration as P0 read it: exact text, prototype type
/// identities, parameter word layout and return type. Downstream consumers use
/// this product instead of resolving the declaration record again.
#[derive(Clone, Serialize)]
pub struct MethodDeclaration {
    pub name: OriginalText,
    pub prototype: OriginalText,
    pub shorty: OriginalText,
    pub parameter_types: Vec<OriginalText>,
    pub parameter_type_ids: Vec<u32>,
    pub return_type: OriginalText,
    pub return_type_id: u32,
    pub parameter_words: u64,
}

impl MethodDeclaration {
    pub fn name_units(&self) -> &[u16] {
        self.name.units()
    }
    pub fn prototype_units(&self) -> &[u16] {
        self.prototype.units()
    }
    pub fn parameter_count(&self) -> usize {
        self.parameter_types.len()
    }
}

/// One original field directory entry: encoded storage group and ordinal, its
/// declaration result and its independent static preparation result.
pub struct FieldDirectoryEntry {
    pub(super) field: FieldId,
    pub(super) access_flags: OriginalFlags,
    pub(super) storage: FieldStorage,
    pub(super) group_ordinal: u32,
    pub(super) declaration: Fact<FieldDeclaration>,
    pub(super) initial: Fact<InitialValue>,
    /// Decoded preparation scalar when the tag and declaration descriptor
    /// determine one; a declaration failure leaves this empty.
    pub(super) scalar: Option<ScalarValue>,
    /// Whether an encoded preparation equals the descriptor's default value.
    /// Implicit defaults are also default; unavailable reads are not.
    pub(super) default_preparation: bool,
}

impl FieldDirectoryEntry {
    pub fn field(&self) -> FieldId {
        self.field
    }
    pub fn access_flags(&self) -> FieldAccessFlags {
        FieldAccessFlags::from_raw(self.access_flags.raw())
    }
    pub fn original_flags(&self) -> &OriginalFlags {
        &self.access_flags
    }
    pub fn storage(&self) -> FieldStorage {
        self.storage
    }
    /// Position inside the original static or instance group.
    pub fn group_ordinal(&self) -> u32 {
        self.group_ordinal
    }
    pub fn declaration(&self) -> &Fact<FieldDeclaration> {
        &self.declaration
    }
    /// Original static preparation, including an implicit default. Independent
    /// of whether the declaration itself was readable.
    pub fn initial_value(&self) -> &Fact<InitialValue> {
        &self.initial
    }
    pub fn scalar(&self) -> Option<&ScalarValue> {
        self.scalar.as_ref()
    }
    pub fn default_preparation(&self) -> bool {
        self.default_preparation
    }
}

/// One original method directory entry. The direct/virtual group and its ordinal
/// come from the encoded class-data group; code offset is the original integer.
pub struct MethodDirectoryEntry {
    pub(super) method: MethodId,
    pub(super) access_flags: OriginalFlags,
    pub(super) group: MethodGroup,
    pub(super) code_offset: Option<ByteOffset>,
    pub(super) declaration: Fact<MethodDeclaration>,
    pub(super) code: Fact<()>,
}

impl MethodDirectoryEntry {
    pub fn method(&self) -> MethodId {
        self.method
    }
    pub fn access_flags(&self) -> MethodAccessFlags {
        MethodAccessFlags::from_raw(self.access_flags.raw())
    }
    pub fn original_flags(&self) -> &OriginalFlags {
        &self.access_flags
    }
    /// The encoded class-data group and the member's position inside it.
    pub fn group(&self) -> MethodGroup {
        self.group
    }
    /// The original class-data code offset; zero means the encoding omitted it.
    pub fn code_offset(&self) -> Option<ByteOffset> {
        self.code_offset
    }
    pub fn declaration(&self) -> &Fact<MethodDeclaration> {
        &self.declaration
    }
    /// Code presence as P0 knows it: `Absent` for an omitted offset, `NotRead`
    /// otherwise. P0 never opens a code item.
    pub fn code(&self) -> &Fact<()> {
        &self.code
    }
}

/// Owned read-only P0 facts: class identity, the original four-group member
/// directory and independently available metadata. Local declaration/metadata
/// damage stays in its own member entry. Method code is not opened.
pub struct ClassFacts {
    pub(super) owner: ClassKey,
    pub(super) header_offset: ByteOffset,
    pub(super) class: ClassDeclaration,
    pub(super) shape: ClassDataShape,
    pub(super) delivered: ClassDataShape,
    pub(super) terminated_at: Option<ByteOffset>,
    pub(super) fields: Vec<FieldDirectoryEntry>,
    pub(super) methods: Vec<MethodDirectoryEntry>,
    pub(super) fields_issue: Option<RequestError>,
    pub(super) methods_issue: Option<RequestError>,
    pub(super) annotations: OriginalAnnotations,
    pub(super) annotations_issue: Option<RequestError>,
}

impl ClassFacts {
    pub fn owner(&self) -> ClassKey {
        self.owner
    }
    pub fn header_offset(&self) -> ByteOffset {
        self.header_offset
    }
    pub fn class_def(&self) -> ClassDefId {
        self.owner.class_def
    }
    pub fn type_index(&self) -> TypeId {
        TypeId(self.class.type_index)
    }
    pub fn descriptor(&self) -> &OriginalText {
        &self.class.descriptor
    }
    pub fn access_flags(&self) -> ClassAccessFlags {
        ClassAccessFlags::from_raw(self.class.access_flags.raw())
    }
    pub fn superclass(&self) -> &Fact<OriginalText> {
        &self.class.superclass
    }
    pub fn interfaces(&self) -> &Fact<Vec<Fact<OriginalText>>> {
        &self.class.interfaces
    }
    pub fn source_file(&self) -> &Fact<OriginalText> {
        &self.class.source_file
    }
    /// Original `class_data` group counts, including groups whose traversal
    /// stopped early. This describes the header, not the delivered directory.
    pub fn class_data_shape(&self) -> ClassDataShape {
        self.shape
    }
    /// Members actually delivered in each original group. This can be a strict
    /// prefix of `class_data_shape` when traversal stopped early.
    pub fn delivered_shape(&self) -> ClassDataShape {
        self.delivered
    }
    /// First original class-data byte that could not be located, if traversal
    /// stopped. `None` means every declared record was located.
    pub fn coverage_terminated_at(&self) -> Option<ByteOffset> {
        self.terminated_at
    }
    /// Whether the class-data traversal reached its declared end. A false value
    /// means the directory is a locatable prefix; member entries stay usable.
    pub fn field_traversal_complete(&self) -> bool {
        self.fields_issue.is_none()
    }
    pub fn method_traversal_complete(&self) -> bool {
        self.methods_issue.is_none()
    }
    pub fn annotations(&self) -> &OriginalAnnotations {
        &self.annotations
    }
    pub fn annotations_issue(&self) -> Option<&RequestError> {
        self.annotations_issue.as_ref()
    }
    /// Resolve an exact original method selector over the delivered directory.
    /// A partial directory is not treated as proof that the member is absent.
    pub fn resolve_method(
        &self,
        name: &[u16],
        prototype: &[u16],
    ) -> Result<MethodId, RequestError> {
        let mut found = None;
        let mut unresolved = None;
        for entry in &self.methods {
            match entry.declaration() {
                Fact::Present(declaration)
                    if declaration.name_units() == name
                        && declaration.prototype_units() == prototype =>
                {
                    if found.is_some() {
                        return Err(RequestError::Input(crate::input::InputError::Malformed {
                            code: crate::input::InputProblem::InvalidReference,
                            offset_byte: self.header_offset,
                        }));
                    }
                    found = Some(entry.method());
                }
                Fact::Unavailable(RequestError::Input(error)) => {
                    unresolved.get_or_insert(*error);
                }
                Fact::Unavailable(_) => {
                    return Err(RequestError::InternalInvariant(
                        "method declaration failure was not a classified input error",
                    ));
                }
                Fact::Present(_) | Fact::Absent | Fact::NotRead => {}
            }
        }
        if let Some(method) = found {
            return Ok(method);
        }
        Err(match unresolved {
            Some(error) => RequestError::Input(error),
            None => RequestError::Input(crate::input::InputError::Malformed {
                code: crate::input::InputProblem::InvalidReference,
                offset_byte: self.header_offset,
            }),
        })
    }

    /// Whether every attempted declaration and metadata read completed. This is
    /// not a statement about method code or Java representability.
    pub fn is_partial(&self) -> bool {
        self.fields_issue.is_some()
            || self.methods_issue.is_some()
            || self.annotations_issue.is_some()
            || self.class.superclass.unavailable().is_some()
            || self.class.interfaces.unavailable().is_some()
            || match &self.class.interfaces {
                Fact::Present(interfaces) => interfaces
                    .iter()
                    .any(|interface| interface.unavailable().is_some()),
                _ => false,
            }
            || self.class.source_file.unavailable().is_some()
            || self
                .fields
                .iter()
                .any(|field| field.declaration.unavailable().is_some())
            || self
                .methods
                .iter()
                .any(|method| method.declaration.unavailable().is_some())
    }

    /// The original field directory in class-data order.
    /// Field-directory traversal or preparation issue, independent of the
    /// per-entry declaration facts.
    pub fn fields_issue(&self) -> Option<&RequestError> {
        self.fields_issue.as_ref()
    }

    pub fn fields(&self) -> &[FieldDirectoryEntry] {
        &self.fields
    }

    /// The original method directory in class-data order. Each entry retains its
    /// direct/virtual group, ordinal and original code offset.
    pub fn methods(&self) -> &[MethodDirectoryEntry] {
        &self.methods
    }

    /// The original directory entry for one field identity.
    pub fn field_entry(&self, field: FieldId) -> Option<&FieldDirectoryEntry> {
        self.fields.iter().find(|entry| entry.field == field)
    }

    /// The original directory entry for one method identity.
    pub fn method_entry(&self, method: MethodId) -> Option<&MethodDirectoryEntry> {
        self.methods.iter().find(|entry| entry.method == method)
    }
}
