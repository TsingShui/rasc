use super::{Origin, RenderError, TextOffset, buffer::Buffer, names::Lookup};
use crate::p8;
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
    fn field(
        &mut self,
        id: crate::FieldId,
        name: &'a [u16],
        form: crate::p7::NameForm,
    ) -> Result<(), RenderError> {
        push(&mut self.fields, (id, name, form))
    }
    fn method(
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

/// Where one delivered annotation record prints. A record whose owner is not a
/// printed declaration keeps the residual block.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Slot {
    /// Above the printed class header.
    Class,
    /// Above that field's declaration.
    Field(crate::FieldId),
    /// Above that member's header.
    Method(crate::MethodId),
    /// Inline before that parameter's type.
    Parameter { method: crate::MethodId, index: u32 },
    /// Owner declaration not printed: the residual block keeps the record.
    Residual,
}

/// The declarations one P9 publication prints, which are the declarations a
/// delivered record can attach to. Every other record keeps the residual block,
/// so no record is dropped and none prints twice.
pub(super) struct Attachments<'a> {
    plan: &'a ValuePlan,
    owners: Owners<'a>,
    /// Ascending identities of the fields that print a declared member.
    fields: Vec<crate::FieldId>,
    /// Ascending identities of the printed members with the parameter list each
    /// one prints.
    methods: Vec<(crate::MethodId, u32)>,
    /// Methods whose complete delivered headers can show interpreted signatures.
    complete_methods: Vec<crate::MethodId>,
    /// Whether this publication prints a class header.
    class_header: bool,
}

/// The header P9 prints for one member. A member whose delivery failed prints
/// its original directory spelling instead, which carries no parameter list.
fn printed_header(member: &p8::MethodMember) -> Option<&crate::p7::MethodHeader> {
    match &member.delivery {
        Ok(p8::MethodDelivery::Expressed(result) | p8::MethodDelivery::Declared(result)) => {
            Some(result.header())
        }
        Ok(p8::MethodDelivery::Stopped(stop)) => {
            stop.expression.as_ref().and_then(|prefix| prefix.header())
        }
        Err(_) => None,
    }
}

