use crate::p0::FieldDirectoryEntry;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::TextBuffer;
use crate::{FieldFacts, FieldStorage, RequestError};

/// An owned readable field declaration and its original static preparation.
pub struct JavaField {
    source: String,
    pub(crate) default_preparation: bool,
    preparation: Option<String>,
    pub(super) initial_expression: Option<String>,
}
impl JavaField {
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Original non-default static preparation, emitted before initializer bodies.
    /// Unavailable preparation remains classified in the field result.
    pub(crate) fn preparation(&self) -> Option<&str> {
        self.preparation.as_deref()
    }
}
pub(crate) fn reconstruct(
    _schedule: &crate::engine::SchedulePermit,
    field: &FieldDirectoryEntry,
    facts: &FieldFacts,
    ty: &crate::p4_types::declarations::field_declarations::FieldType,
    control: &mut impl InputControl,
) -> Result<Option<JavaField>, RequestError> {
    reconstruct_impl(field, facts, ty, control)
}

fn reconstruct_impl(
    field: &FieldDirectoryEntry,
    facts: &FieldFacts,
    ty: &crate::p4_types::declarations::field_declarations::FieldType,
    control: &mut impl InputControl,
) -> Result<Option<JavaField>, RequestError> {
    let flags = field.access_flags_raw();
    if !crate::substrate::flags::field_valid(flags)
        || (flags & crate::substrate::flags::STATIC != 0)
            != matches!(field.storage(), FieldStorage::Static(_))
    {
        return Ok(None);
    }
    if !super::declarations::identifier(&facts.name, control)? {
        return Ok(None);
    }

    let mut source = TextBuffer::new();
    for (bit, spelling) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (16, "final "),
        (64, "volatile "),
        (128, "transient "),
    ] {
        if flags & bit != 0 {
            source.push(spelling, control)?;
        }
    }
    source.push(ty.spelling(), control)?;
    source.push(" ", control)?;
    for scalar in char::decode_utf16(facts.name.iter().copied()) {
        control.checkpoint()?;
        source.character(
            scalar
                .map_err(|_| RequestError::InternalInvariant("verified field name lost scalar"))?,
            control,
        )?;
    }
    source.push(";", control)?;
    let initial_expression = if matches!(field.storage(), FieldStorage::Static(_)) {
        super::preparation::expression(
            &facts.descriptor,
            field.scalar(),
            field.default_preparation(),
            control,
        )?
    } else {
        None
    };
    let preparation = if !field.default_preparation()
        && matches!(field.storage(), FieldStorage::Static(_))
        && initial_expression.is_some()
    {
        let mut preparation = TextBuffer::new();
        preparation.push("static { ", control)?;
        for scalar in char::decode_utf16(facts.name.iter().copied()) {
            control.checkpoint()?;
            preparation.character(
                scalar.map_err(|_| {
                    RequestError::InternalInvariant("verified preparation name lost scalar")
                })?,
                control,
            )?;
        }
        preparation.push(" = ", control)?;
        preparation.push(
            initial_expression
                .as_deref()
                .ok_or(RequestError::InternalInvariant(
                    "preparation expression disappeared",
                ))?,
            control,
        )?;
        preparation.push("; }", control)?;
        Some(preparation.finish())
    } else {
        None
    };
    Ok(Some(JavaField {
        source: source.finish(),
        default_preparation: matches!(field.storage(), FieldStorage::Instance)
            || field.default_preparation(),
        preparation,
        initial_expression,
    }))
}
