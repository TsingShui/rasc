use crate::p4_types::declarations::annotations::{Bindings, Literal};
use crate::substrate::owned::TextBuffer;
use crate::{ClassKey, RequestError, RunContext};

pub(super) struct VerifiedUses {
    source: String,
    _owner: ClassKey,
    _origins: Vec<UseOrigin>,
}
struct UseOrigin {
    _range: crate::ByteRange,
    _elements: Vec<crate::MethodId>,
}
impl VerifiedUses {
    pub(super) fn source(&self) -> &str {
        &self.source
    }
}
pub(super) fn verify(
    bindings: &Bindings,
    key: ClassKey,
    control: &mut RunContext<'_>,
) -> Result<Option<VerifiedUses>, RequestError> {
    let Some(uses) = bindings.uses() else {
        return Ok(None);
    };
    if bindings.owner() != key {
        return Ok(None);
    }
    let mut source = TextBuffer::new();
    let mut origins = Vec::new();
    for annotation in uses {
        for default in annotation.defaults() {
            control.checkpoint()?;

            if !super::declarations::identifier(default.name(), control)?
                || literal(default.value(), control)?.is_none()
            {
                return Ok(None);
            }
        }
        control.checkpoint()?;

        let descriptor = annotation.descriptor();
        let start = descriptor
            .iter()
            .rposition(|&unit| unit == 47)
            .map_or(1, |at| at + 1);
        let name = &descriptor[start..descriptor.len() - 1];
        if !super::declarations::identifier(name, control)? {
            return Ok(None);
        }
        source.push("@", control)?;
        for scalar in char::decode_utf16(name.iter().copied()) {
            source.character(
                scalar
                    .map_err(|_| RequestError::InternalInvariant("annotation type lost scalar"))?,
                control,
            )?;
        }
        source.push("(", control)?;
        let mut methods = Vec::new();
        for (index, element) in annotation.elements().iter().enumerate() {
            control.checkpoint()?;

            if !super::declarations::identifier(element.name(), control)? {
                return Ok(None);
            }
            for earlier in &annotation.elements()[..index] {
                control.checkpoint()?;
                if earlier.method() == element.method() || earlier.name() == element.name() {
                    return Ok(None);
                }
            }
            crate::substrate::owned::push(&mut methods, element.method(), control)?;
            if index != 0 {
                source.push(", ", control)?;
            }
            for scalar in char::decode_utf16(element.name().iter().copied()) {
                source.character(
                    scalar.map_err(|_| {
                        RequestError::InternalInvariant("annotation element lost scalar")
                    })?,
                    control,
                )?;
            }
            source.push(" = ", control)?;
            let Some(value) = literal(element.value(), control)? else {
                return Ok(None);
            };
            source.push(&value, control)?;
        }
        source.push(")\n", control)?;
        crate::substrate::owned::push(
            &mut origins,
            UseOrigin {
                _range: annotation.origin(),
                _elements: methods,
            },
            control,
        )?;
    }
    Ok(Some(VerifiedUses {
        source: source.finish(),
        _owner: key,
        _origins: origins,
    }))
}
fn literal(
    value: &Literal,
    control: &mut impl crate::pipeline::control::InputControl,
) -> Result<Option<String>, RequestError> {
    match value {
        Literal::String(units) => {
            let mut copy = Vec::new();
            for &unit in units {
                crate::substrate::owned::push(&mut copy, unit, control)?;
            }
            super::preparation::expression(
                crate::p4_types::BuiltinReference::String.descriptor(),
                Some(&crate::p0::ScalarValue::String(
                    crate::p0::OriginalText::new(copy),
                )),
                false,
                control,
            )
        }
        Literal::IntArray(values) => {
            let mut source = TextBuffer::new();
            source.push("{", control)?;
            for (ordinal, value) in values.iter().enumerate() {
                control.checkpoint()?;
                if ordinal != 0 {
                    source.push(", ", control)?;
                }
                source.arguments(format_args!("{value}"), control)?;
            }
            source.push("}", control)?;
            Ok(Some(source.finish()))
        }
    }
}

