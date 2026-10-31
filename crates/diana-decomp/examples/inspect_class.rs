use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage, Engine,
    EnvironmentId, FailureKind, ImageOrigin, InputId, Limits, MethodOutput, MethodSelector,
    Options, Request, RunContext, SourceLanguage, render,
};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if !matches!(arguments.len(), 3 | 5) {
        return Err("usage: inspect_class <dex> <header-offset> <class-def-index> [<method-name> <full-proto>]".into());
    }
    let bytes = std::fs::read(&arguments[0])?;
    let header_offset = ByteOffset(arguments[1].parse()?);
    let class_def = ClassDefId(arguments[2].parse()?);
    let method = if arguments.len() == 5 {
        Some(MethodSelector {
            name: arguments[3].encode_utf16().collect(),
            prototype: arguments[4].encode_utf16().collect(),
        })
    } else {
        None
    };
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes: &bytes,
            header_offset,
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::empty(EnvironmentId(0)),
        &mut control,
    )?;
    let report = engine.decompile(
        Request {
            class: ClassKey {
                dex: DexId(0),
                class_def,
            },
            method,
        },
        &mut control,
    )?;
    drop(engine);
    drop(bytes);
    let rendered = render(&report, &mut control)?;
    let members: Vec<Value> = match report.class() {
        ClassOutput::Mixed { members, .. } => members.iter().map(|member| {
            let key = member.key();
            let (status, facts, reason) = match member {
                MethodOutput::DeclarationOnly { facts, .. } => ("declaration_only", Some(facts), None),
                MethodOutput::Debug { facts, reason, .. } => ("debug", facts.as_ref(), Some(reason.0)),
                MethodOutput::Unavailable { facts, reason, .. } => ("unavailable", facts.as_ref(), Some(reason.0)),
            };
            json!({"method_id_idx":key.method.0,"status":status,"name_units":facts.map(|v| &v.name),"proto_units":facts.map(|v| &v.prototype),"access_flags":facts.map(|v| v.access_flags),"reason":reason})
        }).collect(),
    };
    let diagnostics: Vec<Value> = report.diagnostics().iter().map(|diagnostic| json!({
        "method_id_idx":diagnostic.method.map(|v|v.method.0),"kind":kind(diagnostic.cause.kind()),"code":format!("{:?}",diagnostic.code()),"severity":if diagnostic.cause.kind()==FailureKind::Unsupported {"warning"} else {"error"},"cause":format!("{:?}",diagnostic.cause),
    })).collect();
    let spans: Vec<Value> = rendered.spans.iter().map(|span| json!({
        "method_id_idx":span.method.method.0,"start":span.start.0,"end":span.end.0,
        "language":match span.language { SourceLanguage::Java=>"java",SourceLanguage::DexDebug=>"dex_debug",SourceLanguage::OriginalExcerpt=>"original_excerpt",SourceLanguage::Diagnostic=>"diagnostic" },
    })).collect();
    let ClassOutput::Mixed {
        descriptor,
        access_flags,
        ..
    } = report.class();
    let value = json!({"schema":"diana.inspect/1","status":"mixed","header_offset":report.image().header_offset.0,"class_def_idx":class_def.0,"descriptor_units":descriptor,"access_flags":access_flags,"members":members,"diagnostics":diagnostics,"text":rendered.source,"spans":spans});
    serde_json::to_writer(std::io::stdout().lock(), &value)?;
    Ok(())
}

fn kind(value: FailureKind) -> &'static str {
    match value {
        FailureKind::Malformed => "malformed",
        FailureKind::Unsupported => "unsupported",
        FailureKind::BudgetExceeded => "budget_exceeded",
        FailureKind::Cancelled => "cancelled",
        FailureKind::InternalInvariant => "internal_invariant",
    }
}
