//! `get-class` / `get-method`: deliver the engine report as text or JSON.

use std::path::Path;

use diana_decomp::{
    ClassOutput, Diagnostic, DiagnosticCause, FailureKind, MethodFacts, MethodOutput,
    SourceLanguage,
};

use super::output::{Failure, Format, publish};
use crate::analysis::at;
use crate::analysis::decompile::{self, Decompiled};
use crate::analysis::request::RequestScope;
use crate::analysis::selector::{self, Selector};

/// Which object the command selects.
pub enum Kind {
    Class { jobs: usize },
    Method,
}

pub fn run(
    input: &Path,
    text: &str,
    kind: Kind,
    format: Format,
    output: Option<&Path>,
    verbose: bool,
) -> Result<(), Failure> {
    let selector = match kind {
        Kind::Class { .. } => Selector::Class(selector::class(text).map_err(|error| Failure {
            code: error.code(),
            message: error.to_string(),
        })?),
        Kind::Method => Selector::Method(selector::method(text).map_err(|error| Failure {
            code: error.code(),
            message: error.to_string(),
        })?),
    };
    let scope = RequestScope::open(input, &super::options()).map_err(Failure::input)?;

    let jobs = match kind {
        Kind::Class { jobs } => jobs,
        Kind::Method => 1,
    };
    let decompiled =
        decompile::run_with_jobs(&scope, &selector, jobs).map_err(|error| Failure {
            code: error.code(),
            message: error.to_string(),
        })?;
    let payload = match format {
        Format::Text => text_payload(&decompiled, verbose),
        Format::Json => json_payload(&decompiled, verbose),
    };
    publish(output, payload.as_bytes(), format)
}

fn text_payload(decompiled: &Decompiled, verbose: bool) -> String {
    let mut payload = match decompiled.method {
        Some(_) => {
            // A method request displays exactly the selected member, without the
            // class shell or another member's source.
            let key = decompiled.report.class().members()[0].key();
            match decompiled
                .rendered
                .spans
                .iter()
                .find(|span| span.method == key)
            {
                Some(span) => decompiled.rendered.source[span.start.0..span.end.0].to_owned(),
                None => decompiled.rendered.source.clone(),
            }
        }
        None => decompiled.rendered.source.clone(),
    };
    if !verbose {
        return payload;
    }
    for diagnostic in decompiled.report.diagnostics() {
        let Some(key) = diagnostic.method else {
            continue;
        };
        if diagnostic.cause.kind() != FailureKind::Unsupported {
            continue;
        }
        let Some(facts) = decompiled
            .report
            .class()
            .members()
            .iter()
            .find(|member| member.key() == key)
            .and_then(member_facts)
        else {
            continue;
        };
        if !payload.ends_with('\n') {
            payload.push('\n');
        }
        payload.push_str("// diana uncertainty: ");
        payload.push_str(&format!("{:?}", diagnostic.code()));
        payload.push_str(" at ");
        payload.push_str(&member_at(decompiled, facts));
        payload.push('\n');
    }
    payload
}

fn class_descriptor(decompiled: &Decompiled) -> Vec<u16> {
    match decompiled.report.class() {
        ClassOutput::JavaUnit { descriptor, .. } | ClassOutput::Mixed { descriptor, .. } => {
            descriptor.clone()
        }
    }
}

fn member_at(decompiled: &Decompiled, facts: &MethodFacts) -> String {
    at::method(
        entry_of(decompiled),
        logical_of(decompiled),
        facts.key.class.class_def.0,
        facts.key.method.0,
        &class_descriptor(decompiled),
        &facts.name,
        &facts.prototype,
    )
}

fn entry_of(decompiled: &Decompiled) -> Option<usize> {
    decompiled.entry
}

fn logical_of(decompiled: &Decompiled) -> u32 {
    decompiled.logical
}

fn status_of(member: &MethodOutput) -> &'static str {
    match member {
        MethodOutput::Java { .. }
        | MethodOutput::JavaInitializer { .. }
        | MethodOutput::DeclarationOnly { .. } => "java",
        MethodOutput::Debug { .. } => "debug",
        MethodOutput::Unavailable { .. } => "unavailable",
        MethodOutput::Generated { .. } => "java",
    }
}

fn language_of(member: &MethodOutput) -> &'static str {
    match member {
        MethodOutput::Java { .. } | MethodOutput::JavaInitializer { .. } => "java",
        MethodOutput::DeclarationOnly { .. } => "java",
        MethodOutput::Generated { .. } => "java",
        MethodOutput::Debug { source, .. } => language_name(source.kind()),
        MethodOutput::Unavailable { excerpt, .. } => match excerpt {
            Some(source) => language_name(source.kind()),
            None => "none",
        },
    }
}

fn language_name(kind: diana_decomp::DebugSourceKind) -> &'static str {
    match kind {
        diana_decomp::DebugSourceKind::WordExcerpt => "raw_excerpt",
        diana_decomp::DebugSourceKind::Snapshot => "dex_debug",
    }
}

fn severity(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Malformed
        | FailureKind::InternalInvariant
        | FailureKind::ResourceExhausted => "error",
        FailureKind::Unsupported => "warning",
    }
}