pub(super) struct VerifiedDefinition {
    elements: Vec<(crate::MethodId, String)>,
}
impl VerifiedDefinition {
    pub(super) fn element_source(&self, method: crate::MethodId) -> Option<&str> {
        self.elements
            .iter()
            .find(|(id, _)| *id == method)
            .map(|(_, source)| source.as_str())
    }
}
pub(super) fn definition(
    bindings: &Bindings,
    key: ClassKey,
    fields: &[crate::FieldOutput],
    members: &[crate::MethodOutput],
    control: &mut RunContext<'_>,
) -> Result<Option<super::class_unit::JavaUnit>, RequestError> {
    let Some(definition) = bindings.definition() else {
        return Ok(None);
    };
    if bindings.owner() != key || !fields.is_empty() || definition.elements().len() != members.len()
    {
        return Ok(None);
    }
    let descriptor = definition.descriptor();

    let Some(name) = super::calls::type_name(descriptor, control)? else {
        return Ok(None);
    };
    let (package, simple) = name
        .rsplit_once('.')
        .map_or((None, name.as_str()), |(package, simple)| {
            (Some(package), simple)
        });
    if simple == "java" {
        return Ok(None);
    }
    let mut header = TextBuffer::new();
    if let Some(package) = package {
        header.push("package ", control)?;
        header.push(package, control)?;
        header.push(";\n", control)?;
    }
    header.push("@java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.RUNTIME)\n@java.lang.annotation.Target({java.lang.annotation.ElementType.TYPE})\npublic @interface ", control)?;
    header.push(simple, control)?;
    header.push(" {\n", control)?;
    let mut elements = Vec::new();
    for (position, element) in definition.elements().iter().enumerate() {
        control.checkpoint()?;
        let crate::MethodOutput::DeclarationOnly { facts, declaration } = &members[position] else {
            return Ok(None);
        };
        if facts.key.class != key
            || facts.key.method != element.method()
            || !crate::substrate::flags::interface_method(facts.access_flags)
            || facts.name != element.name()
            || facts.prototype != element.prototype()
            || declaration.signature().is_some()
            || !super::declarations::identifier(element.name(), control)?
        {
            return Ok(None);
        }

        for earlier in &definition.elements()[..position] {
            control.checkpoint()?;
            if earlier.method() == element.method() || earlier.name() == element.name() {
                return Ok(None);
            }
        }
        let Some(source) = element_source(element, control)? else {
            return Ok(None);
        };
        crate::substrate::owned::push(&mut elements, (element.method(), source), control)?;
    }
    Ok(Some(super::class_unit::JavaUnit::annotation(
        header.finish(),
        VerifiedDefinition { elements },
    )))
}
fn element_source(
    element: &crate::p4_types::declarations::annotations::DeclarationElement,
    budget: &mut crate::RunContext<'_>,
) -> Result<Option<String>, RequestError> {
    let mut source = TextBuffer::new();
    source.push(
        match element.prototype().get(2) {
            Some(76) => "java.lang.String ",
            Some(91) => "int[] ",
            _ => {
                return Err(RequestError::InternalInvariant(
                    "annotation element lost checked return type",
                ));
            }
        },
        budget,
    )?;
    for scalar in char::decode_utf16(element.name().iter().copied()) {
        source.character(
            scalar.map_err(|_| {
                RequestError::InternalInvariant("annotation element name lost scalar")
            })?,
            budget,
        )?;
    }
    source.push("()", budget)?;
    if let Some(default) = element.default() {
        let Some(value) = literal(default, budget)? else {
            return Ok(None);
        };
        source.push(" default ", budget)?;
        source.push(&value, budget)?;
    }
    source.push(";", budget)?;
    Ok(Some(source.finish()))
}
