use crate::context::Context;
use crate::input::ProgramIndex;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push};
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
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &Context,
    facts: &FieldFacts,
    control: &mut impl InputControl,
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
            if !string_scope(index, key, catalog, control)? {
                return Ok(None);
            }
            "java.lang.String"
        }
        _ => return Ok(None),
    };
    Ok(Some(FieldType { spelling }))
}

fn string_scope(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &Context,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    let class = index.class_with(key.class_def, control)?;
    let owner = copy_units(class.descriptor, control)?;
    let context = index.class_context(key.class_def, control)?;
    if context.annotations_offset != crate::ByteOffset(0) || !context.interfaces?.is_empty() {
        return Ok(false);
    }
    let parent = context.superclass?;
    let Some(caller) = catalog.class(&owner, control)? else {
        return Ok(false);
    };
    if !matches!(
        caller.coverage,
        crate::input::DeclarationCoverage::AllDeclarations
    ) {
        return Ok(false);
    }
    match (parent, caller.parent.as_deref()) {
        (Some(parent), Some(expected)) if parent.matches_with(expected, control)? => {}
        (None, None) => {}
        _ => return Ok(false),
    }
    let mut current = caller;
    let mut seen = Vec::new();
    loop {
        control.charge_work(1)?;
        for &old in &seen {
            if crate::context::same_units(old, &current.descriptor, control)? {
                return Ok(false);
            }
        }
        push(&mut seen, current.descriptor.as_slice(), control)?;
        if current.generic || current.enclosing_scope || !current.interfaces.is_empty() {
            return Ok(false);
        }
        let mut start = 1usize;
        for (at, &unit) in current.descriptor.iter().enumerate() {
            control.charge_work(1)?;
            if unit == 47 {
                start = at + 1;
            }
        }
        let Some(end) = current.descriptor.len().checked_sub(1) else {
            return Ok(false);
        };
        if current.descriptor.get(start..end) == Some(&[106, 97, 118, 97]) {
            return Ok(false);
        }
        match &current.parent {
            Some(parent) => {
                let Some(next) = catalog.class(parent, control)? else {
                    return Ok(false);
                };
                current = next;
            }
            None => {
                return Ok(crate::context::same_units(
                    &current.descriptor,
                    crate::substrate::primitive_types::BuiltinReference::Object.descriptor(),
                    control,
                )?);
            }
        }
    }
}
