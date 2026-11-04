use super::ProgramIndex;
use crate::substrate::owned::push;
use crate::{
    ClassKey, FieldFacts, FieldInitialValue, FieldKey, FieldStorage, RequestError, RunContext,
};

pub(crate) struct FieldInput {
    _sealed: (),
    pub key: FieldKey,
    pub access_flags: u32,
    pub storage: FieldStorage,
    pub facts: Result<FieldFacts, RequestError>,
    pub initial: Option<FieldInitialValue>,
    pub default_preparation: bool,
    pub value: Option<crate::p0::ScalarValue>,
}

pub(crate) struct FieldDirectory {
    pub fields: Vec<FieldInput>,
    pub issue: Option<RequestError>,
}

pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    control: &mut RunContext<'_>,
) -> Result<FieldDirectory, RequestError> {
    // P0 is the single class-data producer. This module maps its field facts to
    // the declaration/preparation view used by Java reconstruction.
    let facts = index.class_facts_cached(key.class_def, control)?;
    let mut fields = Vec::new();
    for entry in facts.fields() {
        control.checkpoint()?;
        let declaration = match entry.declaration() {
            crate::p0::Fact::Present(declaration) => Ok(FieldFacts {
                name: crate::substrate::owned::copy_slice_units(declaration.name.units(), control)?,
                descriptor: crate::substrate::owned::copy_slice_units(
                    declaration.descriptor.units(),
                    control,
                )?,
            }),
            crate::p0::Fact::Unavailable(crate::RequestError::Input(error)) => {
                Err(RequestError::Input(*error))
            }
            crate::p0::Fact::Unavailable(_) => Err(RequestError::InternalInvariant(
                "P0 field declaration failure was not a classified input error",
            )),
            crate::p0::Fact::Absent | crate::p0::Fact::NotRead => Err(
                RequestError::InternalInvariant("P0 field directory delivered no declaration"),
            ),
        };
        let initial = match entry.initial_value() {
            crate::p0::Fact::Present(crate::p0::InitialValue::ImplicitDefault) => {
                Some(FieldInitialValue::ImplicitDefault)
            }
            crate::p0::Fact::Present(crate::p0::InitialValue::Encoded {
                tag,
                value_arg,
                range_bytes,
                bytes,
                ..
            }) => Some(FieldInitialValue::Encoded {
                tag: *tag,
                arg: *value_arg,
                range: crate::ByteRange {
                    start: crate::ByteOffset(range_bytes[0]),
                    end: crate::ByteOffset(range_bytes[1]),
                },
                bytes: bytes.clone(),
            }),
            crate::p0::Fact::Absent => Some(FieldInitialValue::InstanceDefault),
            crate::p0::Fact::NotRead | crate::p0::Fact::Unavailable(_) => None,
        };
        let value = entry.scalar().cloned();
        let default_preparation = entry.default_preparation();
        push(
            &mut fields,
            FieldInput {
                _sealed: (),
                key: FieldKey {
                    class: key,
                    field: entry.field(),
                },
                access_flags: entry.original_flags().raw(),
                storage: entry.storage(),
                facts: declaration,
                initial,
                default_preparation,
                value,
            },
            control,
        )?;
    }
    Ok(FieldDirectory {
        fields,
        issue: match facts.fields_issue() {
            Some(RequestError::Input(error)) => Some(RequestError::Input(*error)),
            Some(_) => Some(RequestError::InternalInvariant(
                "P0 field issue was not a classified input error",
            )),
            None => None,
        },
    })
}
