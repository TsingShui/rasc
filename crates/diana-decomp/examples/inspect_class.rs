use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DexId, DexImage, DiagnosticCause, Engine,
    EnvironmentId, FailureKind, ImageOrigin, InputId, JavaTargetItem, Limits, MethodOutput,
    MethodSelector, Options, Request, RequestError, RunContext, SourceLanguage, render,
};
use serde_json::{Value, json};
use std::cell::Cell;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments: Vec<_> = std::env::args().skip(1).collect();
    let mut output = None;
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
            // The flag is accepted for older callers; external declarations are
            // no longer read.
        } else if control_flag {
            control_path = Some(path);
        } else {
            output = Some(path);
        }
    }
    if !matches!(arguments.len(), 3 | 5) {
        return Err("usage: inspect_class <dex> <header-offset> <class-def-index> [<method-name> <full-proto>] [--catalog <declaration-json>] [--control <control-json>] [--output <json-path>]".into());
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
    #[derive(Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct Controls {
        limits: Limits,
        representation: diana_decomp::RepresentationPolicy,
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
        &mut control,
    ) {
        Ok(engine) => engine.with_representation(controls.representation),
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
    let mut diagnostics: Vec<Value> = report.diagnostics().iter().map(diagnostic_value).collect();
    let mut classes = vec![(report.class(), 0usize)];
    let mut nested_annotations = Vec::new();
    for child in report.nested() {
        let base = diagnostics.len();
        nested_annotations.push(annotation_facts(child.annotations(), base));
        if matches!(report.class(), ClassOutput::Mixed { .. }) {
            diagnostics.extend(child.diagnostics().iter().map(diagnostic_value));
        }
        classes.push((child.class(), base));
    }
    let members: Vec<Value> = classes.into_iter().flat_map(|(class, base)| class.members().iter().map(move |member| (class, member, base))).map(|(class, member, base)| {
            let owner = match class { ClassOutput::JavaUnit { descriptor, .. } | ClassOutput::Mixed { descriptor, .. } => descriptor };
            let key = member.key();
            let (status, facts, reason) = match member {
                MethodOutput::Generated { facts, .. } => ("generated", Some(facts), None),
                MethodOutput::Java { facts, .. } | MethodOutput::JavaInitializer { facts, .. } => ("java",Some(facts),None),
                MethodOutput::DeclarationOnly { facts, .. } => ("declaration_only", Some(facts), None),
                MethodOutput::Debug { facts, reason, .. } => ("debug", facts.as_ref(), Some(reason.0)),
                MethodOutput::Unavailable { facts, reason, .. } => ("unavailable", facts.as_ref(), Some(reason.0)),
            };
            let reason = reason.map(|index| base + index);
            let span = rendered.spans.iter().find(|span| span.method == key);
            let source = span.and_then(|span| rendered.source.get(span.start.0..span.end.0));
            json!({"class_def_idx":key.class.class_def.0,"owner_units":owner,"method_id_idx":key.method.0,"member_kind":if matches!(member,MethodOutput::JavaInitializer { .. }) {"initializer"} else {"method"},"status":status,"name_units":facts.map(|v| &v.name),"proto_units":facts.map(|v| &v.prototype),"access_flags":facts.map(|v| v.access_flags),"evidence":match member { MethodOutput::Java { declaration, .. } => Some(java_evidence(declaration.environment(), declaration.compilation_contract(), declaration.calls())), MethodOutput::JavaInitializer { declaration, .. } => Some(java_evidence(declaration.environment(), declaration.compilation_contract(), declaration.calls())), _ => None },"reason":reason,"source":source,"language":span.map(|span|language(span.language))})
        }).collect();
    let spans: Vec<Value> = rendered
        .spans
        .iter()
        .map(|span| {
            json!({
                "class_def_idx":span.method.class.class_def.0,"method_id_idx":span.method.method.0,"start":span.start.0,"end":span.end.0,
                "language":language(span.language),
            })
        })
        .collect();
    let (status, descriptor, access_flags) = match report.class() {
        ClassOutput::Mixed {
            descriptor,
            access_flags,
            ..
        } => ("mixed", descriptor, access_flags),
        ClassOutput::JavaUnit {
            descriptor,
            access_flags,
            ..
        } => ("java_unit", descriptor, access_flags),
    };
    let fields: Vec<Value> = report.class().fields().iter().map(|field| {
        let (status, source, reason) = match &field.declaration {
            diana_decomp::FieldDeclarationOutput::Java(declaration) => ("java", Some(declaration.source()), None),
            diana_decomp::FieldDeclarationOutput::Generated(_) => ("generated", None, None),
            diana_decomp::FieldDeclarationOutput::Unavailable(reason) => ("unavailable", None, Some(reason.0)),
        };
        let initial = match &field.initial {
            diana_decomp::FieldInitialValue::InstanceDefault => json!({"kind":"instance_default"}),
            diana_decomp::FieldInitialValue::ImplicitDefault => json!({"kind":"implicit_default"}),
            diana_decomp::FieldInitialValue::Encoded { tag, arg, range, bytes } => json!({"kind":"encoded","tag":tag,"value_arg":arg,"file_byte_start":range.start.0,"file_byte_end":range.end.0,"bytes":bytes}),
            diana_decomp::FieldInitialValue::Unavailable(reason) => json!({"kind":"unavailable","reason":reason.0}),
        };
        json!({"field_id_idx":field.key.field.0,"access_flags":field.access_flags,"static_ordinal":match field.storage {diana_decomp::FieldStorage::Static(ordinal)=>Some(ordinal.0),diana_decomp::FieldStorage::Instance=>None},"name_units":field.facts.as_ref().map(|facts|&facts.name),"type_units":field.facts.as_ref().map(|facts|&facts.descriptor),"initial":initial,"status":status,"source":source,"reason":reason})
    }).collect();
    let class_spans: Vec<Value> = rendered.class_spans.iter().map(|span| json!({"class_def_idx":span.class.class_def.0,"dex_id":span.class.dex.0,"start":span.start.0,"end":span.end.0})).collect();
    let annotations = annotation_facts(report.annotations(), 0);
    let value = json!({"schema":"diana.inspect/1","annotations":annotations,"nested_annotations":nested_annotations,"class_spans":class_spans,"fields":fields,"status":status,"header_offset":report.image().header_offset.0,"class_def_idx":class_def.0,"descriptor_units":descriptor,"access_flags":access_flags,"members":members,"diagnostics":diagnostics,"text":rendered.source,"spans":spans});
    write_value(value, output)
}

