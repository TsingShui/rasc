use crate::input::field_declarations::FieldInput;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::TextBuffer;
use crate::{FieldFacts, FieldStorage, RequestError};

/// A source field declaration checked by P8. Unit admission separately requires
/// initialization and whole-class binding evidence; this is not a JavaUnit.
pub struct VerifiedJavaField {
    source: String,
    pub(crate) default_preparation: bool,
}
impl VerifiedJavaField {
    pub fn source(&self) -> &str {
        &self.source
    }
}
pub(crate) fn verify(
    _schedule: &crate::engine::SchedulePermit,
    field: &FieldInput,
    facts: &FieldFacts,
    ty: &crate::p4_calls::field_declarations::FieldType,
    control: &mut impl InputControl,
) -> Result<Option<VerifiedJavaField>, RequestError> {
    verify_impl(field, facts, ty, control)
}

fn verify_impl(
    field: &FieldInput,
    facts: &FieldFacts,
    ty: &crate::p4_calls::field_declarations::FieldType,
    control: &mut impl InputControl,
) -> Result<Option<VerifiedJavaField>, RequestError> {
    let flags = field.access_flags;
    if flags & !(1 | 2 | 4 | 8 | 16 | 64 | 128) != 0
        || (flags & 7).count_ones() > 1
        || flags & (16 | 64) == 80
        || (flags & 8 != 0) != matches!(field.storage, FieldStorage::Static(_))
    {
        return Ok(None);
    }
    if flags & 16 != 0 {
        return Err(RequestError::FinalAssignmentUnproved { field: field.key });
    }
    if !super::declarations::identifier(&facts.name, control)? {
        return Ok(None);
    }
    super::declarations::check_constant_utf8(&facts.name, control)?;
    super::declarations::check_constant_utf8(&facts.descriptor, control)?;
    let mut source = TextBuffer::new();
    for (bit, spelling) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (64, "volatile "),
        (128, "transient "),
    ] {
        if flags & bit != 0 {
            source.push(spelling, control)?;
        }
    }
    source.push(ty.spelling(), control)?;
    source.push(" ", control)?;
    for &unit in &facts.name {
        control.charge_work(1)?;
        source.character(
            char::from_u32(u32::from(unit)).ok_or(RequestError::InternalInvariant(
                "verified field name lost scalar",
            ))?,
            control,
        )?;
    }
    source.push(";", control)?;
    Ok(Some(VerifiedJavaField {
        source: source.finish(),
        default_preparation: matches!(field.storage, FieldStorage::Instance)
            || field.default_preparation,
    }))
}
