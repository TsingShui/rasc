use super::{
    Origin, RenderError, SourceProduct,
    buffer::Buffer,
    expressions::Expressions,
    names::{Lookup, MethodNames},
};
use crate::{MethodId, p0::Fact, p7, p8};

pub(super) fn render(out: &mut Buffer, source: &SourceProduct) -> Result<(), RenderError> {
    match source {
        SourceProduct::Class(class) => class_result(out, class),
        SourceProduct::Method(method) => {
            let lookup = Lookup::complete(method.names())?;
            member(
                out,
                &lookup,
                method.facts(),
                Some(method.values()),
                method.member(),
                None,
            )
        }
        SourceProduct::ClassStopped(stop) | SourceProduct::MethodStopped(stop) => {
            let lookup = Lookup::new(stop.partial_name_uses(), stop.partial_name_targets())?;
            out.fmt(format_args!("assembly_unavailable<{:?}>;\n", stop.error()))?;
            if matches!(source, SourceProduct::ClassStopped(_)) {
                out.put("class original_type<")?;
                out.units(stop.facts().descriptor().units(), true)?;
                out.put(">")?;
                out.open()?;
                for f in stop.facts().fields() {
                    out.indent()?;
                    let a = out.position();
                    out.fmt(format_args!("field#{} {:?} ", f.field().0, f.storage()))?;
                    match f.declaration() {
                        Fact::Present(d) => {
                            out.units(d.descriptor.units(), true)?;
                            out.put(" ")?;
                            out.units(d.name.units(), true)?;
                        }
                        Fact::Absent => out.put("absent_declaration")?,
                        Fact::NotRead => out.put("declaration_not_read")?,
                        Fact::Unavailable(e) => {
                            out.fmt(format_args!("unavailable_declaration<{e}>"))?
                        }
                    }
                    out.put(";\n")?;
                    out.span(a, Origin::Field(f.field()))?;
                }
            }
            super::raw::assembly(out, stop, matches!(source, SourceProduct::ClassStopped(_)))?;
            for m in stop.methods() {
                member(out, &lookup, stop.facts(), None, m, None)?;
            }
            if matches!(source, SourceProduct::ClassStopped(_)) {
                out.close()?;
            }
            Ok(())
        }
    }
}
fn class_result(out: &mut Buffer, class: &p8::ClassResult) -> Result<(), RenderError> {
    let lookup = Lookup::complete(class.names())?;
    if let Some(package) = class.names().package()
        && !package.units.is_empty()
    {
        out.put("package ")?;
        out.name(&package.units, package.form)?;
        out.put(";\n\n")?;
    }
    for import in class.names().imports() {
        let target = &lookup.targets[import.target.0 as usize];
        out.put("import ")?;
        out.units(&target.full, false)?;
        out.put(";\n")?;
    }
    super::values::metadata(out, &lookup, class.values())?;
    let start = out.position();
    let header = class.header();
    super::names::flags(
        out,
        header.flags,
        super::names::FlagScope::Class {
            interface: matches!(
                header.kind,
                p8::ClassKind::Interface | p8::ClassKind::Annotation
            ),
        },
    )?;
    out.put(match header.kind {
        p8::ClassKind::Class => "class ",
        p8::ClassKind::Interface => "interface ",
        p8::ClassKind::Annotation => "@interface ",
        p8::ClassKind::Enum => "enum ",
    })?;
    out.name(&header.name.units, header.name.form)?;
    // An interface, annotation or enum has no superclass clause in Java; the
    // delivered Object/Enum entry is implicit there. A missing or unavailable
    // superclass fact is still printed so the gap stays visible.
    let implicit = matches!(
        header.kind,
        p8::ClassKind::Interface | p8::ClassKind::Annotation | p8::ClassKind::Enum
    ) && matches!(header.superclass, p8::TypeExpression::Present(_));
    if !implicit && !matches!(header.superclass, p8::TypeExpression::Absent) {
        out.put(" extends ")?;
        type_expression(out, &lookup, &header.superclass)?;
    }
    match &header.interfaces {
        p8::InterfaceList::Present(list) if !list.is_empty() => {
            out.put(" implements ")?;
            for (i, interface) in list.iter().enumerate() {
                if i > 0 {
                    out.put(", ")?;
                }
                type_expression(out, &lookup, &interface.name)?;
            }
        }
        p8::InterfaceList::NotRead => out.put(" interfaces_not_read")?,
        p8::InterfaceList::Unavailable => out.put(" interfaces_unavailable")?,
        _ => {}
    }
    out.open()?;
    for f in class.fields() {
        out.indent()?;
        let a = out.position();
        super::names::flags(out, f.flags, super::names::FlagScope::Field)?;
        match &f.expression {
            p8::FieldExpression::Declared {
                name,
                form,
                field_type,
            } => {
                match lookup.signature(p8::SignatureOwner::Field(f.id)) {
                    Some(root) => super::names::signature(out, &lookup, root)?,
                    None => lookup.print(out, *field_type)?,
                }
                out.put(" ")?;
                out.name(name, *form)?;
            }
            p8::FieldExpression::Absent => {
                out.fmt(format_args!("field#{} absent_declaration", f.id.0))?
            }
            p8::FieldExpression::NotRead => {
                out.fmt(format_args!("field#{} declaration_not_read", f.id.0))?
            }
            p8::FieldExpression::Unavailable => {
                out.fmt(format_args!("field#{} unavailable_declaration", f.id.0))?;
                if let Some(original) = class.facts().field_entry(f.id)
                    && let Fact::Unavailable(error) = original.declaration()
                {
                    out.fmt(format_args!("<{error}>"))?;
                }
            }
        }
        out.put(";\n")?;
        out.span(a, Origin::Field(f.id))?;
    }
    if class.preparation_has_values() {
        out.indent()?;
        out.put("static_preparation_before_clinit")?;
        out.open()?;
        for &field in class.preparation_order() {
            let f = &class.fields()[field];
            out.indent()?;
            let a = out.position();
            // The declaration name identifies the field; an unavailable
            // declaration keeps the original index instead of a guessed name.
            match &f.expression {
                p8::FieldExpression::Declared { name, form, .. } => out.name(name, *form)?,
                _ => out.fmt(format_args!("field#{}", f.id.0))?,
            }
            out.put(" = ")?;
            match f.preparation {
                p8::Preparation::ImplicitDefault => out.put("implicit_default")?,
                p8::Preparation::Encoded | p8::Preparation::EncodedPrefix => {
                    let range = class
                        .values()
                        .preparations()
                        .iter()
                        .find(|(id, _)| *id == f.id)
                        .map(|(_, range)| range.clone());
                    match range {
                        Some(range) => super::values::values(
                            out,
                            &lookup,
                            &class.values().expressions()[range.start as usize..range.end as usize],
                        )?,
                        None => out.put("encoded_value_unavailable")?,
                    }
                    if matches!(f.preparation, p8::Preparation::EncodedPrefix) {
                        out.put(" /* encoded prefix */")?;
                    }
                }
                p8::Preparation::Unavailable => {
                    out.put("unavailable<")?;
                    match class
                        .facts()
                        .field_entry(f.id)
                        .map(|entry| entry.initial_value())
                    {
                        Some(Fact::Unavailable(error)) => out.fmt(format_args!("{error}"))?,
                        _ => out.put("preparation state unknown")?,
                    }
                    out.put(">")?;
                }
                p8::Preparation::Absent => out.put("encoded_value_absent")?,
                p8::Preparation::NotRead => out.put("preparation_not_read")?,
            }
            out.put(";\n")?;
            out.span(a, Origin::Preparation(f.id))?;
        }
        out.close()?;
    }
    for m in class.methods() {
        member(
            out,
            &lookup,
            class.facts(),
            None,
            m,
            Some(&class.header().name),
        )?;
    }
    for (kind, error) in [
        ("fields", class.facts().fields_issue()),
        ("methods", class.facts().methods_issue()),
        ("annotations", class.facts().annotations_issue()),
    ] {
        if let Some(error) = error {
            out.indent()?;
            out.fmt(format_args!("unavailable_{kind}<{error}>;\n"))?;
        }
    }
    if let Some(offset) = class.facts().coverage_terminated_at() {
        out.indent()?;
        out.fmt(format_args!(
            "directory_suffix_not_located<file_byte={offset}>;\n"
        ))?;
    }
    out.close()?;
    out.span(start, Origin::Class)
}
fn type_expression(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    ty: &p8::TypeExpression,
) -> Result<(), RenderError> {
    match ty {
        p8::TypeExpression::Present(id) => lookup.print(out, *id),
        p8::TypeExpression::Absent => out.put("absent_type"),
        p8::TypeExpression::NotRead => out.put("type_not_read"),
        p8::TypeExpression::Unavailable => out.put("unavailable_type"),
    }
}
fn member(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    facts: &crate::p0::ClassFacts,
    values: Option<&p8::ValuePlan>,
    member: &p8::MethodMember,
    class_name: Option<&p8::ValueName>,
) -> Result<(), RenderError> {
    let initial_depth = out.depth;
    let start = out.position();
    let result = method(out, lookup, facts, values, member, class_name);
    match result {
        Ok(()) => {}
        Err(RenderError::Invariant(message)) => {
            out.problem(member.id, super::LocalRenderError::Invariant(message))?;
            while out.depth > initial_depth {
                out.close()?;
            }
        }
        Err(RenderError::MethodScratch(error)) => {
            if out.depth == initial_depth {
                original_header(out, facts, member.id)?;
                out.put(";\n")?;
            }
            out.problem(member.id, super::LocalRenderError::Scratch(error))?;
            while out.depth > initial_depth {
                out.close()?;
            }
        }
        Err(e) => return Err(e),
    }
    out.span(start, Origin::Method(member.id))
}
fn method(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    facts: &crate::p0::ClassFacts,
    values: Option<&p8::ValuePlan>,
    member: &p8::MethodMember,
    class_name: Option<&p8::ValueName>,
) -> Result<(), RenderError> {
    // MethodDisplay already carries only this member's metadata; class metadata
    // is printed once from its supplied record scopes before member declarations.
    if let Some(values) = values {
        super::values::metadata(out, lookup, values)?;
    }
    match &member.delivery {
        Ok(p8::MethodDelivery::Expressed(result) | p8::MethodDelivery::Declared(result)) => {
            let header = result.header();
            if let Some(error) = &header.metadata_error {
                out.indent()?;
                out.fmt(format_args!("method_metadata_unavailable<{error}>;\n"))?;
            }
            let names = MethodNames::collect(
                member.id,
                result.code().map_or(0, |c| c.types().len()),
                header.declaration.parameter_count(),
                lookup,
            )?;
            header_line(
                out,
                lookup,
                &names,
                header,
                class_name,
                lookup.find_use(p8::TypeSource::Owner),
            )?;
            if let Some(code) = result.code() {
                out.open()?;
                let mut pending = Vec::new();
                pending
                    .try_reserve_exact(code.emission().controls().len())
                    .map_err(RenderError::MethodScratch)?;
                pending.resize(code.emission().controls().len(), None);
                let mut context = Expressions {
                    out,
                    method: member.id,
                    code,
                    names: &names,
                    lookup,
                    pending_controls: Some(&mut pending),
                };
                context.body()?;
                out.close()?;
            } else if matches!(header.kind, p7::MethodKind::ClassInitializer) {
                out.open()?;
                out.close()?;
            } else {
                out.put("; // no code item\n")?;
            }
        }
        Ok(p8::MethodDelivery::Stopped(stop)) => {
            if let Some(header) = stop.expression.as_ref().and_then(|p| p.header()) {
                let names = MethodNames::collect(
                    member.id,
                    stop.expression.as_ref().map_or(0, |p| p.type_uses().len()),
                    header.declaration.parameter_count(),
                    lookup,
                )?;
                header_line(out, lookup, &names, header, None, None)?;
            } else {
                original_header(out, facts, member.id)?;
            }
            out.open()?;
            out.indent()?;
            out.fmt(format_args!(
                "unrecovered_control<stage={:?}; kind={:?}; ",
                stop.stage, stop.cause_kind
            ))?;
            match &stop.cause {
                Ok(c) => out.quote(&c.detail)?,
                Err(e) => out.fmt(format_args!("diagnostic_unavailable: {e}"))?,
            }
            out.put(">;\n")?;
            if let Some(prefix) = &stop.expression {
                let names = MethodNames::collect(
                    member.id,
                    prefix.type_uses().len(),
                    prefix
                        .header()
                        .map_or(0, |h| h.declaration.parameter_count()),
                    lookup,
                )?;
                let mut context = Expressions {
                    out,
                    method: member.id,
                    code: prefix.as_ref(),
                    names: &names,
                    lookup,
                    pending_controls: None,
                };
                for local in prefix.local_declarations() {
                    context.out.indent()?;
                    context.out.put("storage_fact ")?;
                    super::expressions::machine_type(context.out, local.original.storage_type())?;
                    context.out.put(" ")?;
                    context.out.put(&local.name)?;
                    context
                        .out
                        .put("; // original storage, display not published\n")?;
                }
                for bundle in prefix.copies() {
                    context.out.indent()?;
                    context.out.fmt(format_args!(
                        "copy_fact #{} at {:?}: {{",
                        bundle.id.0, bundle.location
                    ))?;
                    context.copy(bundle.id)?;
                    context
                        .out
                        .put("} // source obligation, not a reconstructed schedule\n")?;
                }
                for (op, statement) in prefix.fragments() {
                    context.out.indent()?;
                    let a = context.out.position();
                    context.out.fmt(format_args!("fragment @{} {{ ", op.pc.0))?;
                    context.assignment(statement)?;
                    context.semantic(statement)?;
                    context.out.put("; } // control not published\n")?;
                    context.out.span(
                        a,
                        Origin::Operation {
                            method: member.id,
                            operation: op.source,
                            pc: op.pc,
                        },
                    )?;
                }
            }
            match &stop.decoded {
                Ok(items) => {
                    for item in items {
                        out.indent()?;
                        let a = out.position();
                        out.fmt(format_args!("decoded @{}: {item:?};\n", item.pc().0))?;
                        out.span(
                            a,
                            Origin::Raw {
                                method: member.id,
                                pc: Some(item.pc()),
                            },
                        )?;
                    }
                }
                Err(e) => {
                    out.indent()?;
                    out.fmt(format_args!("decoded_prefix_unavailable<{e}>;\n"))?;
                }
            }
            match &stop.raw {
                Ok(Some(raw)) => {
                    out.indent()?;
                    let a = out.position();
                    out.fmt(format_args!(
                        "raw_code<file_bytes={}..{}; registers={}; incoming={}; outgoing={}> [",
                        raw.range.start.0,
                        raw.range.end.0,
                        raw.registers,
                        raw.incoming,
                        raw.outgoing
                    ))?;
                    for b in &raw.bytes {
                        out.fmt(format_args!("{b:02x} "))?;
                    }
                    out.put("];\n")?;
                    out.span(
                        a,
                        Origin::Raw {
                            method: member.id,
                            pc: None,
                        },
                    )?;
                }
                Ok(None) => out.line("raw_code_not_available;")?,
                Err(e) => {
                    out.indent()?;
                    out.fmt(format_args!("raw_code_retention_failed<{e}>;\n"))?;
                }
            }
            out.close()?;
        }
        Err(error) => {
            original_header(out, facts, member.id)?;
            out.open()?;
            out.indent()?;
            out.fmt(format_args!("member_delivery_unavailable<{error}>;\n"))?;
            out.close()?;
        }
    }
    Ok(())
}
fn header_line(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    names: &MethodNames,
    h: &p7::MethodHeader,
    class_name: Option<&p8::ValueName>,
    owner: Option<p8::TypeUseId>,
) -> Result<(), RenderError> {
    out.indent()?;
    // A class initializer is a static block. DEX may give it access bits that
    // Java cannot express there; they stay in the original flags of the product.
    if matches!(h.kind, p7::MethodKind::ClassInitializer) {
        out.put("static")?;
        return Ok(());
    }
    super::names::flags(
        out,
        h.access_flags,
        super::names::FlagScope::Method {
            initializer: matches!(h.kind, p7::MethodKind::Constructor),
        },
    )?;
    let method = h.key.method;
    match h.kind {
        p7::MethodKind::ClassInitializer => unreachable!("handled above"),
        // A constructor carries the owner name and no return type.
        p7::MethodKind::Constructor => {
            // The class declaration name is the constructor name. A reference
            // spelling can be qualified by an unrelated field or local shadow.
            match (class_name, owner) {
                (Some(name), _) => out.name(&name.units, name.form)?,
                (None, Some(id)) => lookup.print(out, id)?,
                (None, None) => out.name(h.declaration.name_units(), h.name_form)?,
            }
            out.put("(")?;
            parameters(out, lookup, names, h)?;
            out.put(")")?;
            return Ok(());
        }
        p7::MethodKind::Ordinary => {}
    }
    match lookup.signature(p8::SignatureOwner::Return(method)) {
        Some(root) => super::names::signature(out, lookup, root)?,
        None => match names.result {
            Some(ty) => lookup.print(out, ty)?,
            None => {
                out.put("type<")?;
                out.units(h.declaration.return_type.units(), true)?;
                out.put(">")?;
            }
        },
    }
    out.put(" ")?;
    out.name(h.declaration.name_units(), h.name_form)?;
    out.put("(")?;
    parameters(out, lookup, names, h)?;
    out.put(")")
}

