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
    let mut output = TextBuffer::report(control);
    let mut spans = Vec::new();
    match &report.class {
        ClassOutput::JavaUnit {
            declaration,
            fields,
            members,
            ..
        } => {
            output.push(declaration.source_header(), control)?;
            for field in fields {
                control.checkpoint()?;
                let crate::FieldDeclarationOutput::Java(declaration) = &field.declaration else {
                    return Err(RequestError::InternalInvariant(
                        "JavaUnit contains unavailable field",
                    ));
                };
                output.push(declaration.source(), control)?;
                output.push("\n", control)?;
            }
            output.push(declaration.preparation(), control)?;
            for member in members {
                control.checkpoint()?;
                if matches!(member, MethodOutput::Generated { .. }) {
                    continue;
                }
                let source = match member {
                    MethodOutput::Java { source, .. }
                    | MethodOutput::JavaInitializer { source, .. } => source.as_str(),
                    MethodOutput::DeclarationOnly { declaration, .. } => declaration.source(),
                    _ => {
                        return Err(RequestError::InternalInvariant(
                            "JavaUnit contains an unverified member",
                        ));
                    }
                };
                let start = SourceByteOffset(output.len());
                output.push(source, control)?;
                let end = SourceByteOffset(output.len());
                reserve(&mut spans, 1, control)?;
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
                if let MethodOutput::Generated { declaration, .. } = member {
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
                    reserve(&mut spans, 1, control)?;
                    spans.push(OriginSpan {
                        method: member.key(),
                        start,
                        end,
                        language: SourceLanguage::Java,
                    });
                }
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
                "Mixed: not a complete Java compilation unit\nOriginal class ",
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
            for field in fields {
                write_field(report, field, &mut output, control)?;
            }
            match shell {
                DeclarationOutput::Unavailable(id) => {
                    write_reason(report, *id, &mut output, control)?
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
                        write_reason(report, *reason, &mut output, control)?;
                        (Some(source.source()), SourceLanguage::DexDebug)
                    }
                    MethodOutput::Unavailable {
                        excerpt, reason, ..
                    } => {
                        write_reason(report, *reason, &mut output, control)?;
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
                reserve(&mut spans, 1, control)?;
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
    control.checkpoint()?;
    Ok(RenderedReport {
        source: output.finish(),
        spans,
    })
}

fn write_field(
    report: &Report,
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
    report: &Report,
    id: DiagnosticId,
    output: &mut TextBuffer,
    control: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let diagnostic = report
        .diagnostics
        .get(id.0)
        .ok_or(RequestError::InternalInvariant(
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
