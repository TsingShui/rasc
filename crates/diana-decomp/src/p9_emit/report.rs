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
        ClassOutput::Mixed {
            descriptor,
            shell,
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
                    MethodOutput::Java { source, .. } => {
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
