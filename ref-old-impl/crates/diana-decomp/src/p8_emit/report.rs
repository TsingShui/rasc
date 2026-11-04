use crate::substrate::owned::{TextBuffer, reserve};
use crate::{
    ClassOutput, DebugSourceKind, DeclarationOutput, DiagnosticCause, DiagnosticId, MethodOutput,
    OriginSpan, RenderedReport, Report, RequestError, RunContext, SourceByteOffset, SourceLanguage,
};

/// Renders only owned result data. It does not reopen input, run analysis or
/// repair unavailable Java. Source spans are UTF8 byte coordinates in this
/// returned buffer; every copy and span growth checks request control.
pub fn render(
    report: &Report,
    control: &mut RunContext<'_>,
) -> Result<RenderedReport, RequestError> {
    let mut output = TextBuffer::new();
    let mut spans = Vec::new();
    let mut class_spans = Vec::new();
    render_class(
        ClassSource {
            class: &report.class,
            annotations: report.annotations(),
        },
        report.diagnostics(),
        report.nested(),
        &mut output,
        &mut spans,
        &mut class_spans,
        control,
    )?;
    if matches!(report.class(), ClassOutput::Mixed { .. }) {
        for child in report.nested() {
            render_class(
                ClassSource {
                    class: &child.class,
                    annotations: child.annotations(),
                },
                child.diagnostics(),
                &[],
                &mut output,
                &mut spans,
                &mut class_spans,
                control,
            )?;
        }
    }
    control.checkpoint()?;
    Ok(RenderedReport {
        source: output.finish(),
        spans,
        class_spans,
    })
}

struct ClassSource<'a> {
    class: &'a ClassOutput,
    annotations: &'a crate::ClassAnnotations,
}