impl<'a> Attachments<'a> {
    /// Attachments of a complete class publication.
    pub fn class(class: &'a p8::ClassResult) -> Result<Self, RenderError> {
        let mut attachments = Self::new(class.values(), true);
        for field in class.fields() {
            if let p8::FieldExpression::Declared { name, form, .. } = &field.expression {
                attachments.owners.field(field.id, name, *form)?;
                push(&mut attachments.fields, field.id)?;
            }
        }
        for member in class.methods() {
            attachments.member(class.facts(), member, Some(&class.header().name))?;
        }
        attachments.finish()
    }
    /// Attachments of a standalone member publication.
    pub fn standalone(display: &'a p8::MethodDisplay) -> Result<Self, RenderError> {
        let mut attachments = Self::new(display.values(), false);
        attachments.member(display.facts(), display.member(), None)?;
        attachments.finish()
    }
    fn new(plan: &'a ValuePlan, class_header: bool) -> Self {
        Self {
            plan,
            owners: Owners::default(),
            fields: Vec::new(),
            methods: Vec::new(),
            complete_methods: Vec::new(),
            class_header,
        }
    }
    fn finish(mut self) -> Result<Self, RenderError> {
        self.fields.sort_unstable();
        self.methods.sort_unstable_by_key(|(id, _)| *id);
        self.complete_methods.sort_unstable();
        Ok(self)
    }
    fn member(
        &mut self,
        facts: &crate::p0::ClassFacts,
        member: &'a p8::MethodMember,
        class_name: Option<&'a p8::ValueName>,
    ) -> Result<(), RenderError> {
        let printed = printed_header(member);
        if let Ok(p8::MethodDelivery::Expressed(result) | p8::MethodDelivery::Declared(result)) =
            &member.delivery
            && !matches!(
                result.header().kind,
                crate::p7::MethodKind::ClassInitializer
            )
        {
            push(&mut self.complete_methods, member.id)?;
        }
        // A residual prefix keeps today's owner spelling, which names only a
        // member whose delivered result carries its declaration.
        if let Ok(delivery) = &member.delivery
            && let Some(result) = delivery.result()
        {
            let header = result.header();
            // A constructor record names the type it constructs, just like its
            // reference in a body: `<init>` is not an identifier.
            match class_name {
                Some(name) if super::names::initializer(header.declaration.name_units()) => {
                    self.owners.method(member.id, &name.units, name.form)?;
                }
                _ => self.owners.method(
                    member.id,
                    header.declaration.name_units(),
                    header.name_form,
                )?,
            }
        }
        // A record attaches to a member that prints a declaration spelling, from
        // its header or from its original directory entry. A member that prints
        // only an unavailable-declaration gap keeps its records in the residual
        // block, and only a printed header has a parameter list to attach to.
        let declared = printed.is_some()
            || facts
                .method_entry(member.id)
                .is_some_and(|entry| entry.declaration().is_present());
        if !declared {
            return Ok(());
        }
        let parameters = printed.map_or(0, |header| header.declaration.parameter_count());
        push(
            &mut self.methods,
            (
                member.id,
                u32::try_from(parameters).map_err(|_| RenderError::Capacity)?,
            ),
        )
    }
    fn slot(&self, location: crate::AnnotationLocation) -> Slot {
        match location {
            crate::AnnotationLocation::Class if self.class_header => Slot::Class,
            crate::AnnotationLocation::Field(id) if self.fields.binary_search(&id).is_ok() => {
                Slot::Field(id)
            }
            crate::AnnotationLocation::Method(id) if self.parameters(id).is_some() => {
                Slot::Method(id)
            }
            crate::AnnotationLocation::Parameter { method, parameter }
                if self
                    .parameters(method)
                    .is_some_and(|count| parameter.0 < count) =>
            {
                Slot::Parameter {
                    method,
                    index: parameter.0,
                }
            }
            _ => Slot::Residual,
        }
    }
    fn parameters(&self, method: crate::MethodId) -> Option<u32> {
        self.methods
            .binary_search_by_key(&method, |(id, _)| *id)
            .ok()
            .map(|index| self.methods[index].1)
    }
    fn declarations(
        &self,
        out: &mut Buffer,
        lookup: &Lookup<'_>,
        slot: Slot,
    ) -> Result<(), RenderError> {
        for a in self.plan.annotations() {
            if self.slot(a.location) != slot {
                continue;
            }
            if let Slot::Method(method) = slot
                && self.complete_methods.binary_search(&method).is_ok()
                && lookup
                    .method_signature_record(method)
                    .is_some_and(|(record, complete)| record == a.id && complete)
            {
                out.span(out.position(), Origin::Annotation(a.id))?;
                continue;
            }
            out.indent()?;
            record(out, lookup, self.plan, a)?;
            out.put("\n")?;
        }
        Ok(())
    }
    /// Restore a signature record when its planned method header could not be
    /// printed. The record was not emitted by `declarations` in that case.
    pub fn print_unrepresented_method_signature(
        &self,
        out: &mut Buffer,
        lookup: &Lookup<'_>,
        method: crate::MethodId,
    ) -> Result<(), RenderError> {
        if self.complete_methods.binary_search(&method).is_err() {
            return Ok(());
        }
        let Some((record_id, true)) = lookup.method_signature_record(method) else {
            return Ok(());
        };
        if let Some(annotation) = self
            .plan
            .annotations()
            .iter()
            .find(|a| a.id == record_id && self.slot(a.location) == Slot::Method(method))
        {
            out.indent()?;
            record(out, lookup, self.plan, annotation)?;
            out.put("\n")?;
        }
        Ok(())
    }
    /// Whether the residual block prints inside a member rendering. A standalone
    /// publication has no class header, so the member owns that block's position.
    pub fn prints_residual_here(&self) -> bool {
        !self.class_header
    }
    /// Prints the records attaching above the class declaration.
    pub fn print_class(&self, out: &mut Buffer, lookup: &Lookup<'_>) -> Result<(), RenderError> {
        self.declarations(out, lookup, Slot::Class)
    }
    /// Prints the records attaching above one field declaration.
    pub fn print_field(
        &self,
        out: &mut Buffer,
        lookup: &Lookup<'_>,
        field: crate::FieldId,
    ) -> Result<(), RenderError> {
        self.declarations(out, lookup, Slot::Field(field))
    }
    /// Prints the records attaching above one member's header.
    pub fn print_method(
        &self,
        out: &mut Buffer,
        lookup: &Lookup<'_>,
        method: crate::MethodId,
    ) -> Result<(), RenderError> {
        self.declarations(out, lookup, Slot::Method(method))
    }
    /// Prints the records attaching inline before one parameter's type.
    pub fn print_parameter(
        &self,
        out: &mut Buffer,
        lookup: &Lookup<'_>,
        method: crate::MethodId,
        index: u32,
    ) -> Result<(), RenderError> {
        let slot = Slot::Parameter { method, index };
        for a in self.plan.annotations() {
            if self.slot(a.location) != slot {
                continue;
            }
            record(out, lookup, self.plan, a)?;
            out.put(" ")?;
        }
        Ok(())
    }
    /// Prints the records whose owner declaration is not printed, each still
    /// prefixed by its original owner.
    pub fn print_residual(&self, out: &mut Buffer, lookup: &Lookup<'_>) -> Result<(), RenderError> {
        let mut residual = self
            .plan
            .annotations()
            .iter()
            .filter(|a| self.slot(a.location) == Slot::Residual)
            .peekable();
        if residual.peek().is_none() {
            return Ok(());
        }
        out.indent()?;
        out.put("metadata")?;
        out.open()?;
        for a in residual {
            out.indent()?;
            let start = out.position();
            scope(out, a.location, Some(&self.owners))?;
            record_text(out, lookup, self.plan, a)?;
            out.put(";\n")?;
            out.span(start, Origin::Annotation(a.id))?;
        }
        out.close()
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

/// One record's own delivered text: its type, arguments and visibility word.
fn record_text(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    plan: &ValuePlan,
    a: &AnnotationExpression,
) -> Result<(), RenderError> {
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
    Ok(())
}

/// The same text as one attached record, with the record's own span.
fn record(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    plan: &ValuePlan,
    a: &AnnotationExpression,
) -> Result<(), RenderError> {
    let start = out.position();
    record_text(out, lookup, plan, a)?;
    out.span(start, Origin::Annotation(a.id))
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
            if lookup
                .method_signature_record(method)
                .is_none_or(|(record, complete)| record != a.id || !complete)
            {
                return Ok(false);
            }
            let Some(returns) = lookup.signature(SignatureOwner::Return(method)) else {
                return Ok(false);
            };
            if let Some(parameters) = lookup.method_parameters(method)
                && !parameters.is_empty()
            {
                super::names::type_parameters(out, lookup, parameters)?;
                out.put(" ")?;
            }
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
        crate::AnnotationLocation::Class => {
            let Some(parameters) = lookup.class_parameters() else {
                return Ok(false);
            };
            super::names::type_parameters(out, lookup, parameters)?;
            if !parameters.is_empty() {
                out.put(" ")?;
            }
            let superclass =
                lookup
                    .signature(SignatureOwner::Superclass)
                    .ok_or(RenderError::Invariant(
                        "interpreted class signature has no superclass",
                    ))?;
            out.put("extends ")?;
            super::names::signature(out, lookup, superclass)?;
            let mut index = 0;
            while let Some(root) = lookup.signature(SignatureOwner::Interface(index)) {
                out.put(if index == 0 { " implements " } else { ", " })?;
                super::names::signature(out, lookup, root)?;
                index = index.checked_add(1).ok_or(RenderError::Invariant(
                    "interface index exceeds the delivered coordinate",
                ))?;
            }
            Ok(true)
        }
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
