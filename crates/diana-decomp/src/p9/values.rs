use super::{Origin, RenderError, TextOffset, buffer::Buffer, names::Lookup};
use crate::p8::*;

fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), RenderError> {
    crate::substrate::owned::push(values, value).map_err(|error| match error {
        crate::RequestError::Allocation(error) => RenderError::Allocation(error),
        crate::RequestError::PlatformCapacity => RenderError::Capacity,
        _ => RenderError::Invariant("annotation scratch push failed"),
    })
}

/// One open annotation container while its children print. The work uses an
/// explicit stack so hostile nesting cannot consume the native call stack.
struct Frame {
    remaining: u32,
    first: bool,
    start: TextOffset,
    id: ValueId,
    close: &'static str,
}

/// Display names for the members an annotation record can belong to.
#[derive(Default)]
pub(super) struct Owners<'a> {
    fields: Vec<(crate::FieldId, &'a [u16], crate::p7::NameForm)>,
    methods: Vec<(crate::MethodId, &'a [u16], crate::p7::NameForm)>,
}

impl<'a> Owners<'a> {
    pub fn field(
        &mut self,
        id: crate::FieldId,
        name: &'a [u16],
        form: crate::p7::NameForm,
    ) -> Result<(), RenderError> {
        push(&mut self.fields, (id, name, form))
    }
    pub fn method(
        &mut self,
        id: crate::MethodId,
        name: &'a [u16],
        form: crate::p7::NameForm,
    ) -> Result<(), RenderError> {
        push(&mut self.methods, (id, name, form))
    }
    fn resolve(
        &self,
        location: crate::AnnotationLocation,
    ) -> Option<(&'a [u16], crate::p7::NameForm)> {
        match location {
            crate::AnnotationLocation::Field(id) => self
                .fields
                .iter()
                .find(|(candidate, ..)| *candidate == id)
                .map(|(_, name, form)| (*name, *form)),
            crate::AnnotationLocation::Method(id) => self
                .methods
                .iter()
                .find(|(candidate, ..)| *candidate == id)
                .map(|(_, name, form)| (*name, *form)),
            crate::AnnotationLocation::Parameter { method, .. } => self
                .methods
                .iter()
                .find(|(candidate, ..)| *candidate == method)
                .map(|(_, name, form)| (*name, *form)),
            crate::AnnotationLocation::Class => None,
        }
    }
}

/// Prints one flat annotation value list. Array and nested-annotation headers
/// carry their child count and a closing event, so grouping is mechanical.
pub(super) fn values(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    values: &[ValueExpression],
) -> Result<(), RenderError> {
    let mut frames: Vec<Frame> = Vec::new();
    let mut index = 0usize;
    let mut top_first = true;
    while index < values.len() || !frames.is_empty() {
        while frames.last().is_some_and(|frame| frame.remaining == 0) {
            let Some(frame) = frames.pop() else {
                return Err(RenderError::Invariant(
                    "annotation container stack became empty",
                ));
            };
            match values.get(index).map(|value| &value.kind) {
                Some(ValueKind::EndContainer) => index += 1,
                _ => {
                    return Err(RenderError::Invariant(
                        "annotation container lost its closing event",
                    ));
                }
            }
            out.put(frame.close)?;
            out.span(frame.start, Origin::Value(frame.id))?;
            if let Some(parent) = frames.last_mut() {
                parent.remaining = parent.remaining.saturating_sub(1);
            }
        }
        let Some(value) = values.get(index) else {
            break;
        };
        index += 1;
        match frames.last_mut() {
            Some(frame) => {
                if !frame.first {
                    out.put(", ")?;
                }
                frame.first = false;
            }
            None => {
                if !top_first {
                    out.put(", ")?;
                }
                top_first = false;
            }
        }
        let start = out.position();
        if let Some(name) = &value.name {
            out.name(&name.units, name.form)?;
            out.put(" = ")?;
        }
        let mut opened = None;
        match &value.kind {
            ValueKind::Literal(literal) => literal_value(out, literal)?,
            ValueKind::Reference { syntax, types, .. } => {
                reference_value(out, lookup, syntax, types)?
            }
            ValueKind::Array { count } => {
                out.put("{")?;
                opened = Some(("}", *count));
            }
            ValueKind::Annotation { type_use, count } => {
                out.put("@")?;
                lookup.print(out, *type_use)?;
                out.put("(")?;
                opened = Some((")", *count));
            }
            ValueKind::EndContainer => {
                return Err(RenderError::Invariant(
                    "unexpected annotation container end",
                ));
            }
        }
        match opened {
            Some((close, remaining)) => push(
                &mut frames,
                Frame {
                    remaining,
                    first: true,
                    start,
                    id: value.id,
                    close,
                },
            )?,
            None => {
                if let Some(frame) = frames.last_mut() {
                    frame.remaining = frame.remaining.saturating_sub(1);
                }
                out.span(start, Origin::Value(value.id))?;
            }
        }
    }
    Ok(())
}

fn literal_value(out: &mut Buffer, value: &Literal) -> Result<(), RenderError> {
    match value {
        Literal::Signed(v) => out.fmt(format_args!("{v}"))?,
        Literal::Unsigned(v) => out.fmt(format_args!("{v}u"))?,
        Literal::Bits { width, bits } => {
            out.fmt(format_args!("bits<width={width}; 0x{bits:x}>"))?
        }
        Literal::String(v) => out.units(v, true)?,
        Literal::Null => out.put("null")?,
        Literal::Boolean(v) => out.put(if *v { "true" } else { "false" })?,
    }
    Ok(())
}

fn reference_value(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    syntax: &ReferenceSyntax,
    types: &[TypeUseId],
) -> Result<(), RenderError> {
    match syntax {
        ReferenceSyntax::Type => out.put("class")?,
        ReferenceSyntax::Field(name) | ReferenceSyntax::Method(name) => {
            out.name(&name.units, name.form)?
        }
        ReferenceSyntax::Prototype => out.put("prototype")?,
        ReferenceSyntax::Handle { kind, name } => {
            out.fmt(format_args!("handle kind={kind} "))?;
            out.name(&name.units, name.form)?;
        }
        ReferenceSyntax::Unavailable(error) => out.fmt(format_args!("unavailable: {error}"))?,
    }
    for &ty in types {
        out.put(" ")?;
        lookup.print(out, ty)?;
    }
    Ok(())
}

pub(super) fn annotation(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    plan: &ValuePlan,
    a: &AnnotationExpression,
    owners: Option<&Owners<'_>>,
) -> Result<(), RenderError> {
    out.indent()?;
    let start = out.position();
    scope(out, a.location, owners)?;
    out.put("@")?;
    lookup.print(out, a.type_use)?;
    out.put("(")?;
    if !interpreted_signature(out, lookup, a)? {
        values(
            out,
            lookup,
            plan.expressions()
                .get(a.values.start as usize..a.values.end as usize)
                .ok_or(RenderError::Invariant(
                    "annotation expression range is unavailable",
                ))?,
        )?;
    }
    out.put(") ")?;
    visibility(out, a.visibility)?;
    if a.extent != crate::AnnotationExtent::Complete {
        out.put(" /* prefix */")?;
    }
    out.put(";\n")?;
    out.span(start, Origin::Annotation(a.id))?;
    Ok(())
}

/// An interpreted `@Signature` record prints the declaration it describes
/// instead of its raw descriptor fragments. The delivered annotation keeps its
/// original text in the product; this is the same interpretation the declaration
/// already prints.
fn interpreted_signature(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    a: &AnnotationExpression,
) -> Result<bool, RenderError> {
    let Some(name) = lookup.target_name(a.type_use) else {
        return Ok(false);
    };
    if name != b"dalvik.annotation.Signature".map(u16::from) {
        return Ok(false);
    }
    match a.location {
        crate::AnnotationLocation::Field(field) => {
            let Some(root) = lookup.signature(SignatureOwner::Field(field)) else {
                return Ok(false);
            };
            super::names::signature(out, lookup, root)?;
            Ok(true)
        }
        crate::AnnotationLocation::Parameter { method, parameter } => {
            let owner = SignatureOwner::Parameter {
                method,
                index: parameter.0,
            };
            let Some(root) = lookup.signature(owner) else {
                return Ok(false);
            };
            super::names::signature(out, lookup, root)?;
            Ok(true)
        }
        crate::AnnotationLocation::Method(method) => {
            let Some(returns) = lookup.signature(SignatureOwner::Return(method)) else {
                return Ok(false);
            };
            out.put("(")?;
            for index in 0..MAX_SIGNATURE_PARAMETERS {
                let owner = SignatureOwner::Parameter { method, index };
                let Some(root) = lookup.signature(owner) else {
                    break;
                };
                if index > 0 {
                    out.put(", ")?;
                }
                super::names::signature(out, lookup, root)?;
            }
            out.put(") -> ")?;
            super::names::signature(out, lookup, returns)?;
            Ok(true)
        }
        crate::AnnotationLocation::Class => Ok(false),
    }
}
/// The declaration's own arity bounds the parameter walk; a delivered signature
/// never carries more parameters than the encoded method it describes.
const MAX_SIGNATURE_PARAMETERS: u32 = 255;
/// The record's original owner, named from the delivered declaration when the
/// product carries one and by original index otherwise.
fn scope(
    out: &mut Buffer,
    location: crate::AnnotationLocation,
    owners: Option<&Owners<'_>>,
) -> Result<(), RenderError> {
    let name = owners.and_then(|owners| owners.resolve(location));
    match location {
        crate::AnnotationLocation::Class => out.put("class ")?,
        crate::AnnotationLocation::Field(id) => {
            out.put("field ")?;
            match name {
                Some((units, form)) => out.name(units, form)?,
                None => out.fmt(format_args!("#{}", id.0))?,
            }
            out.put(" ")?;
        }
        crate::AnnotationLocation::Method(id) => {
            out.put("method ")?;
            match name {
                Some((units, form)) => out.name(units, form)?,
                None => out.fmt(format_args!("#{}", id.0))?,
            }
            out.put(" ")?;
        }
        crate::AnnotationLocation::Parameter { method, parameter } => {
            out.put("method ")?;
            match name {
                Some((units, form)) => out.name(units, form)?,
                None => out.fmt(format_args!("#{}", method.0))?,
            }
            out.fmt(format_args!(" parameter {} ", parameter.0))?;
        }
    }
    Ok(())
}

fn visibility(out: &mut Buffer, visibility: u8) -> Result<(), RenderError> {
    match visibility {
        0 => out.put("build")?,
        1 => out.put("runtime")?,
        2 => out.put("system")?,
        other => out.fmt(format_args!("visibility<0x{other:02x}>"))?,
    }
    Ok(())
}

pub(super) fn metadata(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    plan: &ValuePlan,
    owners: Option<&Owners<'_>>,
) -> Result<(), RenderError> {
    if plan.annotations().is_empty() {
        return Ok(());
    }
    out.indent()?;
    out.put("metadata")?;
    out.open()?;
    for a in plan.annotations() {
        annotation(out, lookup, plan, a, owners)?;
    }
    out.close()
}
