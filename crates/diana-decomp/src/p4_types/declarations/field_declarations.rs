use crate::context::Context;
use crate::input::ProgramIndex;
use crate::pipeline::control::InputControl;
use crate::{ClassKey, FieldFacts, RequestError};

pub(crate) struct FieldType {
    spelling: &'static str,
}
impl FieldType {
    pub(crate) fn spelling(&self) -> &'static str {
        self.spelling
    }
}

pub(crate) fn bind(
    _index: &ProgramIndex<'_>,
    _key: ClassKey,
    _catalog: &Context,
    facts: &FieldFacts,
    _control: &mut impl InputControl,
) -> Result<Option<FieldType>, RequestError> {
    let spelling = match facts.descriptor.as_slice() {
        [90] => "boolean",
        [66] => "byte",
        [83] => "short",
        [67] => "char",
        [73] => "int",
        [74] => "long",
        [70] => "float",
        [68] => "double",
        descriptor
            if descriptor
                == crate::substrate::primitive_types::BuiltinReference::String.descriptor() =>
        {
            "java.lang.String"
        }
        _ => return Ok(None),
    };
    Ok(Some(FieldType { spelling }))
}
