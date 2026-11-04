use super::ProgramIndex;
use super::values::{EncodedKind, ValueStep};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push, reserve};
use crate::{ByteOffset, ByteRange, ClassKey, RequestError, StringId, TypeId};

/// Coverage of the checked metadata traversal, not annotation type or Java validity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationExtent {
    Complete,
    Prefix,
}
#[derive(Debug)]
pub enum AnnotationValueKind {
    Signed(i64),
    Unsigned(u64),
    Bits {
        width: crate::ByteLength,
        bits: u64,
    },
    String {
        reference: StringId,
        units: Vec<u16>,
    },
    Reference(crate::raw::OperandReference),
    Null,
    Boolean(bool),
    Array {
        count: u32,
    },
    Annotation {
        type_id: TypeId,
        count: u32,
    },
}
/// One decoded value header and scalar/reference payload. Its physical byte range
/// excludes the element name index and, for containers, their child encodings.
#[derive(Debug)]
pub struct AnnotationValue {
    name: Option<Vec<u16>>,
    name_id: Option<StringId>,
    range: ByteRange,
    tag: u8,
    arg: u8,
    kind: AnnotationValueKind,
}
impl AnnotationValue {
    pub fn name(&self) -> Option<&[u16]> {
        self.name.as_deref()
    }
    pub fn name_id(&self) -> Option<StringId> {
        self.name_id
    }
    pub fn range(&self) -> ByteRange {
        self.range
    }
    pub fn tag(&self) -> u8 {
        self.tag
    }
    pub fn value_arg(&self) -> u8 {
        self.arg
    }
    pub fn kind(&self) -> &AnnotationValueKind {
        &self.kind
    }
}
/// Flat value events. Arrays and nested annotations end with `EndContainer`;
/// the outer annotation record itself has no closing event.
#[derive(Debug)]
pub enum AnnotationToken {
    Value(AnnotationValue),
    EndContainer,
}
/// DEX prototype parameter position, excluding an implicit receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnnotationParameterIndex(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationLocation {
    Class,
    Field(crate::FieldId),
    Method(crate::MethodId),
    Parameter {
        method: crate::MethodId,
        parameter: AnnotationParameterIndex,
    },
}

