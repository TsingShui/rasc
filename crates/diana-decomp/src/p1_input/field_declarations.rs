use super::ProgramIndex;
use super::catalog::{FieldStorage as RawStorage, MemberEntry, MemberSelection};
use super::values::{EncodedKind, EncodedValues, ValueStep};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push};
use crate::{
    ClassKey, FieldFacts, FieldInitialValue, FieldKey, FieldStorage, RequestError,
    StaticFieldOrdinal,
};

pub(crate) enum PreparationValue {
    Signed(i64),
    Unsigned(u64),
    Float(u32),
    Double(u64),
    Boolean(bool),
    Null,
    String(Vec<u16>),
}

pub(crate) struct FieldInput {
    _sealed: (),
    pub key: FieldKey,
    pub access_flags: u32,
    pub storage: FieldStorage,
    /// Position inside the original static or instance group.
    pub ordinal: u32,
    pub facts: Result<FieldFacts, RequestError>,
    pub initial: Option<FieldInitialValue>,
    pub default_preparation: bool,
    pub value: Option<PreparationValue>,
}

pub(crate) struct FieldDirectory {
    pub fields: Vec<FieldInput>,
    pub issue: Option<RequestError>,
}

pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    control: &mut impl InputControl,
) -> Result<FieldDirectory, RequestError> {
    let class = index.class_with(key.class_def, control)?;
    let mut fields = Vec::new();
    let mut static_count = 0u32;
    let visit: Result<(), RequestError> =
        class.visit_members_with(control, MemberSelection::Fields, |entry, control| {
            let MemberEntry::Field(field) = entry else {
                return Err(RequestError::InternalInvariant(
                    "field directory visited a method",
                ));
            };
            let storage = match field.storage {
                RawStorage::Instance => FieldStorage::Instance,
                RawStorage::Static { ordinal } => {
                    static_count = static_count
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                    FieldStorage::Static(StaticFieldOrdinal(ordinal))
                }
            };
            let facts = match field.declaration {
                Ok(declaration) => Ok(FieldFacts {
                    name: copy_units(declaration.name, control)?,
                    descriptor: copy_units(declaration.field_type, control)?,
                }),
                Err(error) => Err(error.into()),
            };
            let initial = match storage {
                FieldStorage::Instance => Some(FieldInitialValue::InstanceDefault),
                FieldStorage::Static(_) => None,
            };
            push(
                &mut fields,
                FieldInput {
                    _sealed: (),
                    key: FieldKey {
                        class: key,
                        field: field.field_idx,
                    },
                    access_flags: field.access_flags,
                    storage,
                    ordinal: field.ordinal,
                    facts,
                    initial,
                    default_preparation: false,
                    value: None,
                },
                control,
            )?;
            Ok(())
        });
    if let Err(error) = visit {
        return local_failure(fields, error);
    }
    let values = (|| -> Result<(), RequestError> {
        let context = index.class_context(key.class_def, control)?;
        let Some(offset) = context.values_offset? else {
            for field in &mut fields {
                control.checkpoint()?;
                if matches!(field.storage, FieldStorage::Static(_)) {
                    field.initial = Some(FieldInitialValue::ImplicitDefault);
                    field.default_preparation = true;
                }
            }
            return Ok(());
        };
        let mut values = EncodedValues::open(index, offset, Some(static_count), control)?;
        for field in &mut fields {
            control.checkpoint()?;
            let FieldStorage::Static(ordinal) = field.storage else {
                continue;
            };
            if ordinal.0 >= values.count {
                field.initial = Some(FieldInitialValue::ImplicitDefault);
                field.default_preparation = true;
                continue;
            }
            let Some(ValueStep::Value(value)) = values.next(index, control)? else {
                return Err(RequestError::InternalInvariant(
                    "encoded preparation lost root",
                ));
            };
            if value.ordinal.0 != ordinal.0 {
                return Err(RequestError::InternalInvariant(
                    "encoded preparation changed static ordinal",
                ));
            }
            let mut depth = if matches!(
                value.kind,
                EncodedKind::Array { .. } | EncodedKind::Annotation { .. }
            ) {
                1usize
            } else {
                0
            };
            let mut bytes = Vec::new();
            for &byte in value.bytes {
                control.checkpoint()?;
                push(&mut bytes, byte, control)?;
            }
            while depth != 0 {
                control.checkpoint()?;
                match values.next(index, control)? {
                    Some(ValueStep::EndContainer) => depth -= 1,
                    Some(ValueStep::Value(child)) => {
                        // Copy the contiguous original range below; annotation-name
                        // indices occur before child value headers.
                        if matches!(
                            child.kind,
                            EncodedKind::Array { .. } | EncodedKind::Annotation { .. }
                        ) {
                            depth = depth.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                        }
                    }
                    None => {
                        return Err(RequestError::InternalInvariant(
                            "encoded preparation container ended early",
                        ));
                    }
                }
            }
            let range = crate::ByteRange {
                start: value.range.start,
                end: values.position(),
            };
            if range != value.range {
                for &byte in index.bytes.get(value.range.end.0..range.end.0).ok_or(
                    RequestError::InternalInvariant("encoded preparation lost original bytes"),
                )? {
                    control.checkpoint()?;
                    push(&mut bytes, byte, control)?;
                }
            }
            field.default_preparation = match field.facts.as_ref() {
                Ok(facts) => default_value(&facts.descriptor, value.tag, &value.kind),
                Err(_) => false,
            };
            field.value = match &value.kind {
                EncodedKind::Signed(value) => Some(PreparationValue::Signed(*value)),
                EncodedKind::Unsigned(value) => Some(PreparationValue::Unsigned(*value)),
                EncodedKind::Bits { bits, .. } if value.tag == 0x10 => {
                    Some(PreparationValue::Float(u32::try_from(*bits).map_err(
                        |_| RequestError::InternalInvariant("float preparation lost width"),
                    )?))
                }
                EncodedKind::Bits { bits, .. } if value.tag == 0x11 => {
                    Some(PreparationValue::Double(*bits))
                }
                EncodedKind::Boolean(value) => Some(PreparationValue::Boolean(*value)),
                EncodedKind::Null => Some(PreparationValue::Null),
                EncodedKind::Reference(crate::raw::OperandReference::String(id)) => {
                    Some(PreparationValue::String(copy_units(
                        index.string_with(*id, control)?,
                        control,
                    )?))
                }
                _ => None,
            };
            field.initial = Some(FieldInitialValue::Encoded {
                tag: value.tag,
                arg: value.arg,
                range,
                bytes,
            });
        }
        if values.next(index, control)?.is_some() {
            return Err(RequestError::InternalInvariant(
                "encoded preparation has extra root",
            ));
        }
        Ok(())
    })();
    if let Err(error) = values {
        return local_failure(fields, error);
    }
    Ok(FieldDirectory {
        fields,
        issue: None,
    })
}

fn local_failure(
    fields: Vec<FieldInput>,
    error: RequestError,
) -> Result<FieldDirectory, RequestError> {
    if matches!(
        error.kind(),
        crate::FailureKind::Cancelled
            | crate::FailureKind::ResourceExhausted
            | crate::FailureKind::InternalInvariant
    ) {
        return Err(error);
    }
    Ok(FieldDirectory {
        fields,
        issue: Some(error),
    })
}

fn default_value(descriptor: &[u16], tag: u8, value: &EncodedKind) -> bool {
    match (descriptor, tag, value) {
        ([90], 0x1f, EncodedKind::Boolean(false)) => true,
        ([66], 0, EncodedKind::Signed(0))
        | ([83], 2, EncodedKind::Signed(0))
        | ([73], 4, EncodedKind::Signed(0))
        | ([74], 6, EncodedKind::Signed(0)) => true,
        ([67], 3, EncodedKind::Unsigned(0)) => true,
        ([70], 0x10, EncodedKind::Bits { bits: 0, .. })
        | ([68], 0x11, EncodedKind::Bits { bits: 0, .. }) => true,
        (_, 0x1e, EncodedKind::Null) => matches!(descriptor.first(), Some(76 | 91)),
        _ => false,
    }
}
