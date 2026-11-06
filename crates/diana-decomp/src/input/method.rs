use super::values::EncodedValues;
use super::{
    Cursor, InputError, InputProblem, MethodBody, MethodError, ProgramIndex, RawMethodView,
    file_offset, malformed,
};

use crate::{ByteLength, ByteOffset, ByteRange, ClassDefId, MethodId, RequestError};

/// Original annotation associations checked for one selected member. Other
/// members' annotation bodies are independent and are not sealed by this value.
#[derive(Clone, Copy, Default)]
pub struct CheckedAnnotations {
    method: Option<ByteRange>,
    parameters: Option<ByteRange>,
}

impl CheckedAnnotations {
    pub fn method_range(&self) -> Option<ByteRange> {
        self.method
    }
    pub fn parameter_range(&self) -> Option<ByteRange> {
        self.parameters
    }
}

/// A reader product: the original code item view and its independently checked
/// metadata. P1 adds the decoded instruction directory and structural layout.
pub enum RawPrintable<'a> {
    Method(RawMethodView<'a>),
    Declaration,
}

/// The input seam for one original declaration in a selected logical program.
/// Construction is restricted to the owning input validator.
pub struct MethodSource<'i, 'a> {
    index: &'i ProgramIndex<'a>,
    method_id: MethodId,
    owner: crate::p0::OwnerContext<'a>,
    access_flags: u32,
    declaration: crate::p0::MethodDeclaration,
    raw: RawPrintable<'a>,
    /// The independent method/parameter annotation read for this member. A
    /// damaged set retains its own classified error and does not remove code.
    metadata: Result<CheckedAnnotations, std::sync::Arc<RequestError>>,
    original_metadata: crate::p0::MethodMetadata,
}

impl<'i, 'a> MethodSource<'i, 'a> {
    /// The original logical index this source borrows.
    pub(crate) fn program(&self) -> &'i ProgramIndex<'a> {
        self.index
    }

    pub fn class_id(&self) -> ClassDefId {
        self.owner.current().key().class_def
    }
    pub fn method_id(&self) -> MethodId {
        self.method_id
    }
    pub fn header_offset(&self) -> ByteOffset {
        self.owner.current().header_offset()
    }
    pub fn owner_facts(&self) -> &crate::p0::OwnerFacts {
        self.owner.current()
    }
    pub(crate) fn owner_context(&self) -> &crate::p0::OwnerContext<'a> {
        &self.owner
    }
    pub fn access_flags(&self) -> u32 {
        self.access_flags
    }
    pub fn declaration(&self) -> &crate::p0::MethodDeclaration {
        &self.declaration
    }
    pub fn raw(&self) -> &RawPrintable<'a> {
        &self.raw
    }
    /// Checked annotation locations, or defaults when that optional read
    /// failed. Code and declaration facts remain valid.
    pub fn annotations(&self) -> CheckedAnnotations {
        match &self.metadata {
            Ok(annotations) => *annotations,
            Err(_) => CheckedAnnotations::default(),
        }
    }
    /// The independent annotation read. `Err` reports a damaged annotation set;
    /// the code and declaration facts of this source stay valid.
    pub fn metadata(&self) -> Result<&CheckedAnnotations, &RequestError> {
        self.metadata.as_ref().map_err(AsRef::as_ref)
    }
    pub fn original_metadata(&self) -> &crate::p0::MethodMetadata {
        &self.original_metadata
    }
    pub(crate) fn metadata_error(&self) -> Option<std::sync::Arc<RequestError>> {
        self.metadata.as_ref().err().cloned()
    }
    pub fn into_raw(self) -> RawPrintable<'a> {
        self.raw
    }
}