/// Original annotation bytes and parsed events at a checked owner location.
/// A prefix can include the offending byte after its last decoded event.
#[derive(Debug)]
pub struct AnnotationRecord {
    location: AnnotationLocation,
    visibility: u8,
    type_id: TypeId,
    descriptor: Vec<u16>,
    range: ByteRange,
    bytes: Vec<u8>,
    tokens: Vec<AnnotationToken>,
    extent: AnnotationExtent,
}
impl AnnotationRecord {
    pub fn location(&self) -> AnnotationLocation {
        self.location
    }
    pub fn visibility(&self) -> u8 {
        self.visibility
    }
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
    pub fn descriptor(&self) -> &[u16] {
        &self.descriptor
    }
    pub fn range(&self) -> ByteRange {
        self.range
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn tokens(&self) -> &[AnnotationToken] {
        &self.tokens
    }
    pub fn extent(&self) -> AnnotationExtent {
        self.extent
    }
}
/// Checked pointer/association bytes locating metadata whose traversal failed.
#[derive(Debug)]
pub struct AnnotationPrefix {
    range: ByteRange,
    bytes: Vec<u8>,
}
impl AnnotationPrefix {
    pub fn range(&self) -> ByteRange {
        self.range
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
/// Owned original metadata in class/field/method/parameter traversal order.
/// Ranges address the original physical input, including any container prefix.
/// `Prefix` retains only the traversed portion; a report diagnostic explains the
/// missing suffix. Complete coverage does not certify ART validity or Java source.
#[derive(Debug)]
pub struct OriginalAnnotations {
    owner: ClassKey,
    directory: ByteOffset,
    records: Vec<AnnotationRecord>,
    prefix: Option<AnnotationPrefix>,
    extent: AnnotationExtent,
}
impl OriginalAnnotations {
    pub fn owner(&self) -> ClassKey {
        self.owner
    }
    pub fn directory(&self) -> ByteOffset {
        self.directory
    }
    pub fn records(&self) -> &[AnnotationRecord] {
        &self.records
    }
    pub fn unavailable_prefix(&self) -> Option<&AnnotationPrefix> {
        self.prefix.as_ref()
    }
    pub fn extent(&self) -> AnnotationExtent {
        self.extent
    }
}
pub(crate) struct ReadAnnotations {
    pub(crate) facts: OriginalAnnotations,
    pub(crate) issue: Option<RequestError>,
}
fn bytes(
    index: &ProgramIndex<'_>,
    range: ByteRange,
    control: &mut impl InputControl,
) -> Result<Vec<u8>, RequestError> {
    let source =
        index
            .bytes
            .get(range.start.0..range.end.0)
            .ok_or(RequestError::InternalInvariant(
                "annotation reader lost checked range",
            ))?;
    let mut result = Vec::new();
    reserve(&mut result, source.len(), control)?;
    for &byte in source {
        control.checkpoint()?;
        result.push(byte);
    }
    Ok(result)
}
fn hard(error: &RequestError) -> bool {
    matches!(
        error.kind(),
        crate::FailureKind::Cancelled
            | crate::FailureKind::ResourceExhausted
            | crate::FailureKind::InternalInvariant
    )
}
fn read_set(
    index: &ProgramIndex<'_>,
    offset: ByteOffset,
    location: AnnotationLocation,
    facts: &mut OriginalAnnotations,
    control: &mut impl InputControl,
) -> Result<(), RequestError> {
    if offset.0 == 0 {
        return Ok(());
    }
    let mut offsets = index.annotation_offsets(0x1003, offset, control)?;
    let mut last = None;
    while offsets.remaining_byte().0 != 0 {
        control.checkpoint()?;
        let pointer = ByteRange {
            start: offsets.position_byte(),
            end: ByteOffset(
                offsets
                    .position_byte()
                    .0
                    .checked_add(4)
                    .ok_or(RequestError::PlatformCapacity)?,
            ),
        };
        facts.prefix = Some(AnnotationPrefix {
            range: pointer,
            bytes: bytes(index, pointer, control)?,
        });
        let (offset, visibility, type_id, mut values) =
            index.next_annotation(&mut offsets, &mut last, control)?;
        let mut tokens = Vec::new();
        let body = (|| -> Result<(), RequestError> {
            while let Some(step) = values.next(index, control)? {
                let token = match step {
                    ValueStep::EndContainer => AnnotationToken::EndContainer,
                    ValueStep::Value(value) => {
                        let name = match value.name {
                            Some(id) => Some(copy_units(index.string_with(id, control)?, control)?),
                            None => None,
                        };
                        let kind = match value.kind {
                            EncodedKind::Signed(value) => AnnotationValueKind::Signed(value),
                            EncodedKind::Unsigned(value) => AnnotationValueKind::Unsigned(value),
                            EncodedKind::Bits { width, bits } => {
                                AnnotationValueKind::Bits { width, bits }
                            }
                            EncodedKind::Reference(crate::raw::OperandReference::String(
                                reference,
                            )) => AnnotationValueKind::String {
                                reference,
                                units: copy_units(index.string_with(reference, control)?, control)?,
                            },
                            EncodedKind::Reference(reference) => {
                                AnnotationValueKind::Reference(reference)
                            }
                            EncodedKind::Null => AnnotationValueKind::Null,
                            EncodedKind::Boolean(value) => AnnotationValueKind::Boolean(value),
                            EncodedKind::Array { count } => AnnotationValueKind::Array { count },
                            EncodedKind::Annotation { type_id, count } => {
                                AnnotationValueKind::Annotation { type_id, count }
                            }
                        };
                        AnnotationToken::Value(AnnotationValue {
                            name,
                            name_id: value.name,
                            range: value.range,
                            tag: value.tag,
                            arg: value.arg,
                            kind,
                        })
                    }
                };
                push(&mut tokens, token, control)?;
            }
            Ok(())
        })();
        if let Err(error) = &body
            && hard(error)
        {
            return Err(body.err().ok_or(RequestError::InternalInvariant(
                "annotation body lost failure",
            ))?);
        }
        let range = ByteRange {
            start: offset,
            end: values.position(),
        };
        let record = AnnotationRecord {
            location,
            visibility,
            type_id,
            descriptor: copy_units(index.type_name(type_id, control)?, control)?,
            range,
            bytes: bytes(index, range, control)?,
            tokens,
            extent: if body.is_ok() {
                AnnotationExtent::Complete
            } else {
                AnnotationExtent::Prefix
            },
        };
        push(&mut facts.records, record, control)?;
        body?;
        facts.prefix = None;
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum AssociationKind {
    Field,
    Method,
    Parameter,
}
fn read_members(
    index: &ProgramIndex<'_>,
    class: &super::ClassView<'_, '_>,
    records: &mut super::Cursor<'_>,
    kind: AssociationKind,
    facts: &mut OriginalAnnotations,
    control: &mut impl InputControl,
) -> Result<(), RequestError> {
    use super::annotations::{AnnotationMembers, AnnotationOwner, association};
    if records.remaining_byte().0 == 0 {
        return Ok(());
    }
    let method = !matches!(kind, AssociationKind::Field);
    let mut members = AnnotationMembers::open(index, class, method, control)?;
    let mut last = None;
    while records.remaining_byte().0 != 0 {
        control.checkpoint()?;
        let range = ByteRange {
            start: records.position_byte(),
            end: ByteOffset(
                records
                    .position_byte()
                    .0
                    .checked_add(8)
                    .ok_or(RequestError::PlatformCapacity)?,
            ),
        };
        facts.prefix = Some(AnnotationPrefix {
            range,
            bytes: bytes(index, range, control)?,
        });
        let (owner, offset, parameters) = association(
            index,
            class,
            &mut members,
            records,
            &mut last,
            method,
            control,
        )?;
        match (matches!(kind, AssociationKind::Parameter), owner) {
            (false, AnnotationOwner::Field(id)) => {
                read_set(index, offset, AnnotationLocation::Field(id), facts, control)?
            }
            (false, AnnotationOwner::Method(id)) => read_set(
                index,
                offset,
                AnnotationLocation::Method(id),
                facts,
                control,
            )?,
            (true, AnnotationOwner::Method(id)) => {
                let mut references = index.annotation_parameters(offset, parameters, control)?;
                for parameter in 0..parameters {
                    let at = references.position_byte();
                    let offset = super::file_offset(references.read_u32_with(control)?, at)?;
                    read_set(
                        index,
                        offset,
                        AnnotationLocation::Parameter {
                            method: id,
                            parameter: AnnotationParameterIndex(
                                u32::try_from(parameter)
                                    .map_err(|_| RequestError::PlatformCapacity)?,
                            ),
                        },
                        facts,
                        control,
                    )?;
                }
            }
            (true, AnnotationOwner::Field(_)) => {
                return Err(RequestError::InternalInvariant(
                    "parameter annotation mapped to a field",
                ));
            }
        }
        facts.prefix = None;
    }
    Ok(())
}

pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    owner: ClassKey,
    control: &mut impl InputControl,
) -> Result<ReadAnnotations, RequestError> {
    let context = index.class_context(owner.class_def, control)?;
    let mut result = ReadAnnotations {
        facts: OriginalAnnotations {
            owner,
            directory: context.annotations_offset,
            records: Vec::new(),
            prefix: None,
            extent: AnnotationExtent::Complete,
        },
        issue: None,
    };
    let outcome = (|| -> Result<(), RequestError> {
        if context.annotations_offset.0 == 0 {
            return Ok(());
        }
        let mut directory = index.annotation_directory(context.annotations_offset, control)?;
        read_set(
            index,
            directory.class,
            AnnotationLocation::Class,
            &mut result.facts,
            control,
        )?;
        let class = index.class_with(owner.class_def, control)?;
        read_members(
            index,
            &class,
            &mut directory.fields,
            AssociationKind::Field,
            &mut result.facts,
            control,
        )?;
        read_members(
            index,
            &class,
            &mut directory.methods,
            AssociationKind::Method,
            &mut result.facts,
            control,
        )?;
        read_members(
            index,
            &class,
            &mut directory.parameters,
            AssociationKind::Parameter,
            &mut result.facts,
            control,
        )?;
        Ok(())
    })();
    if let Err(error) = outcome {
        if hard(&error) {
            return Err(error);
        }
        result.facts.extent = AnnotationExtent::Prefix;
        result.issue = Some(error);
    }
    Ok(result)
}
