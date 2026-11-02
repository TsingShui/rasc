//! Development caller for the application decompilation adapter (E2E app seams).
//!
//! Usage: app_decompile <input> <class-selector> [--method REF] [--output FILE]
use diana::analysis::decompile;
use diana::analysis::input::InputOptions;
use diana::analysis::request::RequestScope;
use diana::analysis::selector::{self, Selector};
use diana_decomp::{ClassOutput, MethodOutput};
use serde_json::json;

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("input");
    let selector_text = args.next().expect("selector");
    let mut method = None;
    let mut output = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--method" => method = args.next(),
            "--output" => output = args.next(),
            other => panic!("unknown flag {other}"),
        }
    }
    let selector = match method {
        Some(reference) => {
            Selector::Method(selector::method(&reference).expect("method reference"))
        }
        None => Selector::Class(selector::class(&selector_text).expect("class selector")),
    };
    let scope = RequestScope::open(
        std::path::Path::new(&input),
        &InputOptions {
            limits: Default::default(),
            allowed_root: None,
        },
    )
    .expect("lease");
    let mut control = diana_decomp::RunContext::new(Default::default(), &|| false);
    let decompiled = decompile::run(&scope, &selector, &mut control).expect("decompile");
    let (status, descriptor, access_flags) = match decompiled.report.class() {
        ClassOutput::JavaUnit {
            descriptor,
            access_flags,
            ..
        } => ("java_unit", descriptor.clone(), *access_flags),
        ClassOutput::Mixed {
            descriptor,
            access_flags,
            ..
        } => ("mixed", descriptor.clone(), *access_flags),
    };
    let mut members = Vec::new();
    for member in decompiled.report.class().members() {
        let key = member.key();
        let (facts, status, language, source, reason) = match member {
            MethodOutput::Generated {
                facts, original, ..
            } => (
                Some(facts),
                "generated",
                "dex_debug",
                original.source().to_owned(),
                None,
            ),
            MethodOutput::Java { facts, source, .. }
            | MethodOutput::JavaInitializer { facts, source, .. } => {
                (Some(facts), "java", "java", source.clone(), None)
            }
            MethodOutput::DeclarationOnly {
                facts, declaration, ..
            } => (
                Some(facts),
                "declaration_only",
                "java",
                declaration.source().to_owned(),
                None,
            ),
            MethodOutput::Debug { source, reason, .. } => (
                None,
                "debug",
                "dex_debug",
                source.source().to_owned(),
                Some(reason.0),
            ),
            MethodOutput::Unavailable {
                reason, excerpt, ..
            } => {
                let (language, source) = match excerpt {
                    Some(source) => ("dex_debug", source.source().to_owned()),
                    None => ("none", String::new()),
                };
                (None, "unavailable", language, source, Some(reason.0))
            }
        };
        let facts = facts
            .map(|facts| (Some(facts.name.clone()), Some(facts.prototype.clone())))
            .unwrap_or((None, None));
        let span = decompiled
            .rendered
            .spans
            .iter()
            .find(|span| span.method == key);
        members.push(json!({
            "class_def_idx": key.class.class_def.0,
            "owner_units": descriptor,
            "method_id_idx": key.method.0,
            "name_units": facts.0,
            "proto_units": facts.1,
            "status": status,
            "language": language,
            "source": source,
            "reason": reason,
            "span_start": span.map(|span| span.start.0),
            "span_end": span.map(|span| span.end.0),
        }));
    }
    let diagnostics: Vec<_> = decompiled
        .report
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            json!({
                "kind": format!("{:?}", diagnostic.cause.kind()).to_lowercase(),
                "code": format!("{:?}", diagnostic.code()),
                "method_id_idx": diagnostic.method.map(|key| key.method.0),
            })
        })
        .collect();
    let spans: Vec<_> = decompiled
        .rendered
        .spans
        .iter()
        .map(|span| {
            json!({
                "method_id_idx": span.method.method.0,
                "start": span.start.0,
                "end": span.end.0,
                "language": match span.language {
                    diana_decomp::SourceLanguage::Java => "java",
                    diana_decomp::SourceLanguage::DexDebug => "dex_debug",
                    diana_decomp::SourceLanguage::OriginalExcerpt => "original_excerpt",
                    diana_decomp::SourceLanguage::Diagnostic => "none",
                },
            })
        })
        .collect();
    let value = json!({
        "schema": "diana.inspect/1",
        "header_offset": decompiled.header_offset.0,
        "class_def_idx": decompiled.class.class_def.0,
        "status": status,
        "descriptor_units": descriptor,
        "access_flags": access_flags,
        "text": decompiled.rendered.source,
        "members": members,
        "diagnostics": diagnostics,
        "spans": spans,
        "published_success": false,
    });
    match output {
        Some(path) => std::fs::write(path, value.to_string()).expect("write output"),
        None => println!("{value}"),
    }
}