fn diagnostic_value(diagnostic: &Diagnostic, decompiled: &Decompiled) -> serde_json::Value {
    let method_at = diagnostic.method.map(|key| {
        let facts = decompiled
            .report
            .class()
            .members()
            .iter()
            .find(|member| member.key() == key)
            .and_then(member_facts);
        match facts {
            Some(facts) => member_at(decompiled, facts),
            None => at::class(
                entry_of(decompiled),
                logical_of(decompiled),
                key.class.class_def.0,
                &class_descriptor(decompiled),
            ),
        }
    });
    let (phase, check, code_unit) = match &diagnostic.cause {
        DiagnosticCause::Unsupported(diana_decomp::OutputReason::JavaCheckFailed {
            phase,
            check,
            pc,
            ..
        }) => (Some(*phase), Some(*check), pc.map(|pc| pc.0)),
        DiagnosticCause::Unsupported(diana_decomp::OutputReason::BindingEvidenceMissing {
            pc,
            ..
        }) => (Some("P4"), Some("method_signature_evidence"), Some(pc.0)),
        _ => (None, None, None),
    };
    serde_json::json!({
        "code": format!("{:?}", diagnostic.code()),
        "kind": format!("{:?}", diagnostic.cause.kind()).to_lowercase(),
        "severity": severity(diagnostic.cause.kind()),
        "method_at": method_at,
        "code_unit": code_unit,
        "phase": phase,
        "check": check,
        "detail": match &diagnostic.cause {
            DiagnosticCause::Request(error) => error.to_string(),
            other => format!("{other:?}"),
        },
    })
}

fn member_facts(member: &MethodOutput) -> Option<&MethodFacts> {
    match member {
        MethodOutput::Generated { facts, .. }
        | MethodOutput::Java { facts, .. }
        | MethodOutput::JavaInitializer { facts, .. }
        | MethodOutput::DeclarationOnly { facts, .. } => Some(facts),
        MethodOutput::Debug { facts, .. } | MethodOutput::Unavailable { facts, .. } => {
            facts.as_ref()
        }
    }
}

fn json_payload(decompiled: &Decompiled, verbose: bool) -> String {
    let diagnostics: Vec<_> = decompiled
        .report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic_value(diagnostic, decompiled))
        .collect();
    let mut methods = Vec::new();
    for member in decompiled.report.class().members() {
        let facts = member_facts(member);
        let at_text = facts
            .map(|facts| member_at(decompiled, facts))
            .unwrap_or_else(|| {
                at::class(
                    entry_of(decompiled),
                    logical_of(decompiled),
                    member.key().class.class_def.0,
                    &class_descriptor(decompiled),
                )
            });
        let span = decompiled
            .rendered
            .spans
            .iter()
            .find(|span| span.method == member.key());
        let source = match member {
            MethodOutput::Java { source, .. } | MethodOutput::JavaInitializer { source, .. } => {
                Some(source.clone())
            }
            MethodOutput::DeclarationOnly { declaration, .. } => {
                Some(declaration.source().to_owned())
            }
            MethodOutput::Debug { source, .. } => Some(source.source().to_owned()),
            MethodOutput::Unavailable { excerpt, .. } => {
                excerpt.as_ref().map(|source| source.source().to_owned())
            }
            MethodOutput::Generated { original, .. } => {
                let _ = original;
                None
            }
        };
        let reason = match member {
            MethodOutput::Debug { reason, .. } | MethodOutput::Unavailable { reason, .. } => {
                diagnostics.get(reason.0).cloned()
            }
            _ => None,
        };
        let uncertainties: Vec<_> = decompiled
            .report
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.method == Some(member.key())
                    && diagnostic.cause.kind() == FailureKind::Unsupported
            })
            .map(|diagnostic| diagnostic_value(diagnostic, decompiled))
            .collect();
        let mut record = serde_json::json!({
            "at": at_text,
            "status": status_of(member),
            "body": match member {
                MethodOutput::DeclarationOnly { .. } => serde_json::Value::String("declaration".to_owned()),
                MethodOutput::Java { .. }
                | MethodOutput::JavaInitializer { .. }
                | MethodOutput::Generated { .. } => serde_json::Value::String("code".to_owned()),
                MethodOutput::Debug { .. } | MethodOutput::Unavailable { .. } => serde_json::Value::Null,
            },
            "emission": match member {
                MethodOutput::Generated { .. } => serde_json::Value::String("generated".to_owned()),
                MethodOutput::Java { .. } | MethodOutput::JavaInitializer { .. } => {
                    serde_json::Value::String("explicit".to_owned())
                }
                _ => serde_json::Value::Null,
            },
            "language": language_of(member),
            "source": source,
            "source_range": span.map(|span| serde_json::json!([span.start.0, span.end.0])),
            "spans": [],
            "reason": reason,
        });
        if verbose {
            record["uncertainties"] = serde_json::Value::Array(uncertainties);
        }
        methods.push(record);
    }
    let spans: Vec<_> = decompiled
        .rendered
        .spans
        .iter()
        .map(|span| {
            let facts = decompiled
                .report
                .class()
                .members()
                .iter()
                .find(|member| member.key() == span.method)
                .and_then(member_facts);
            serde_json::json!({
                "range": [span.start.0, span.end.0],
                "member_at": facts.map(|facts| member_at(decompiled, facts)),
                "sites": [],
                "synthetic_reason": match span.language {
                    SourceLanguage::Java => serde_json::Value::Null,
                    _ => serde_json::Value::Null,
                },
            })
        })
        .collect();
    let status = match decompiled.method {
        Some(_) => status_of(&decompiled.report.class().members()[0]),
        None => decompiled.status,
    };
    serde_json::json!({
        "schema_version": 2,
        "command": if decompiled.method.is_some() { "get-method" } else { "get-class" },
        "selection": {
            "class_at": decompiled.class_at,
            "method_at": decompiled.method_at,
        },
        "status": status,
        "source": decompiled.rendered.source,
        "methods": methods,
        "diagnostics": diagnostics,
        "spans": spans,
    })
    .to_string()
}
