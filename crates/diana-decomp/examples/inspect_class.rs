use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage,
    DiagnosticCause, Engine, EnvironmentId, ExternalMethodSignature, FailureKind, ImageOrigin,
    InputId, JavaTargetItem, Limits, MethodOutput, MethodSelector, Options, Request, RequestError,
    RunContext, SourceLanguage, render,
};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments: Vec<_> = std::env::args().skip(1).collect();
    let mut output = None;
    let mut catalog_path = None;
    while arguments.len() >= 2 {
        let flag = arguments[arguments.len() - 2].as_str();
        if !matches!(flag, "--output" | "--catalog") {
            break;
        }
        let catalog_flag = flag == "--catalog";
        let path = arguments.pop().ok_or("missing path")?;
        arguments.pop();
        if catalog_flag {
            catalog_path = Some(path);
        } else {
            output = Some(path);
        }
    }
    if !matches!(arguments.len(), 3 | 5) {
        return Err("usage: inspect_class <dex> <header-offset> <class-def-index> [<method-name> <full-proto>] [--catalog <signature-json>] [--output <json-path>]".into());
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
    #[derive(serde::Deserialize)]
    struct CatalogInput {
        environment_id: u64,
        methods: Vec<ExternalMethodSignature>,
    }
    let catalog = if let Some(path) = catalog_path {
        let facts: CatalogInput = serde_json::from_reader(std::fs::File::open(path)?)?;
        DeclarationCatalog::partial_signatures(EnvironmentId(facts.environment_id), facts.methods)
    } else {
        DeclarationCatalog::empty(EnvironmentId(0))
    };
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes: &bytes,
            header_offset,
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        catalog,
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
            let span = rendered.spans.iter().find(|span| span.method == key);
            let source = span.and_then(|span| rendered.source.get(span.start.0..span.end.0));
            json!({"method_id_idx":key.method.0,"status":status,"name_units":facts.map(|v| &v.name),"proto_units":facts.map(|v| &v.prototype),"access_flags":facts.map(|v| v.access_flags),"reason":reason,"source":source,"language":span.map(|span|language(span.language))})
        }).collect(),
    };
    let diagnostics: Vec<Value> = report.diagnostics().iter().map(|diagnostic| {
        let limit = match &diagnostic.cause {
            DiagnosticCause::Request(RequestError::JavaTargetLimit {item,actual,maximum}) => Some((match item {JavaTargetItem::ParameterWords=>"parameter_words",JavaTargetItem::MethodName=>"method_name",JavaTargetItem::MethodDescriptor=>"method_descriptor"},actual,maximum)),
            _ => None,
        };
        json!({"method_id_idx":diagnostic.method.map(|v|v.method.0),"kind":kind(diagnostic.cause.kind()),"code":format!("{:?}",diagnostic.code()),"severity":if diagnostic.cause.kind()==FailureKind::Unsupported {"warning"} else {"error"},"cause":format!("{:?}",diagnostic.cause),"target_item":limit.map(|v|v.0),"actual":limit.map(|v|v.1),"maximum":limit.map(|v|v.2)})
    }).collect();
    let spans: Vec<Value> = rendered
        .spans
        .iter()
        .map(|span| {
            json!({
                "method_id_idx":span.method.method.0,"start":span.start.0,"end":span.end.0,
                "language":language(span.language),
            })
        })
        .collect();
    let ClassOutput::Mixed {
        descriptor,
        access_flags,
        ..
    } = report.class();
    let value = json!({"schema":"diana.inspect/1","status":"mixed","header_offset":report.image().header_offset.0,"class_def_idx":class_def.0,"descriptor_units":descriptor,"access_flags":access_flags,"members":members,"diagnostics":diagnostics,"text":rendered.source,"spans":spans});
    if let Some(path) = output {
        serde_json::to_writer(std::fs::File::create(path)?, &value)?;
    } else {
        serde_json::to_writer(std::io::stdout().lock(), &value)?;
    }
    Ok(())
}

fn language(value: SourceLanguage) -> &'static str {
    match value {
        SourceLanguage::Java => "java",
        SourceLanguage::DexDebug => "dex_debug",
        SourceLanguage::OriginalExcerpt => "original_excerpt",
        SourceLanguage::Diagnostic => "diagnostic",
    }
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