fn render_class(
    source: ClassSource<'_>,
    diagnostics: &[crate::Diagnostic],
    nested: &[crate::NestedReport],
    output: &mut TextBuffer,
    spans: &mut Vec<OriginSpan>,
    class_spans: &mut Vec<crate::ClassOriginSpan>,
    control: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let class = source.class;
    let class_start = SourceByteOffset(output.len());
    match class {
        ClassOutput::JavaUnit {
            declaration,
            fields,
            members,
            ..
        } => {
            output.push(declaration.source_header(), control)?;
            for field in fields {
                control.checkpoint()?;
                if matches!(
                    field.declaration,
                    crate::FieldDeclarationOutput::Generated(_)
                ) {
                    continue;
                }
                let crate::FieldDeclarationOutput::Java(declaration) = &field.declaration else {
                    return Err(RequestError::InternalInvariant(
                        "JavaUnit contains unavailable field",
                    ));
                };
                output.push(declaration.source(), control)?;
                output.push("\n", control)?;
            }
            output.push(declaration.preparation(), control)?;
            write_java_members(members, Some(declaration), output, spans, control)?;
            let mut open = Vec::new();
            for nested in nested {
                control.checkpoint()?;
                while open.len() >= nested.depth.0 {
                    let span: usize = open.pop().ok_or(RequestError::InternalInvariant(
                        "nested source depth is zero",
                    ))?;
                    output.push("}\n", control)?;
                    class_spans[span].end = SourceByteOffset(output.len());
                }
                if open.len().checked_add(1) != Some(nested.depth.0) {
                    return Err(RequestError::InternalInvariant(
                        "nested source depth has a gap",
                    ));
                }
                let plan = nested
                    .plan
                    .as_ref()
                    .ok_or(RequestError::InternalInvariant("JavaUnit lost nested plan"))?;
                let start = SourceByteOffset(output.len());
                output.push(plan.header(), control)?;
                for field in nested.class.fields() {
                    control.checkpoint()?;
                    if Some(field.key.field) == plan.capture_field() {
                        continue;
                    }
                    let crate::FieldDeclarationOutput::Java(declaration) = &field.declaration
                    else {
                        return Err(RequestError::InternalInvariant("nested field lost source"));
                    };
                    output.push(declaration.source(), control)?;
                    output.push("\n", control)?;
                }
                output.push(plan.preparation(), control)?;
                write_java_members(nested.class.members(), None, output, spans, control)?;
                let key = match &nested.class {
                    ClassOutput::JavaUnit { key, .. } | ClassOutput::Mixed { key, .. } => *key,
                };
                reserve(&mut open, 1, control)?;
                open.push(class_spans.len());
                reserve(class_spans, 1, control)?;
                class_spans.push(crate::ClassOriginSpan {
                    class: key,
                    start,
                    end: SourceByteOffset(output.len()),
                });
            }
            while let Some(span) = open.pop() {
                output.push("}\n", control)?;
                class_spans[span].end = SourceByteOffset(output.len());
            }
            output.push("}\n", control)?;
        }
        ClassOutput::Mixed {
            descriptor,
            shell,
            fields,
            members,
            ..
        } => {
            output.push(
                "Mixed: readable members and original facts\nOriginal class ",
                control,
            )?;
            for scalar in char::decode_utf16(descriptor.iter().copied()) {
                output.character(
                    scalar.map_err(|_| {
                        RequestError::InternalInvariant(
                            "checked class descriptor has an isolated surrogate",
                        )
                    })?,
                    control,
                )?;
            }
            output.push("\n", control)?;
            let key = match class {
                ClassOutput::JavaUnit { key, .. } | ClassOutput::Mixed { key, .. } => *key,
            };
            if source.annotations.original().owner() != key {
                return Err(RequestError::InternalInvariant(
                    "class annotation facts lost owner",
                ));
            }
            match source.annotations.status() {
                crate::AnnotationStatus::Complete => {}
                crate::AnnotationStatus::Partial(reason)
                | crate::AnnotationStatus::Unavailable(reason) => {
                    write_reason(diagnostics, reason, output, control)?
                }
            }
            for annotation in source.annotations.original().records() {
                control.checkpoint()?;
                output.arguments(
                    format_args!(
                        "Original annotation {:?} {:?} visibility={} {:?} {:?}\n",
                        annotation.location(),
                        annotation.type_id(),
                        annotation.visibility(),
                        annotation.extent(),
                        annotation.tokens()
                    ),
                    control,
                )?;
            }
            if let Some(prefix) = source.annotations.original().unavailable_prefix() {
                output.arguments(
                    format_args!(
                        "Original annotation pointer {:?}: {:?}\n",
                        prefix.range(),
                        prefix.bytes()
                    ),
                    control,
                )?;
            }
            for field in fields {
                write_field(diagnostics, field, output, control)?;
            }
            match shell {
                DeclarationOutput::Unavailable(id) => {
                    write_reason(diagnostics, *id, output, control)?
                }
            }
            for member in members {
                control.checkpoint()?;
                let key = member.key();
                output.arguments(
                    format_args!(
                        "method {}/{}/{}\n",
                        key.class.dex.0, key.class.class_def.0, key.method.0
                    ),
                    control,
                )?;
                let (source, language) = match member {
                    MethodOutput::Generated { original, .. } => {
                        (Some(original.source()), SourceLanguage::DexDebug)
                    }
                    MethodOutput::Java { source, .. }
                    | MethodOutput::JavaInitializer { source, .. } => {
                        (Some(source.as_str()), SourceLanguage::Java)
                    }
                    MethodOutput::DeclarationOnly { declaration, .. } => {
                        (Some(declaration.source()), SourceLanguage::Java)
                    }
                    MethodOutput::Debug { source, reason, .. } => {
                        write_reason(diagnostics, *reason, output, control)?;
                        (Some(source.source()), SourceLanguage::DexDebug)
                    }
                    MethodOutput::Unavailable {
                        excerpt, reason, ..
                    } => {
                        write_reason(diagnostics, *reason, output, control)?;
                        let language = match excerpt.as_ref().map(|source| source.kind()) {
                            Some(DebugSourceKind::WordExcerpt) => SourceLanguage::OriginalExcerpt,
                            Some(DebugSourceKind::Snapshot) => SourceLanguage::DexDebug,
                            None => SourceLanguage::Diagnostic,
                        };
                        (excerpt.as_ref().map(|source| source.source()), language)
                    }
                };
                let start = SourceByteOffset(output.len());
                match source {
                    Some(source) => output.push(source, control)?,
                    None => output.push(
                        "Unavailable: original member source could not be retained",
                        control,
                    )?,
                }
                let end = SourceByteOffset(output.len());
                reserve(spans, 1, control)?;
                spans.push(OriginSpan {
                    method: key,
                    start,
                    end,
                    language,
                });
                output.push("\n", control)?;
            }
        }
    }
    let key = match class {
        ClassOutput::JavaUnit { key, .. } | ClassOutput::Mixed { key, .. } => *key,
    };
    reserve(class_spans, 1, control)?;
    class_spans.push(crate::ClassOriginSpan {
        class: key,
        start: class_start,
        end: SourceByteOffset(output.len()),
    });
    if matches!(class, ClassOutput::JavaUnit { .. }) {
        for member in class.members() {
            control.checkpoint()?;
            if let MethodOutput::Generated {
                declaration: crate::GeneratedMember::Enum(declaration),
                ..
            } = member
            {
                if declaration.owner() != key {
                    return Err(RequestError::InternalInvariant(
                        "generated enum lost owner span",
                    ));
                }
                reserve(spans, 1, control)?;
                spans.push(OriginSpan {
                    method: member.key(),
                    start: class_start,
                    end: SourceByteOffset(output.len()),
                    language: SourceLanguage::Java,
                });
            }
        }
    }
    Ok(())
}

