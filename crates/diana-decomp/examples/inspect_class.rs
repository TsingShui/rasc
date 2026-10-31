use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage,
    DiagnosticCause, Engine, EnvironmentId, ExternalMethodSignature, FailureKind, ImageOrigin,
    InputId, JavaTargetItem, Limits, MethodOutput, MethodSelector, Options, Request, RequestError,
    RunContext, SourceLanguage, render,
};
use serde_json::{Value, json};
use std::cell::Cell;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments: Vec<_> = std::env::args().skip(1).collect();
    let mut output = None;
    let mut catalog_path = None;
    let mut control_path = None;
    while arguments.len() >= 2 {
        let flag = arguments[arguments.len() - 2].as_str();
        if !matches!(flag, "--output" | "--catalog" | "--control") {
            break;
        }
        let catalog_flag = flag == "--catalog";
        let control_flag = flag == "--control";
        let path = arguments.pop().ok_or("missing path")?;
        arguments.pop();
        if catalog_flag {
            catalog_path = Some(path);
        } else if control_flag {
            control_path = Some(path);
        } else {
            output = Some(path);
        }
    }
    if !matches!(arguments.len(), 3 | 5) {
        return Err("usage: inspect_class <dex> <header-offset> <class-def-index> [<method-name> <full-proto>] [--catalog <signature-json>] [--control <control-json>] [--output <json-path>]".into());
    }
    let bytes = std::fs::read(&arguments[0])?;
    let header_offset = ByteOffset(arguments[1].parse()?);
    let class_selection = &arguments[2];
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
    #[derive(Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct Controls {
        limits: Limits,
        cancellation: Option<Cancellation>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Cancellation {
        arm: String,
        #[serde(rename = "cancelAfterChecks")]
        after: u64,
    }
    let controls: Controls = if let Some(path) = control_path {
        serde_json::from_reader(std::fs::File::open(path)?)?
    } else {
        Controls::default()
    };
    if controls
        .cancellation
        .as_ref()
        .is_some_and(|policy| policy.arm != "after-open")
    {
        return Err("unsupported cancellation arm".into());
    }
    let armed = Cell::new(false);
    let checks = Cell::new(0u64);
    let cancelled = || {
        let Some(policy) = &controls.cancellation else {
            return false;
        };
        if !armed.get() {
            return false;
        }
        let count = checks.get().saturating_add(1);
        checks.set(count);
        count >= policy.after
    };
    let mut control = RunContext::new(controls.limits, &cancelled);
    let engine = match Engine::open(
        vec![DexImage {
            bytes: &bytes,
            header_offset,
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        catalog,
        &mut control,
    ) {
        Ok(engine) => engine,
        Err(error) => return write_value(failure(DiagnosticCause::Input(error), None), output),
    };
    armed.set(true);
    let class_def = if class_selection.starts_with('L') {
        if header_offset != ByteOffset(0) {
            return Err("symbolic caller selection requires a standalone DEX".into());
        }
        let header = diana_decomp::input::read_header(&bytes, &mut control)?;
        let index = diana_decomp::input::ProgramIndex::open(&bytes, &mut control)?;
        let descriptor: Vec<u16> = class_selection.encode_utf16().collect();
        let mut found = None;
        for id in 0..header.class_defs.count {
            if index
                .class(ClassDefId(id), &mut control)?
                .descriptor
                .matches(&descriptor, &mut control)?
            {
                if found.is_some() {
                    return Err("ambiguous caller class selection".into());
                }
                found = Some(ClassDefId(id));
            }
        }
        found.ok_or("caller class selection is not defined")?
    } else {
        ClassDefId(class_selection.parse()?)
    };
    let report = match engine.decompile(
        Request {
            class: ClassKey {
                dex: DexId(0),
                class_def,
            },
            method,
        },
        &mut control,
    ) {
        Ok(report) => report,
        Err(error) => return write_value(failure(DiagnosticCause::Request(error), None), output),
    };
    drop(engine);
    drop(bytes);
    let rendered = match render(&report, &mut control) {
        Ok(rendered) => rendered,
        Err(error) => {
            return write_value(
                failure(DiagnosticCause::Request(error), Some(&report)),
                output,
            );
        }
    };
    let members: Vec<Value> = match report.class() {
        ClassOutput::Mixed { members, .. } => members.iter().map(|member| {
            let key = member.key();
            let (status, facts, reason) = match member {
                MethodOutput::Java { facts, .. } => ("java",Some(facts),None),
                MethodOutput::DeclarationOnly { facts, .. } => ("declaration_only", Some(facts), None),
                MethodOutput::Debug { facts, reason, .. } => ("debug", facts.as_ref(), Some(reason.0)),
                MethodOutput::Unavailable { facts, reason, .. } => ("unavailable", facts.as_ref(), Some(reason.0)),
            };
            let span = rendered.spans.iter().find(|span| span.method == key);
            let source = span.and_then(|span| rendered.source.get(span.start.0..span.end.0));
            json!({"method_id_idx":key.method.0,"status":status,"name_units":facts.map(|v| &v.name),"proto_units":facts.map(|v| &v.prototype),"access_flags":facts.map(|v| v.access_flags),"evidence":match member {MethodOutput::Java {declaration,..}=>Some(json!({"environment_id":declaration.environment().0,"compilation_contract":format!("{:?}",declaration.compilation_contract())})),_=>None},"reason":reason,"source":source,"language":span.map(|span|language(span.language))})
        }).collect(),
    };
    let diagnostics: Vec<Value> = report.diagnostics().iter().map(diagnostic_value).collect();
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
    write_value(value, output)
}

fn failure(cause: DiagnosticCause, report: Option<&diana_decomp::Report>) -> Value {
    let diagnostic = diana_decomp::Diagnostic {
        method: None,
        cause,
    };
    let mut diagnostics = Vec::new();
    if diagnostic.cause.kind() != FailureKind::Cancelled
        && let Some(report) = report
    {
        for original in report.diagnostics() {
            if matches!(
                original.cause.kind(),
                FailureKind::Malformed | FailureKind::InternalInvariant | FailureKind::Cancelled
            ) {
                diagnostics.push(diagnostic_value(original));
            }
        }
    }
    diagnostics.push(diagnostic_value(&diagnostic));
    json!({"schema":"diana.inspect/1","status":"request_error","report":report.map(|_|json!({"available":true})),"published_success":false,"diagnostics":diagnostics})
}

fn diagnostic_value(diagnostic: &diana_decomp::Diagnostic) -> Value {
    let target = match &diagnostic.cause {
        DiagnosticCause::Request(RequestError::JavaTargetLimit {
            item,
            actual,
            maximum,
        }) => Some((
            match item {
                JavaTargetItem::ParameterWords => "parameter_words",
                JavaTargetItem::MethodName => "method_name",
                JavaTargetItem::MethodDescriptor => "method_descriptor",
            },
            actual,
            maximum,
        )),
        _ => None,
    };
    let control = match &diagnostic.cause {
        DiagnosticCause::Input(error) => input_control(error),
        DiagnosticCause::Method(error) => method_control(error),
        DiagnosticCause::Debug(error) => debug_control(error),
        DiagnosticCause::Request(error) => match error {
            RequestError::Control(error) => Some(error),
            RequestError::Input(error) => input_control(error),
            RequestError::Method(error) => method_control(error),
            RequestError::Debug(error) => debug_control(error),
            _ => None,
        },
        _ => None,
    };
    let limit = match control {
        Some(diana_decomp::ControlError::BudgetExceeded { limit, .. }) => Some(limit.to_string()),
        Some(diana_decomp::ControlError::AccountingOverflow { resource, .. }) => {
            Some(resource.to_string())
        }
        _ => None,
    };
    json!({"method_id_idx":diagnostic.method.map(|v|v.method.0),"kind":kind(diagnostic.cause.kind()),"code":format!("{:?}",diagnostic.code()),"severity":if diagnostic.cause.kind()==FailureKind::Unsupported {"warning"} else {"error"},"cause":format!("{:?}",diagnostic.cause),"target_item":target.map(|v|v.0),"actual":target.map(|v|v.1),"maximum":target.map(|v|v.2),"limit":limit})
}
fn input_control(error: &diana_decomp::input::InputError) -> Option<&diana_decomp::ControlError> {
    match error {
        diana_decomp::input::InputError::Control(error) => Some(error),
        _ => None,
    }
}
fn method_control(error: &diana_decomp::input::MethodError) -> Option<&diana_decomp::ControlError> {
    match error {
        diana_decomp::input::MethodError::Control(error) => Some(error),
        diana_decomp::input::MethodError::Input(error) => input_control(error),
        _ => None,
    }
}
fn debug_control(error: &diana_decomp::DebugError) -> Option<&diana_decomp::ControlError> {
    match error {
        diana_decomp::DebugError::Control(error) => Some(error),
        diana_decomp::DebugError::Input(error) => input_control(error),
        diana_decomp::DebugError::Method(error) => method_control(error),
        _ => None,
    }
}

fn write_value(value: Value, output: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
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