fn parameters(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    names: &MethodNames,
    h: &p7::MethodHeader,
) -> Result<(), RenderError> {
    let method = h.key.method;
    for (i, ty) in h.declaration.parameter_types.iter().enumerate() {
        if i > 0 {
            out.put(", ")?;
        }
        let index = u32::try_from(i).map_err(|_| RenderError::Capacity)?;
        if let Some(root) = lookup.signature(p8::SignatureOwner::Parameter { method, index }) {
            super::names::signature(out, lookup, root)?;
            out.put(" ")?;
            out.put(&h.parameter_names[i])?;
            continue;
        }
        if let Some(id) = names.parameters[i] {
            lookup.print(out, id)?;
        } else {
            out.put("type<")?;
            out.units(ty.units(), true)?;
            out.put(">")?;
        }
        out.put(" ")?;
        out.put(&h.parameter_names[i])?;
    }
    Ok(())
}

fn original_header(
    out: &mut Buffer,
    facts: &crate::p0::ClassFacts,
    id: MethodId,
) -> Result<(), RenderError> {
    out.indent()?;
    out.fmt(format_args!("method#{} ", id.0))?;
    if let Some(entry) = facts.method_entry(id) {
        match entry.declaration() {
            Fact::Present(d) => {
                out.units(d.name_units(), true)?;
                out.put(" ")?;
                out.units(d.prototype_units(), true)?;
            }
            Fact::Unavailable(e) => out.fmt(format_args!("unavailable_declaration<{e}>"))?,
            Fact::NotRead => out.put("declaration_not_read")?,
            Fact::Absent => out.put("absent_declaration")?,
        }
    } else {
        out.put("directory_entry_unavailable")?;
    }
    Ok(())
}
