//! Development shell over the native engine. It owns only a bare DEX path; the
//! library still borrows bytes and performs no filesystem or ZIP work.
//!
//! Non-interactive JSONL protocol:
//!   {"id":1,"cmd":"p0","class":0}
//!   {"id":2,"cmd":"p1","class":0,"method":0}
//! Output is one JSON object per line. `--exec` runs commands without a REPL.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use diana_decomp::input::ProgramIndex;
use diana_decomp::p1::DecodedItem;
use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, DexId, DexImage, Engine, FailureKind, ImageOrigin, InputId,
    MethodId, Options, Request, RequestError, RunContext,
};
use serde_json::{Value, json};

struct Shell {
    dex: PathBuf,
    bytes: Vec<u8>,
}

impl Shell {
    fn open(path: PathBuf) -> Result<Self, String> {
        let bytes = std::fs::read(&path).map_err(|error| format!("cannot read input: {error}"))?;
        if !bytes.starts_with(b"dex\n") {
            return Err("dbg_shell accepts a bare DEX, not an APK or ZIP".into());
        }
        Ok(Self { dex: path, bytes })
    }

    fn run(&mut self, request: &Value) -> Result<Value, RequestError> {
        let cancel = || false;
        let mut control = RunContext::new(&cancel);
        let index = ProgramIndex::open(&self.bytes, &mut control).map_err(RequestError::Input)?;
        let engine = Engine::open(
            vec![DexImage {
                bytes: &self.bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            &mut control,
        )
        .map_err(RequestError::Input)?;
        let command = request
            .get("cmd")
            .and_then(Value::as_str)
            .ok_or(RequestError::InternalInvariant("command has no cmd"))?;
        match command {
            "classes" => {
                let mut values = Vec::new();
                let mut class = 0u32;
                while let Ok(view) = index.class(ClassDefId(class), &mut control) {
                    if class >= 4096 {
                        break;
                    }
                    values.push(json!({
                        "class_def": class,
                        "descriptor": String::from_utf16_lossy(&view.descriptor.units(&mut control)?),
                    }));
                    class += 1;
                    if values.len() >= 64 {
                        break;
                    }
                }
                Ok(json!({"dex": self.dex, "classes": values}))
            }
            "class" => {
                let class = class_id(request)?;
                let facts = engine.class_facts(class_key(class), &mut control)?;
                let declared = facts.class_data_shape();
                let delivered = facts.delivered_shape();
                Ok(json!({
                    "class_def": class,
                    "descriptor": text(facts.descriptor()),
                    "fields": facts.fields().len(),
                    "methods": facts.methods().len(),
                    "class_data": {
                        "static_fields": declared.static_fields,
                        "instance_fields": declared.instance_fields,
                        "direct_methods": declared.direct_methods,
                        "virtual_methods": declared.virtual_methods,
                    },
                    "delivered": {
                        "static_fields": delivered.static_fields,
                        "instance_fields": delivered.instance_fields,
                        "direct_methods": delivered.direct_methods,
                        "virtual_methods": delivered.virtual_methods,
                    },
                    "terminated_at": facts.coverage_terminated_at().map(|value| value.0),
                }))
            }
            "p0" => {
                let class = class_id(request)?;
                let facts = engine.class_facts(class_key(class), &mut control)?;
                let declared = facts.class_data_shape();
                let delivered = facts.delivered_shape();
                let fields: Vec<Value> = facts
                    .fields()
                    .iter()
                    .map(|entry| {
                        json!({
                            "field": entry.field().0,
                            "access_flags": entry.original_flags().raw(),
                            "storage": format!("{:?}", entry.storage()),
                            "group_ordinal": entry.group_ordinal(),
                            "default_preparation": entry.default_preparation(),
                            "declaration": entry.declaration().is_present(),
                            "preparation": match entry.initial_value() {
                                diana_decomp::p0::Fact::Present(value) => match value {
                                    diana_decomp::p0::InitialValue::ImplicitDefault => "implicit_default",
                                    diana_decomp::p0::InitialValue::Encoded { .. } => "encoded",
                                },
                                diana_decomp::p0::Fact::Absent => "absent",
                                diana_decomp::p0::Fact::NotRead => "not_read",
                                diana_decomp::p0::Fact::Unavailable(_) => "unavailable",
                            },
                        })
                    })
                    .collect();
                let methods: Vec<Value> = facts
                    .methods()
                    .iter()
                    .map(|entry| {
                        json!({
                            "method": entry.method().0,
                            "group": format!("{:?}", entry.group()),
                            "access_flags": entry.original_flags().raw(),
                            "code_offset": entry.code_offset().map(|value| value.0),
                            "declaration": entry.declaration().is_present(),
                        })
                    })
                    .collect();
                Ok(json!({
                    "schema_version": 1,
                    "stage": "p0",
                    "class_def": class,
                    "descriptor": text(facts.descriptor()),
                    "class_data": {
                        "static_fields": declared.static_fields,
                        "instance_fields": declared.instance_fields,
                        "direct_methods": declared.direct_methods,
                        "virtual_methods": declared.virtual_methods,
                    },
                    "delivered": {
                        "static_fields": delivered.static_fields,
                        "instance_fields": delivered.instance_fields,
                        "direct_methods": delivered.direct_methods,
                        "virtual_methods": delivered.virtual_methods,
                    },
                    "terminated_at": facts.coverage_terminated_at().map(|value| value.0),
                    "fields": fields,
                    "methods": methods,
                }))
            }
            "methods" => {
                let class = class_id(request)?;
                let facts = engine.class_facts(class_key(class), &mut control)?;
                let values: Vec<Value> = facts
                    .methods()
                    .iter()
                    .map(|entry| {
                        json!({
                            "method": entry.method().0,
                            "group": format!("{:?}", entry.group()),
                            "access_flags": entry.original_flags().raw(),
                            "code_offset": entry.code_offset().map(|value| value.0),
                            "declaration": entry.declaration().is_present(),
                        })
                    })
                    .collect();
                Ok(json!({"class_def": class, "methods": values}))
            }
            "p1" | "decoded" => {
                let class = class_id(request)?;
                let method = method_id(request)?;
                let mut analysis = engine.analyze(
                    Request {
                        class: class_key(class),
                        method: None,
                    },
                    &mut control,
                )?;
                let Some(index) = analysis
                    .methods
                    .iter()
                    .position(|entry| entry.key.method == MethodId(method))
                else {
                    return Err(RequestError::InternalInvariant(
                        "method is not in the P0 directory",
                    ));
                };
                let decoded = analysis.methods.swap_remove(index).result?;
                let mut items = Vec::new();
                for item in decoded.items() {
                    items.push(match item {
                        DecodedItem::Instruction(instruction) => json!({
                            "pc": instruction.pc().0,
                            "kind": "instruction",
                            "opcode": instruction.spec().mnemonic,
                            "width": instruction.width().0,
                            "registers": instruction.registers().map(|value| value.0).collect::<Vec<_>>(),
                            "branch_target": instruction.branch_target().map(|value| value.0),
                            "payload_target": instruction.payload_target().map(|value| value.0),
                            "references": instruction.references().map(|reference| json!({
                                "kind": format!("{:?}", reference.kind),
                                "target": format!("{:?}", reference.target),
                                "slot": format!("{:?}", reference.slot),
                            })).collect::<Vec<_>>(),
                        }),
                        DecodedItem::Payload(payload) => json!({
                            "pc": payload.pc().0,
                            "kind": "payload",
                            "layout": format!("{:?}", payload.kind()),
                        }),
                    });
                }
                Ok(json!({
                    "class_def": class,
                    "method": method,
                    "instruction_bytes": [
                        decoded.instruction_bytes().start.0,
                        decoded.instruction_bytes().end.0,
                    ],
                    "items": items,
                    "try_ranges": decoded.try_boundaries().collect::<Vec<_>>(),
                    "handler_targets": decoded.handler_targets().collect::<Vec<_>>(),
                }))
            }
            "quit" | "exit" => Ok(json!({"bye": true})),
            other => Err(RequestError::InternalInvariant(match other {
                "load" => "load is not supported; pass --dex",
                _ => "unknown command",
            })),
        }
    }
}

fn class_key(class: u32) -> ClassKey {
    ClassKey {
        dex: DexId(0),
        class_def: ClassDefId(class),
    }
}

fn text(value: &diana_decomp::p0::OriginalText) -> String {
    String::from_utf16_lossy(value.units())
}

fn class_id(request: &Value) -> Result<u32, RequestError> {
    request
        .get("class")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(RequestError::InternalInvariant("missing class index"))
}

fn method_id(request: &Value) -> Result<u32, RequestError> {
    request
        .get("method")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(RequestError::InternalInvariant("missing method index"))
}

fn kind(error: &RequestError) -> &'static str {
    match error.kind() {
        FailureKind::Malformed => "malformed",
        FailureKind::Unsupported => "unsupported",
        FailureKind::ResourceExhausted => "resource_exhausted",
        FailureKind::Cancelled => "cancelled",
        FailureKind::InternalInvariant => "internal_invariant",
    }
}

fn response(id: Option<&Value>, result: Result<Value, RequestError>) -> Value {
    match result {
        Ok(value) => {
            let mut object = serde_json::Map::new();
            if let Some(id) = id {
                object.insert("id".into(), id.clone());
            }
            object.insert("ok".into(), Value::Bool(true));
            object.insert("result".into(), value);
            Value::Object(object)
        }
        Err(error) => {
            let mut object = serde_json::Map::new();
            if let Some(id) = id {
                object.insert("id".into(), id.clone());
            }
            object.insert("ok".into(), Value::Bool(false));
            object.insert(
                "error".into(),
                json!({"kind": kind(&error), "detail": error.to_string()}),
            );
            Value::Object(object)
        }
    }
}

fn command(line: &str) -> Result<Value, RequestError> {
    let trimmed = line.trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed)
            .map_err(|_| RequestError::InternalInvariant("invalid JSON command"));
    }
    let mut words = trimmed.split_whitespace();
    let cmd = words
        .next()
        .ok_or(RequestError::InternalInvariant("empty command"))?;
    let mut object = serde_json::Map::new();
    object.insert("cmd".into(), Value::String(cmd.to_owned()));
    let mut position = 0usize;
    for word in words {
        let name = if position == 0 { "class" } else { "method" };
        if let Ok(value) = word.parse::<u64>() {
            object.insert(name.into(), Value::Number(value.into()));
        }
        position += 1;
    }
    Ok(Value::Object(object))
}

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let mut dex = None;
    let mut exec = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dex" | "-d" => {
                let Some(path) = args.next() else {
                    eprintln!("dbg_shell: --dex needs a path");
                    return std::process::ExitCode::from(2);
                };
                dex = Some(PathBuf::from(path));
            }
            "--exec" | "-e" => {
                let Some(value) = args.next() else {
                    eprintln!("dbg_shell: --exec needs a command");
                    return std::process::ExitCode::from(2);
                };
                exec.push(value);
            }
            "--help" | "-h" => {
                eprintln!(
                    "usage: dbg_shell --dex <classes.dex> [--exec <json>]...\n\
                     stdin: one command per line, JSON or `p1 <class> <method>`"
                );
                return std::process::ExitCode::SUCCESS;
            }
            path if dex.is_none() => dex = Some(PathBuf::from(path)),
            other => {
                eprintln!("dbg_shell: unknown argument {other}");
                return std::process::ExitCode::from(2);
            }
        }
    }
    let Some(path) = dex else {
        eprintln!("dbg_shell: --dex is required");
        return std::process::ExitCode::from(2);
    };
    let mut shell = match Shell::open(path) {
        Ok(shell) => shell,
        Err(error) => {
            eprintln!("dbg_shell: {error}");
            return std::process::ExitCode::from(1);
        }
    };
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    if !exec.is_empty() {
        for line in exec {
            let request = match command(&line) {
                Ok(request) => request,
                Err(error) => {
                    let value = response(None, Err(error));
                    let _ = writeln!(stdout, "{value}");
                    continue;
                }
            };
            let id = request.get("id").cloned();
            let value = response(id.as_ref(), shell.run(&request));
            let _ = writeln!(stdout, "{value}");
        }
        return std::process::ExitCode::SUCCESS;
    }
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let request = match command(&line) {
            Ok(request) => request,
            Err(error) => {
                let value = response(None, Err(error));
                let _ = writeln!(stdout, "{value}");
                continue;
            }
        };
        let id = request.get("id").cloned();
        let value = response(id.as_ref(), shell.run(&request));
        if writeln!(stdout, "{value}").is_err() {
            break;
        }
        if request.get("cmd").and_then(Value::as_str) == Some("quit") {
            break;
        }
    }
    std::process::ExitCode::SUCCESS
}