impl<'a> ProgramIndex<'a> {
    /// Builds the method-source product for one original entry delivered by the
    /// class-fact producer. The entry supplies the identity, declaration result,
    /// encoded group and original code offset, so this never re-walks class-data.
    /// The re-read declaration record is checked against that entry; a disagreement
    /// is an internal invariant, not a second source of member facts.
    pub fn method_source<'i>(
        &'i self,
        facts: &crate::p0::ClassFacts,
        method_id: MethodId,
    ) -> Result<MethodSource<'i, 'a>, MethodError> {
        if facts.owner().dex != self.dex_id()
            || facts.header_offset() != self.header.header_offset
            || self.class_type_id(facts.class_def())? != facts.type_index()
        {
            return Err(MethodError::InternalInvariant(
                "class facts and class view disagree on the original type",
            ));
        }
        let entry = facts.method_entry(method_id).ok_or_else(|| {
            MethodError::Input(malformed(
                InputProblem::InvalidReference,
                self.header.method_ids.offset_byte,
            ))
        })?;
        let access_flags = entry.access_flags().raw();
        // The declaration is P0's product. The reader binds the entry identity
        // and opens only the original code item; it never resolves the
        // declaration record again.
        let declaration = match entry.declaration() {
            crate::p0::Fact::Present(declaration) => {
                declaration.try_copy().map_err(method_error)?
            }
            crate::p0::Fact::Unavailable(crate::RequestError::Input(error)) => {
                return Err(MethodError::Input(*error));
            }
            crate::p0::Fact::Unavailable(_) => {
                return Err(MethodError::InternalInvariant(
                    "P0 declaration failure was not a classified input error",
                ));
            }
            crate::p0::Fact::Absent | crate::p0::Fact::NotRead => {
                return Err(MethodError::Input(malformed(
                    InputProblem::InvalidClassData,
                    self.header.class_defs.offset_byte,
                )));
            }
        };
        let body = super::code::read_code(
            self,
            entry.code_offset(),
            method_id,
            access_flags,
            Some(crate::RegisterWordCount(
                declaration.parameter_words
                    + u64::from(access_flags & crate::substrate::flags::STATIC == 0),
            )),
        )?;
        self.finish_method_source(facts, method_id, access_flags, body, declaration)
    }

    /// Locates one original method by walking its class-data directory and then
    /// builds the same method-source product. Declaration-query consumers that
    /// hold no class facts use this; it does not create a second analysis route.
    pub fn method_source_at<'i>(
        &'i self,
        class_id: ClassDefId,
        method_id: MethodId,
    ) -> Result<MethodSource<'i, 'a>, MethodError> {
        // Declaration-query consumers reuse the same P0 class production as
        // request scheduling; they do not perform a second class-data walk.
        let facts = self.class_facts_cached(class_id).map_err(method_error)?;
        self.method_source(&facts, method_id)
    }

    fn finish_method_source<'i>(
        &'i self,
        facts: &crate::p0::ClassFacts,
        method_id: MethodId,
        access_flags: u32,
        body: MethodBody<'a>,
        declaration: crate::p0::MethodDeclaration,
    ) -> Result<MethodSource<'i, 'a>, MethodError> {
        let class_id = facts.class_def();
        // Optional metadata is read independently: a damaged debug program or
        // annotation set keeps its own state and never removes intact code.
        let metadata = self
            .check_method_metadata(class_id, method_id, declaration.parameter_count())
            .map_err(|e| std::sync::Arc::new(RequestError::from(e)));
        // The reader establishes only safe original ranges and independent
        // metadata. P1 owns instruction decoding, structural checks and
        // reference-content association.
        let raw = match body {
            MethodBody::DeclarationOnly => RawPrintable::Declaration,
            MethodBody::Code(code) => RawPrintable::Method(code),
        };

        Ok(MethodSource {
            index: self,
            method_id,
            owner: crate::p0::OwnerContext::new(facts.owner_facts(), self.declarations.clone()),
            access_flags,
            declaration,
            raw,
            metadata,
            original_metadata: facts.method_metadata(method_id),
        })
    }

    fn check_method_metadata(
        &self,
        class_id: ClassDefId,
        method_id: MethodId,
        parameters: usize,
    ) -> Result<CheckedAnnotations, MethodError> {
        let offset = self.class_annotations_offset(class_id)?;
        let mut checked = CheckedAnnotations::default();
        if offset == ByteOffset(0) {
            return Ok(checked);
        }
        let directory = self.annotation_directory(offset)?;
        if let Some(offset) = selected_offset(directory.methods, method_id)? {
            checked.method = Some(self.check_annotation_set(offset)?);
        }
        if let Some(offset) = selected_offset(directory.parameters, method_id)? {
            let mut list = self.annotation_parameters(offset, parameters)?;
            while list.remaining_byte() != ByteLength(0) {
                let at = list.position_byte();
                let set = file_offset(list.read_u32()?, at)?;
                if set != ByteOffset(0) {
                    self.check_annotation_set(set)?;
                }
            }
            checked.parameters = Some(ByteRange {
                start: offset,
                end: list.position_byte(),
            });
        }
        Ok(checked)
    }

    fn check_annotation_set(&self, offset: ByteOffset) -> Result<ByteRange, MethodError> {
        let mut offsets = self.annotation_offsets(0x1003, offset)?;
        let mut last = None;
        while offsets.remaining_byte() != ByteLength(0) {
            let (_, _, _, mut values) = self.next_annotation(&mut offsets, &mut last)?;
            consume_values(self, &mut values)?;
        }
        Ok(ByteRange {
            start: offset,
            end: offsets.position_byte(),
        })
    }
}

fn consume_values(
    index: &ProgramIndex<'_>,
    values: &mut EncodedValues<'_>,
) -> Result<(), MethodError> {
    while values.next(index)?.is_some() {}
    Ok(())
}

fn selected_offset(
    mut records: Cursor<'_>,
    method_id: MethodId,
) -> Result<Option<ByteOffset>, InputError> {
    let mut previous = None;
    let mut selected = None;
    while records.remaining_byte() != ByteLength(0) {
        let at = records.position_byte();
        let id = MethodId(records.read_u32()?);
        let offset = file_offset(records.read_u32()?, at)?;
        if previous.is_some_and(|previous| previous >= id) {
            return Err(malformed(InputProblem::InvalidReference, at));
        }
        previous = Some(id);
        if id == method_id {
            selected = Some(offset);
        }
    }
    Ok(selected)
}

fn method_error(error: RequestError) -> MethodError {
    match error {
        RequestError::Input(error) => MethodError::Input(error),
        RequestError::Method(error) => error,

        RequestError::Allocation(error) => MethodError::Allocation(error),
        RequestError::PlatformCapacity => MethodError::PlatformCapacity,
        RequestError::InternalInvariant(message) => MethodError::InternalInvariant(message),
        _ => MethodError::InternalInvariant("method reader returned a foreign error"),
    }
}