fn write_java_members(
    members: &[MethodOutput],
    unit: Option<&crate::JavaUnit>,
    output: &mut TextBuffer,
    spans: &mut Vec<OriginSpan>,
    control: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    for member in members {
        control.checkpoint()?;
        if matches!(member, MethodOutput::Generated { .. }) {
            continue;
        }
        let source = match member {
            MethodOutput::Java { source, .. } | MethodOutput::JavaInitializer { source, .. } => {
                source.as_str()
            }
            MethodOutput::DeclarationOnly { declaration, .. } => unit
                .and_then(|unit| unit.annotation_element_source(member.key().method))
                .unwrap_or(declaration.source()),
            _ => {
                return Err(RequestError::InternalInvariant(
                    "JavaUnit contains a member without readable source",
                ));
            }
        };
        let start = SourceByteOffset(output.len());
        output.push(source, control)?;
        let end = SourceByteOffset(output.len());
        reserve(spans, 1, control)?;
        spans.push(OriginSpan {
            method: member.key(),
            start,
            end,
            language: SourceLanguage::Java,
        });
        output.push("\n", control)?;
    }
    for member in members {
        control.checkpoint()?;
        if let MethodOutput::Generated {
            declaration: crate::GeneratedMember::Bridge(declaration),
            ..
        } = member
        {
            let target = members
                .iter()
                .find(|member| member.key().method == declaration.target())
                .ok_or(RequestError::InternalInvariant(
                    "generated bridge lost owner",
                ))?;
            let span = spans
                .iter()
                .find(|span| span.method == target.key())
                .ok_or(RequestError::InternalInvariant(
                    "generated bridge lost owner span",
                ))?;
            let start = span.start;
            let end = span.end;
            reserve(spans, 1, control)?;
            spans.push(OriginSpan {
                method: member.key(),
                start,
                end,
                language: SourceLanguage::Java,
            });
        }
    }
    Ok(())
}

fn write_field(
    report: &[crate::Diagnostic],
    field: &crate::FieldOutput,
    output: &mut TextBuffer,
    control: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    output.arguments(
        format_args!(
            "field {}/{}/{} flags={} {:?}\n",
            field.key.class.dex.0,
            field.key.class.class_def.0,
            field.key.field.0,
            field.access_flags,
            field.storage
        ),
        control,
    )?;
    if let Some(facts) = &field.facts {
        for (label, units) in [
            ("name_utf16", &facts.name),
            ("descriptor_utf16", &facts.descriptor),
        ] {
            output.push(label, control)?;
            output.push(":", control)?;
            for &unit in units {
                output.arguments(format_args!(" {unit}"), control)?;
            }
            output.push("\n", control)?;
        }
    }
    match &field.initial {
        crate::FieldInitialValue::Encoded {
            tag,
            arg,
            range,
            bytes,
        } => {
            output.arguments(
                format_args!(
                    "encoded tag={tag} arg={arg} range={}..{} bytes:",
                    range.start.0, range.end.0
                ),
                control,
            )?;
            for &byte in bytes {
                output.arguments(format_args!(" {byte:02x}"), control)?;
            }
            output.push("\n", control)?;
        }
        crate::FieldInitialValue::ImplicitDefault => output.push("implicit default\n", control)?,
        crate::FieldInitialValue::InstanceDefault => output.push("instance default\n", control)?,
        crate::FieldInitialValue::Unavailable(reason) => {
            write_reason(report, *reason, output, control)?
        }
    }
    match &field.declaration {
        crate::FieldDeclarationOutput::Generated(crate::GeneratedField::OuterCapture(_)) => {
            output.push("Generated: nested outer capture\n", control)?
        }
        crate::FieldDeclarationOutput::Generated(crate::GeneratedField::Enum(field)) => output
            .arguments(
                format_args!("Generated: enum {:?}\n", field.role()),
                control,
            )?,
        crate::FieldDeclarationOutput::Java(declaration) => {
            output.push(declaration.source(), control)?;
            output.push("\n", control)?;
        }
        crate::FieldDeclarationOutput::Unavailable(reason) => {
            write_reason(report, *reason, output, control)?
        }
    }
    Ok(())
}

fn write_reason(
    report: &[crate::Diagnostic],
    id: DiagnosticId,
    output: &mut TextBuffer,
    control: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let diagnostic = report.get(id.0).ok_or(RequestError::InternalInvariant(
        "result diagnostic is absent",
    ))?;
    if diagnostic.cause.kind() == crate::FailureKind::Cancelled {
        return Err(RequestError::InternalInvariant(
            "cancelled report reached rendering",
        ));
    }
    match &diagnostic.cause {
        DiagnosticCause::Input(error) => output.arguments(format_args!("{error}"), control)?,
        DiagnosticCause::Method(error) => output.arguments(format_args!("{error}"), control)?,
        DiagnosticCause::Debug(error) => output.arguments(format_args!("{error}"), control)?,
        DiagnosticCause::Request(error) => output.arguments(format_args!("{error}"), control)?,
        DiagnosticCause::Original(issue) => {
            output.arguments(format_args!("{:?}: {issue:?}", issue.kind()), control)?
        }
        DiagnosticCause::Unsupported(reason) => {
            output.arguments(format_args!("Unsupported: {reason:?}"), control)?
        }
    }
    output.push("\n", control)
}
