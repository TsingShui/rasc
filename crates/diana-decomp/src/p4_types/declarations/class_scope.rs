use crate::context::Context;
use crate::input::ProgramIndex;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push};
use crate::{ClassDefId, RequestError};

/// Original class header facts. This product does not certify source inheritance,
/// access, overrides or completeness of declarations outside the input.
pub(crate) struct ClassBindings {
    _sealed: (),
    pub owner: Vec<u16>,
    pub parent: Option<Vec<u16>>,
    pub interfaces: Vec<Vec<u16>>,
    pub string_specialization: bool,
}

pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    id: ClassDefId,
    _catalog: &Context,
    control: &mut impl InputControl,
) -> Result<Option<ClassBindings>, RequestError> {
    let class = index.class_with(id, control)?;
    let owner = copy_units(class.descriptor, control)?;
    let context = index.class_context(id, control)?;
    let parent = context
        .superclass?
        .map(|name| copy_units(name, control))
        .transpose()?;
    let bytes = context.interfaces?;
    let mut cursor =
        crate::input::Cursor::bounded(bytes, crate::ByteOffset(0), crate::ByteOffset(bytes.len()))?;
    let mut interfaces = Vec::new();
    while cursor.remaining_byte().0 != 0 {
        control.checkpoint()?;
        let ty = crate::TypeId(u32::from(cursor.read_u16_with(control)?));
        let name = copy_units(index.type_name(ty, control)?, control)?;
        push(&mut interfaces, name, control)?;
    }
    // Preserve the already-supported specialization only when the original
    // Signature spells it exactly. Unknown generic information stays erased.
    let mut string_specialization = false;
    if interfaces.len() == 1
        && parent.as_deref() == Some(crate::p4_types::BuiltinReference::Object.descriptor())
        && let Some(signature) = crate::input::signature::class(index, id, control)?
    {
        let mut expected = Vec::new();
        for &unit in crate::p4_types::BuiltinReference::Object.descriptor() {
            push(&mut expected, unit, control)?;
        }
        let name = &interfaces[0];
        if let Some(prefix) = name.strip_suffix(&[u16::from(b';')]) {
            for &unit in prefix {
                push(&mut expected, unit, control)?;
            }
            for &unit in &const { crate::substrate::owned::ascii_units(*b"<Ljava/lang/String;>;") }
            {
                push(&mut expected, unit, control)?;
            }
            string_specialization =
                crate::context::same_units(signature.units(), &expected, control)?;
        }
    }
    Ok(Some(ClassBindings {
        _sealed: (),
        owner,
        parent,
        interfaces,
        string_specialization,
    }))
}