fn annotation_reference(reference: diana_decomp::raw::OperandReference) -> Value {
    use diana_decomp::raw::OperandReference as R;
    let (kind, index) = match reference {
        R::String(id) => ("string", id.0),
        R::Type(id) => ("type", id.0),
        R::Field(id) => ("field", id.0),
        R::Method(id) => ("method", id.0),
        R::Proto(id) => ("proto", id.0),
        R::CallSite(id) => ("call_site", id.0),
        R::MethodHandle(id) => ("method_handle", id.0),
    };
    json!({"kind":kind,"index":index})
}
fn annotation_facts(annotations: &diana_decomp::ClassAnnotations, base: usize) -> Value {
    use diana_decomp::{
        AnnotationLocation as L, AnnotationStatus as S, AnnotationToken as T,
        AnnotationValueKind as K,
    };
    let (status, reason) = match annotations.status() {
        S::Complete => ("complete", None),
        S::Partial(id) => ("partial", Some(base + id.0)),
        S::Unavailable(id) => ("unavailable", Some(base + id.0)),
    };
    let original = annotations.original();
    let records: Vec<_> = original.records().iter().map(|record| {
        let location = match record.location() { L::Class => json!({"kind":"class"}), L::Field(id) => json!({"kind":"field","field_id_idx":id.0}), L::Method(id) => json!({"kind":"method","method_id_idx":id.0}), L::Parameter { method, parameter } => json!({"kind":"parameter","method_id_idx":method.0,"parameter_index":parameter.0}) };
        let values: Vec<_> = record.tokens().iter().map(|token| match token {
            T::EndContainer => json!({"kind":"end_container"}),
            T::Value(value) => {
                let kind = match value.kind() {
                    K::Signed(value) => json!({"kind":"signed","value":value.to_string()}), K::Unsigned(value) => json!({"kind":"unsigned","value":value.to_string()}),
                    K::Bits { width, bits } => json!({"kind":"bits","width":width.0,"bits":format!("{bits:016x}")}),
                    K::String { reference, units } => json!({"kind":"string","string_id_idx":reference.0,"units":units}),
                    K::Reference(reference) => json!({"kind":"reference","reference":annotation_reference(*reference)}),
                    K::Null => json!({"kind":"null"}), K::Boolean(value) => json!({"kind":"boolean","value":value}),
                    K::Array { count } => json!({"kind":"array","count":count}), K::Annotation { type_id, count } => json!({"kind":"annotation","type_id_idx":type_id.0,"count":count}),
                };
                json!({"kind":"value","tag":value.tag(),"value_arg":value.value_arg(),"name_id_idx":value.name_id().map(|id|id.0),"name_units":value.name(),"file_byte_start":value.range().start.0,"file_byte_end":value.range().end.0,"value":kind})
            }
        }).collect();
        json!({"location":location,"visibility":record.visibility(),"type_id_idx":record.type_id().0,"descriptor_units":record.descriptor(),"extent":format!("{:?}",record.extent()),"file_byte_start":record.range().start.0,"file_byte_end":record.range().end.0,"bytes":record.bytes(),"values":values})
    }).collect();
    json!({"status":status,"reason":reason,"dex_id":original.owner().dex.0,"class_def_idx":original.owner().class_def.0,"directory_offset":original.directory().0,"records":records,"unavailable_prefix":original.unavailable_prefix().map(|prefix|json!({"file_byte_start":prefix.range().start.0,"file_byte_end":prefix.range().end.0,"bytes":prefix.bytes()}))})
}

fn java_evidence<'a>(
    environment: EnvironmentId,
    contract: diana_decomp::CompilationContract,
    calls: impl Iterator<Item = diana_decomp::JavaCallSite<'a>>,
) -> Value {
    json!({"environment_id":environment.0,"compilation_contract":format!("{contract:?}"),"calls":calls.map(|call|json!({"reference_method_id_idx":call.reference.0,"receiver_register":call.receiver.map(|register|register.0),"allocation_pc":call.allocation.map(|pc|pc.0),"owner_units":call.owner,"name_units":call.name,"proto_units":call.prototype,"invoke_pc":call.invoke.0,"result_pc":call.result.map(|pc|pc.0),"kind":format!("{:?}",call.kind)})).collect::<Vec<_>>()})
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
                JavaTargetItem::ConstantUtf8Bytes => "constant_utf8_bytes",
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
        let mut writer = std::io::BufWriter::new(std::fs::File::create(path)?);
        serde_json::to_writer(&mut writer, &value)?;
        std::io::Write::flush(&mut writer)?;
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
