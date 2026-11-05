//! Development shell over the native engine. It owns only a bare DEX path; the
//! library still borrows bytes and performs no filesystem or ZIP work.
//!
//! Non-interactive JSONL protocol:
//!   {"id":1,"cmd":"p0","class":0}
//!   {"id":2,"cmd":"p1","class":0,"method":0}
//! Output is one JSON object per line. `--exec` runs commands without a REPL.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use diana_decomp::input::ProgramIndex;
use diana_decomp::p1::{DecodedInstruction, DecodedItem, DecodedPayload};
use diana_decomp::raw::{Format as DexFormat, RawLiteral, ReferenceSlot};
use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, DexId, DexImage, Engine, FailureKind, ImageOrigin, InputId,
    MethodId, MethodOutcome, Request, RequestError,
};
use serde_json::{Value, json};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Format {
    Json,
    Text,
}

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
        let index = ProgramIndex::open(&self.bytes).map_err(RequestError::Input)?;
        let engine = Engine::open(vec![DexImage {
            bytes: &self.bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }])
        .map_err(RequestError::Input)?;
        let command = request
            .get("cmd")
            .and_then(Value::as_str)
            .ok_or(RequestError::InternalInvariant("command has no cmd"))?;
        match command {
            "classes" => {
                let mut values = Vec::new();
                let mut class = 0u32;
                while let Ok(view) = index.class(ClassDefId(class)) {
                    if class >= 4096 {
                        break;
                    }
                    values.push(json!({
                        "class_def": class,
                        "descriptor": String::from_utf16_lossy(&view.descriptor.units()?),
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
                let facts = engine.class_facts(class_key(class))?;
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
                let facts = engine.class_facts(class_key(class))?;
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
                let facts = engine.class_facts(class_key(class))?;
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
            "roundtrip-all" => {
                let started = std::time::Instant::now();
                let classes = index.class_count();
                let mut methods = 0u64;
                let mut checked = 0u64;
                let mut skipped = 0u64;
                let mut mismatches = 0u64;
                let mut class_errors = 0u64;
                let mut method_errors = 0u64;
                let mut no_code = 0u64;
                let mut failures = Vec::new();
                let mut skipped_formats: BTreeMap<String, u64> = BTreeMap::new();
                for class in 0..classes {
                    let analysis = match engine.analyze(Request {
                        class: class_key(class),
                        method: None,
                    }) {
                        Ok(analysis) => analysis,

                        Err(_) => {
                            class_errors += 1;
                            continue;
                        }
                    };
                    for entry in &analysis.methods {
                        methods += 1;
                        let method = entry.key.method;
                        let outcome = match &entry.result {
                            Ok(outcome) => outcome,
                            Err(_) => {
                                method_errors += 1;
                                continue;
                            }
                        };
                        if matches!(outcome, MethodOutcome::DeclarationOnly) {
                            no_code += 1;
                            continue;
                        }
                        let Some(decoded) = outcome.evidence() else {
                            method_errors += 1;
                            continue;
                        };
                        let start = decoded.instruction_bytes().start.0;
                        for item in decoded.items() {
                            let DecodedItem::Instruction(instruction) = item else {
                                let DecodedItem::Payload(payload) = item else {
                                    unreachable!()
                                };
                                let Some(encoded) = encode_payload(payload, decoded.source())
                                else {
                                    skipped += 1;
                                    *skipped_formats.entry("payload".to_owned()).or_default() += 1;
                                    continue;
                                };
                                let Ok(pc) = usize::try_from(payload.pc().0) else {
                                    skipped += 1;
                                    continue;
                                };
                                let offset = start + pc * 2;
                                let Some(raw) = self.bytes.get(offset..offset + encoded.len())
                                else {
                                    mismatches += 1;
                                    continue;
                                };
                                checked += 1;
                                if raw != encoded.as_slice() {
                                    mismatches += 1;
                                    if failures.len() < 100 {
                                        failures.push(json!({
                                            "class": class,
                                            "method": method.0,
                                            "pc": payload.pc().0,
                                            "kind": "payload",
                                            "format": format!("{:?}", payload.kind()),
                                            "expected": encoded,
                                            "actual": raw,
                                        }));
                                    }
                                }
                                continue;
                            };
                            let Some(encoded) = encode(instruction) else {
                                skipped += 1;
                                *skipped_formats
                                    .entry(format!("{:?}", instruction.spec().format))
                                    .or_default() += 1;
                                continue;
                            };
                            let Ok(pc) = usize::try_from(instruction.pc().0) else {
                                skipped += 1;
                                continue;
                            };
                            let offset = start + pc * 2;
                            let Some(raw) = self.bytes.get(offset..offset + encoded.len() * 2)
                            else {
                                mismatches += 1;
                                if failures.len() < 100 {
                                    failures.push(json!({
                                        "class": class,
                                        "method": method.0,
                                        "pc": instruction.pc().0,
                                        "error": "raw range",
                                    }));
                                }
                                continue;
                            };
                            let actual: Vec<u16> = raw
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|pair| u16::from_le_bytes(*pair))
                                .collect();
                            checked += 1;
                            if actual != encoded {
                                mismatches += 1;
                                if failures.len() < 100 {
                                    failures.push(json!({
                                        "class": class,
                                        "method": method.0,
                                        "pc": instruction.pc().0,
                                        "opcode": instruction.spec().mnemonic,
                                        "format": format!("{:?}", instruction.spec().format),
                                        "expected": encoded,
                                        "actual": actual,
                                    }));
                                }
                            }
                        }
                    }
                }
                Ok(json!({
                    "classes": classes,
                    "methods": methods,
                    "checked": checked,
                    "skipped": skipped,
                    "mismatches": mismatches,
                    "class_errors": class_errors,
                    "method_errors": method_errors,
                    "no_code": no_code,
                    "skipped_formats": skipped_formats,
                    "failures": failures,
                    "failures_truncated": mismatches > u64::try_from(failures.len()).unwrap_or(u64::MAX),
                    "elapsed_ms": started.elapsed().as_millis(),
                }))
            }
            "roundtrip" => {
                let class = class_id(request)?;
                let method = method_id(request)?;
                let analysis = engine.analyze_method(class_key(class), MethodId(method))?;
                let outcome = analysis.result?;
                if let Some(response) = inspection_stop(&outcome) {
                    return Ok(response);
                }
                let decoded = outcome
                    .evidence()
                    .ok_or(RequestError::InternalInvariant("method has no evidence"))?;
                let base = decoded.instruction_bytes().start.0;
                let mut checked = 0usize;
                let mut skipped = Vec::new();
                let mut mismatches = Vec::new();
                for item in decoded.items() {
                    let DecodedItem::Instruction(instruction) = item else {
                        skipped.push(json!({"pc": item.pc().0, "kind": "payload"}));
                        continue;
                    };
                    let Some(encoded) = encode(instruction) else {
                        skipped.push(json!({
                            "pc": instruction.pc().0,
                            "opcode": instruction.spec().mnemonic,
                            "format": format!("{:?}", instruction.spec().format),
                        }));
                        continue;
                    };
                    let Ok(pc) = usize::try_from(instruction.pc().0) else {
                        continue;
                    };
                    let offset = base + pc * 2;
                    let Some(raw) = self.bytes.get(offset..offset + encoded.len() * 2) else {
                        mismatches.push(json!({"pc": instruction.pc().0, "error": "raw range"}));
                        continue;
                    };
                    let actual: Vec<u16> = raw
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| u16::from_le_bytes(*pair))
                        .collect();
                    checked += 1;
                    if actual != encoded {
                        mismatches.push(json!({
                            "pc": instruction.pc().0,
                            "opcode": instruction.spec().mnemonic,
                            "format": format!("{:?}", instruction.spec().format),
                            "expected": encoded,
                            "actual": actual,
                        }));
                    }
                }
                Ok(json!({
                    "class_def": class,
                    "method": method,
                    "checked": checked,
                    "skipped": skipped,
                    "mismatches": mismatches,
                }))
            }
            "p1" | "decoded" => {
                let class = class_id(request)?;
                let method = method_id(request)?;
                let analysis = engine.analyze_method(class_key(class), MethodId(method))?;
                let outcome = analysis.result?;
                if let Some(response) = inspection_stop(&outcome) {
                    return Ok(response);
                }
                let decoded = outcome
                    .evidence()
                    .ok_or(RequestError::InternalInvariant("method has no evidence"))?;
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
                            "result_pc": instruction.result_pc().map(|value| value.0),
                            "producer_pc": instruction.producer_pc().map(|value| value.0),
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
                let mut catch_targets = Vec::new();
                for boundary in decoded.try_boundaries().collect::<Vec<_>>() {
                    let clauses =
                        decoded.catch_clauses_at(diana_decomp::CodeUnitOffset(boundary))?;
                    if !clauses.is_empty() {
                        catch_targets.push(json!({"try_start": boundary, "clauses": clauses}));
                    }
                }
                let entry = decoded.entry();
                let slot_json = |slot: diana_decomp::RegisterSlot| json!({"start": slot.start.0, "width": format!("{:?}", slot.width)});
                let parameters = entry
                    .parameters()
                    .iter()
                    .map(|parameter| {
                        json!({
                            "type_id": parameter.type_id().0,
                            "slot": slot_json(parameter.slot()),
                        })
                    })
                    .collect::<Vec<_>>();
                let reference_facts = decoded
                    .reference_facts()
                    .map(|(pc, slot, fact)| {
                        json!({
                            "pc": pc.0,
                            "slot": format!("{slot:?}"),
                            "fact": format!("{fact:?}"),
                            "use_issue": decoded.reference_use_issue(pc, slot).map(|e|format!("{e}")),
                        })
                    })
                    .collect::<Vec<_>>();
                let invokes = decoded
                    .invoke_layouts()
                    .map(|(pc, layout)| {
                        let layout = match layout {
                            Ok(layout) => layout,
                            Err(error) => return json!({"pc":pc.0,"available":false,"error":format!("{error}")}),
                        };
                        json!({
                            "pc": pc.0,
                            "receiver": layout.has_receiver(),
                            "arguments": layout.arguments().iter()
                                .map(|width| format!("{width:?}"))
                                .collect::<Vec<_>>(),
                            "result": layout.result().map(|width| format!("{width:?}")),
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(json!({
                    "class_def": class,
                    "method": method,
                    "catch_targets": catch_targets,
                    "catch_types":catch_types_json(decoded.source().catch_types()),
                    "class_id": decoded.class_id().0,
                    "access_flags": decoded.access_flags(),
                    "annotations": {
                        "method": decoded.annotations().method_range().map(|range| format!("{range:?}")),
                        "parameters": decoded.annotations().parameter_range().map(|range| format!("{range:?}")),
                    },
                    "metadata_issue": decoded.metadata_issue(),
                    "declaration": {
                        "name": decoded.declaration().name.to_string(),
                        "prototype": decoded.declaration().prototype.to_string(),
                        "parameter_type_ids": decoded.declaration().parameter_type_ids,
                    },
                    "entry": {
                        "incoming_start": entry.incoming_start().0,
                        "this": entry.this().map(slot_json),
                        "parameters": parameters,
                    },
                    "invokes": invokes,
                    "reference_facts": reference_facts,
                    "issues": decoded.issues().map(|(pc,e)|json!({"pc":pc.0,"kind":format!("{:?}",e.kind()),"detail":format!("{e}")})).collect::<Vec<_>>(),
                    "instruction_bytes": [
                        decoded.instruction_bytes().start.0,
                        decoded.instruction_bytes().end.0,
                    ],
                    "items": items,
                    "try_ranges": decoded.try_boundaries().collect::<Vec<_>>(),
                    "handler_targets": decoded.handler_targets().collect::<Vec<_>>(),
                }))
            }
            "p3-all" | "p4-all" => ssa_all(
                &engine,
                index.class_count(),
                request
                    .get("class")
                    .map(|_| class_id(request))
                    .transpose()?,
            ),
            "p4" | "types" => {
                let analysis = engine
                    .analyze_method(class_key(class_id(request)?), MethodId(method_id(request)?))?;
                let outcome = analysis.result?;
                let details = request
                    .get("details")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match &outcome {
                    MethodOutcome::Typed(typed) => Ok(type_json(typed, details)),
                    MethodOutcome::SsaStopped(stop) => Ok(
                        json!({"stage":"p4","upstream_stage":"p3","kind":format!("{:?}",stop.error().kind()),"error":format!("{}",stop.error())}),
                    ),
                    MethodOutcome::TypeStopped(stop) => Ok(
                        json!({"stage":"p4","stopped":true,"kind":format!("{:?}",stop.error().kind()),"error":format!("{}",stop.error()),"partial":{"values":stop.partial().values().len(),"uses":stop.partial().uses().len(),"issues":stop.partial().issues().len()}}),
                    ),
                    _ => Ok(inspection_stop(&outcome)
                        .unwrap_or(json!({"stage":"p4","upstream_stopped":true}))),
                }
            }
            "p3" | "ssa" => {
                let analysis = engine
                    .analyze_method(class_key(class_id(request)?), MethodId(method_id(request)?))?;
                let outcome = analysis.result?;
                match &outcome {
                    MethodOutcome::Typed(ssa) => Ok(ssa_json(
                        ssa.ssa_facts(),
                        ssa.execution(),
                        request
                            .get("details")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    )),
                    MethodOutcome::TypeStopped(stop) => Ok(ssa_json(
                        stop.ssa_facts(),
                        stop.execution(),
                        request
                            .get("details")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    )),
                    MethodOutcome::SsaStopped(stop) => Ok(json!({"stage":"p3", "stopped":true,
                        "kind":format!("{:?}",stop.error().kind()), "error":format!("{}",stop.error()),
                        "partial":{"points":stop.partial().points().len(),"states":stop.partial().states().len(),
                            "uses":stop.partial().uses().len(),"phis":stop.partial().phis().len()}})),
                    _ => Ok(inspection_stop(&outcome)
                        .unwrap_or(json!({"stage":"p3","declaration_only":true}))),
                }
            }
            "p2-all" => {
                let started = std::time::Instant::now();
                let classes = index.class_count();
                let mut methods = 0u64;
                let mut no_code = 0u64;
                let mut class_errors = 0u64;
                let mut p1_errors = 0u64;
                let mut flowed = 0u64;
                let mut failed = 0u64;
                let mut internal = 0u64;
                let mut unsupported = 0u64;
                let mut malformed = 0u64;
                let mut resource = 0u64;
                let mut blocks = 0u64;
                let mut operations = 0u64;
                let mut edges = 0u64;
                let mut exits = 0u64;
                let mut normal_edges = 0u64;
                let mut exception_edges = 0u64;
                let mut covered_operations = 0u64;
                let mut merged_results = 0u64;
                let mut terminals = 0u64;
                let mut payloads = 0u64;
                let mut unreachable = 0u64;
                let mut replay_opcodes = BTreeMap::<String, u64>::new();
                let mut failures = Vec::new();
                for class in 0..classes {
                    let analysis = match engine.analyze(Request {
                        class: class_key(class),
                        method: None,
                    }) {
                        Ok(analysis) => analysis,

                        Err(_) => {
                            class_errors += 1;
                            continue;
                        }
                    };
                    for entry in &analysis.methods {
                        methods += 1;
                        let method = entry.key.method;
                        let outcome = match &entry.result {
                            Ok(outcome) => outcome,
                            Err(_) => {
                                p1_errors += 1;
                                continue;
                            }
                        };
                        let result = match outcome {
                            MethodOutcome::DeclarationOnly => {
                                no_code += 1;
                                continue;
                            }
                            MethodOutcome::DisassemblyStopped(_) => {
                                p1_errors += 1;
                                continue;
                            }
                            MethodOutcome::Typed(ssa) => Ok(ssa.execution()),
                            MethodOutcome::TypeStopped(stop) => Ok(stop.execution()),
                            MethodOutcome::SsaStopped(stop) => Ok(stop.execution()),
                            MethodOutcome::FlowStopped(stop) => Err(stop.error()),
                        };
                        match result {
                            Ok(flow) => {
                                flowed += 1;
                                for operation in flow.operations() {
                                    let original = outcome
                                        .evidence()
                                        .ok_or(RequestError::InternalInvariant(
                                            "P2 evidence missing",
                                        ))?
                                        .instruction_at(operation.pc())
                                        .ok_or(RequestError::InternalInvariant(
                                            "replayed operation lost its source",
                                        ))?;
                                    *replay_opcodes
                                        .entry(original.spec().mnemonic.to_owned())
                                        .or_default() += 1;
                                }
                                blocks += u64::try_from(flow.blocks().len()).unwrap_or(u64::MAX);
                                operations +=
                                    u64::try_from(flow.operations().len()).unwrap_or(u64::MAX);
                                edges += u64::try_from(flow.edges().len()).unwrap_or(u64::MAX);
                                exits += u64::try_from(flow.exits().len()).unwrap_or(u64::MAX);
                                for edge in flow.edges() {
                                    match edge.role() {
                                        diana_decomp::p2::EdgeRole::Normal(_) => {
                                            normal_edges += 1;
                                        }
                                        diana_decomp::p2::EdgeRole::Exception { .. } => {
                                            exception_edges += 1;
                                        }
                                    }
                                }
                                for coverage in flow.coverage() {
                                    match coverage {
                                        diana_decomp::p2::Coverage::Operation(_) => {
                                            covered_operations += 1;
                                        }
                                        diana_decomp::p2::Coverage::MergedResult(_) => {
                                            merged_results += 1;
                                        }
                                        diana_decomp::p2::Coverage::Terminal(_) => terminals += 1,
                                        diana_decomp::p2::Coverage::Payload => payloads += 1,
                                        diana_decomp::p2::Coverage::Unreachable => unreachable += 1,
                                    }
                                }
                            }
                            Err(error) => {
                                failed += 1;
                                match error.kind() {
                                    FailureKind::InternalInvariant => internal += 1,
                                    FailureKind::Unsupported => unsupported += 1,
                                    FailureKind::Malformed => malformed += 1,
                                    FailureKind::ResourceExhausted => resource += 1,
                                }
                                if failures.len() < 100 {
                                    failures.push(json!({
                                        "class": class,
                                        "method": method.0,
                                        "kind": format!("{:?}", error.kind()),
                                        "detail": format!("{error}"),
                                    }));
                                }
                            }
                        }
                    }
                }
                Ok(json!({
                    "classes": classes,
                    "methods": methods,
                    "no_code": no_code,
                    "class_errors": class_errors,
                    "p1_errors": p1_errors,
                    "flowed": flowed,
                    "failed": failed,
                    "internal": internal,
                    "unsupported": unsupported,
                    "malformed": malformed,
                    "resource": resource,
                    "blocks": blocks,
                    "operations": operations,
                    "edges": edges,
                    "normal_edges": normal_edges,
                    "exception_edges": exception_edges,
                    "exits": exits,
                    "covered_operations": covered_operations,
                    "merged_results": merged_results,
                    "terminals": terminals,
                    "payloads": payloads,
                    "unreachable": unreachable,
                    "replay_opcodes": replay_opcodes,
                    "failures": failures,
                    "failures_truncated": failed > u64::try_from(failures.len()).unwrap_or(u64::MAX),
                    "elapsed_ms": started.elapsed().as_millis(),
                }))
            }
            "p2" => {
                let class = class_id(request)?;
                let method = method_id(request)?;
                let analysis = engine.analyze_method(class_key(class), MethodId(method))?;
                let outcome = analysis.result?;
                if let Some(response) = inspection_stop(&outcome) {
                    return Ok(response);
                }
                let flow = outcome
                    .execution()
                    .ok_or(RequestError::InternalInvariant("method has no flow"))?;
                let details = if request.get("details").and_then(Value::as_bool) == Some(true) {
                    Some(json!({
                        "operations": flow.operations().iter().map(|operation| json!({
                            "id": operation.id().0,
                            "pc": operation.pc().0,
                            "block": operation.block().0,
                            "kind": format!("{:?}", operation.kind()),
                            "reads": flow.reads(operation).ok().map(|reads| reads.iter().map(|slot| json!({
                                "start": slot.start.0, "words": slot.words(),
                            })).collect::<Vec<_>>()),
                            "reads_issue": flow.reads(operation).err().map(|e|format!("{e}")),
                            "input_words": flow.input_words(operation).map(|w|w.map(|r|r.0).collect::<Vec<_>>()),
                            "writes": flow.writes(operation).iter().map(|slot| json!({
                                "start": slot.start.0, "words": slot.words(),
                            })).collect::<Vec<_>>(),
                            "effects": format!("{:?}", operation.effects()),
                            "commit": format!("{:?}", operation.normal().commit()),
                            "normal": format!("{:?}", operation.normal()),
                            "exceptional": operation.exceptional().map(|completion| format!("{completion:?}")),
                            "result_pc": operation.result_pc().map(|pc| pc.0),
                        })).collect::<Vec<_>>(),
                        "edges": flow.edges().iter().map(|edge| json!({
                            "id": edge.id().0, "from": edge.from().0,
                            "source_op": edge.source_op().map(|id| id.0),
                            "role": format!("{:?}", edge.role()),
                            "target": format!("{:?}", edge.target()),
                        })).collect::<Vec<_>>(),
                    }))
                } else {
                    None
                };
                let mut normal_edges = 0usize;
                let mut exception_edges = 0usize;
                for edge in flow.edges() {
                    match edge.role() {
                        diana_decomp::p2::EdgeRole::Normal(_) => normal_edges += 1,
                        diana_decomp::p2::EdgeRole::Exception { .. } => exception_edges += 1,
                    }
                }
                let mut coverage = std::collections::BTreeMap::new();
                for value in flow.coverage() {
                    let name = match value {
                        diana_decomp::p2::Coverage::Operation(_) => "operation",
                        diana_decomp::p2::Coverage::Terminal(_) => "terminal",
                        diana_decomp::p2::Coverage::MergedResult(_) => "merged_result",
                        diana_decomp::p2::Coverage::Payload => "payload",
                        diana_decomp::p2::Coverage::Unreachable => "unreachable",
                    };
                    *coverage.entry(name).or_insert(0usize) += 1;
                }
                Ok(json!({
                    "class_def": class,
                    "method": method,
                    "decoded": true,
                    "blocks": flow.blocks().len(),
                    "operations": flow.operations().len(),
                    "details": details,
                    "issues": flow.issues().map(|(pc,e)|json!({"pc":pc.0,"kind":format!("{:?}",e.kind()),"detail":format!("{e}")})).collect::<Vec<_>>(),
                    "edges": flow.edges().len(),
                    "normal_edges": normal_edges,
                    "exception_edges": exception_edges,
                    "exits": flow.exits().len(),
                    "coverage": coverage,
                    "entry": {
                        "incoming_start": flow.entry().incoming_start().0,
                        "this": flow.entry().this().map(|slot| json!({"start": slot.start.0, "width": format!("{:?}", slot.width)})),
                        "parameters": flow.entry().parameters().len(),
                    },
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

fn catch_types_json(facts: &[diana_decomp::input::CatchTypeFact<'_>]) -> Value {
    let error = |e: &diana_decomp::input::InputError| json!({"kind":format!("{:?}",e.kind()),"detail":format!("{e}")});
    json!(facts.iter().map(|fact|json!({
        "type_id":fact.type_id().0,
        "descriptor":match fact.descriptor() {
            Ok(descriptor)=>match descriptor.units() {Ok(units)=>json!({"units":units}),Err(e)=>json!({"error":error(&e)})},
            Err(e)=>json!({"error":error(e)}),
        },
        "use_issue":fact.use_issue().map(error),
    })).collect::<Vec<_>>())
}

fn type_json(typed: &diana_decomp::p4::TypedMethod<'_>, details: bool) -> Value {
    let f = typed.facts();
    let flow = typed.execution();
    use diana_decomp::p4::{Interpretation, Problem};
    let mut missing = std::collections::BTreeMap::<String, u64>::new();
    for issue in f.issues() {
        if let Problem::Missing(fact) = issue.problem {
            *missing.entry(format!("{fact:?}")).or_default() += 1;
        }
    }
    json!({"stage":"p4","class":typed.class_id().0,"method":typed.method_id().0,
        "input_issues":typed.execution().issues().map(|(pc,e)|json!({"pc":pc.0,"kind":format!("{:?}",e.kind()),"detail":format!("{e}")})).collect::<Vec<_>>(),
        "values":f.values().len(),"uses":f.uses().len(),"phis":f.phis().len(),
        "constructors":f.constructors().len(),"initialization_uses":f.initialization().len(),"array_fills":f.array_fills().len(),
        "catch_types":catch_types_json(flow.catch_types()),
        "issues":f.issues().len(),"missing_facts":missing,
        "unknown_views":f.uses().iter().filter(|u|matches!(u.interpretation(),Interpretation::Unknown(_))).count(),
        "conflict_values":f.values().iter().filter(|v|v.families().is_conflict()).count(),
        "details":details.then(||json!({
            "names":f.names().iter().enumerate().map(|(i,n)|json!({"id":i,"units":n.units(),"origin":format!("{:?}",n.origin())})).collect::<Vec<_>>(),
            "values":f.values().iter().map(|v|json!({"value":v.value().0,"families":format!("{:?}",v.families()),"types":v.types().iter().map(|n|n.0).collect::<Vec<_>>(),
                "unknown_type_source":v.has_unknown_type_source(),"literal":v.literal().map(|b|json!({"bits":b.value,"width":format!("{:?}",b.width)})),
                "range":v.integral_range().map(|r|json!([r.min,r.max])),"identity":format!("{:?}",v.identity())})).collect::<Vec<_>>(),
            "uses":f.uses().iter().map(|u|json!({"site":format!("{:?}",u.site()),"state":u.state().0,"required":format!("{:?}",u.requirement().families()),
                "types":u.requirement().types().iter().map(|n|n.0).collect::<Vec<_>>(),"view":format!("{:?}",u.interpretation())})).collect::<Vec<_>>(),
            "phis":f.phis().iter().map(|p|json!({"phi":p.phi().0,"representation":format!("{:?}",p.representation()),
                "incoming":p.incoming().iter().map(|i|json!({"transfer":i.transfer.0,"state":i.state.0,"view":format!("{:?}",i.interpretation)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "array_fills":f.array_fills().iter().map(|v|json!({"operation":v.operation.0,"array":v.array.0,"element":v.element.map(|e|format!("{e:?}")).ok(),"problem":v.element.err().map(|e|format!("{e:?}")),"payload":match flow.array_payload(v.operation) {
                Ok(Some(p))=>json!({"pc":p.pc().0,"element_width":p.element_width().0,"element_count":p.element_count(),"bits":p.elements().map(|e|e.bits).collect::<Vec<_>>()}),
                Ok(None)=>Value::Null,Err(e)=>json!({"kind":format!("{:?}",e.kind()),"detail":format!("{e}")}),
            }})).collect::<Vec<_>>(),
            "constructors":f.constructors().iter().map(|c|json!({"operation":c.operation.0,"receiver":c.receiver.map(|s|s.0),"identity":format!("{:?}",c.identity),"relation":format!("{:?}",c.relation)})).collect::<Vec<_>>(),
            "initialization":f.initialization().iter().map(|i|json!({"site":format!("{:?}",i.site),"point":i.point.0,"state":format!("{:?}",i.state)})).collect::<Vec<_>>(),
            "issues":f.issues().iter().map(|i|json!({"site":format!("{:?}",i.site),"problem":format!("{:?}",i.problem)})).collect::<Vec<_>>()
        }))
    })
}

fn ssa_json(
    f: &diana_decomp::p3::SsaFacts,
    flow: diana_decomp::p2::ExecutionFacts<'_, '_>,
    details: bool,
) -> Value {
    json!({"stage":"p3", "class":flow.class_id().0,"method":flow.method_id().0,
    "catch_types":catch_types_json(flow.catch_types()),
    "issues":flow.issues().map(|(pc,e)|json!({"pc":pc.0,"kind":format!("{:?}",e.kind()),"detail":format!("{e}")})).collect::<Vec<_>>(),
    "unresolved_input_sites":f.operation_sites().iter().filter(|s|!s.inputs_available()).count(),
    "points":f.points().len(),"transfers":f.transfers().len(),"slots":f.slots().len(),
    "states":f.states().len(),"values":f.values().len(),"uses":f.uses().len(),"phis":f.phis().len(),
    "unavailable_uses":f.uses().iter().filter(|u|!f.state(u.state()).is_some_and(|s|s.availability().is_available())).count(),
    "details":details.then(||json!({
        "operation_sites":f.operation_sites().iter().map(|s|json!({"operation":s.operation().0,"execute":s.execute().0,"commit":s.commit().map(|p|p.0),
            "commit_rule":format!("{:?}",s.commit_rule()),"commit_pc":s.commit_pc().map(|p|p.0),"inputs_available":s.inputs_available(),"uses":[s.use_start(),s.use_len()],"outputs":[s.output_start(),s.output_len()]})).collect::<Vec<_>>(),
        "slots":f.slots().iter().map(|s|json!({"start":s.start.0,"width":format!("{:?}",s.width)})).collect::<Vec<_>>(),
        "points":f.points().iter().map(|p|json!({"id":p.id().0,"kind":format!("{:?}",p.kind()),"idom":p.idom().map(|p|p.0)})).collect::<Vec<_>>(),
        "transfers":f.transfers().iter().map(|t|json!({"id":t.id().0,"from":t.from().0,"target":format!("{:?}",t.target()),"origin":format!("{:?}",t.origin())})).collect::<Vec<_>>(),
        "states":f.states().iter().map(|s|json!({"id":s.id().0,"slot":s.slot().0,"point":s.point().0,"kind":format!("{:?}",s.kind()),"availability":format!("{:?}",s.availability())})).collect::<Vec<_>>(),
        "values":f.values().iter().map(|v|json!({"id":v.id().0,"slot":v.slot().0,"point":v.point().0,"origin":format!("{:?}",v.origin())})).collect::<Vec<_>>(),
        "uses":f.uses().iter().map(|u|json!({"site":format!("{:?}",u.site()),"point":u.point().0,"slot":u.slot().0,"state":u.state().0,"value":f.readable_value(u).map(|v|v.0)})).collect::<Vec<_>>(),
        "outputs":f.outputs().iter().map(|o|json!({"operation":o.operation().0,"output":o.output(),"point":o.point().0,"slot":o.slot().0,"state":o.state().0,"value":f.output_value(o).map(|v|v.0)})).collect::<Vec<_>>(),
        "phis":f.phis().iter().map(|p|json!({"id":p.id().0,"point":p.point().0,"slot":p.slot().0,"state":p.state().0,"value":p.value().0,
            "incoming":p.incoming().iter().map(|i|json!({"transfer":i.transfer().0,"state":i.state().0})).collect::<Vec<_>>()})).collect::<Vec<_>>()
    }))})
}

fn ssa_all(
    engine: &Engine<'_>,
    classes: u32,
    selected: Option<u32>,
) -> Result<Value, RequestError> {
    let started = std::time::Instant::now();
    let (mut methods, mut no_code, mut ready, mut failed, mut upstream, mut class_errors) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut points, mut states, mut values, mut uses, mut phis, mut unavailable) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut typed_ready, mut type_failed, mut type_issues, mut unknown_views) =
        (0u64, 0u64, 0u64, 0u64);
    let mut missing = std::collections::BTreeMap::<String, u64>::new();
    let mut problems = std::collections::BTreeMap::<String, u64>::new();
    let (mut typed_partial, mut conflict_values) = (0u64, 0u64);
    let mut array_fill_types = std::collections::BTreeMap::<String, u64>::new();
    let mut array_fill_unknown = 0u64;
    let mut first_failure = None;
    let mut analysis_time = std::time::Duration::ZERO;
    let (mut local_issues, mut partial_methods, mut unresolved_inputs) = (0u64, 0u64, 0u64);
    let requested = selected.map_or(classes, |_| 1);
    for ordinal in 0..requested {
        let class = selected.unwrap_or(ordinal);
        let began = std::time::Instant::now();
        let result = engine.analyze(Request {
            class: class_key(class),
            method: None,
        });
        analysis_time += began.elapsed();
        let analysis = match result {
            Ok(a) => a,
            Err(e) => {
                class_errors += 1;
                if first_failure.is_none() {
                    first_failure = Some(json!({"class":class,"error":format!("{e}")}));
                }
                continue;
            }
        };
        for entry in &analysis.methods {
            methods += 1;
            let available = match &entry.result {
                Ok(MethodOutcome::Typed(typed)) => {
                    typed_ready += 1;
                    for view in typed.facts().array_fills() {
                        match view.element {
                            Ok(element) => {
                                *array_fill_types.entry(format!("{element:?}")).or_default() += 1
                            }
                            Err(_) => array_fill_unknown += 1,
                        }
                    }
                    type_issues += typed.facts().issues().len() as u64;
                    typed_partial += u64::from(!typed.facts().issues().is_empty());
                    conflict_values += typed
                        .facts()
                        .values()
                        .iter()
                        .filter(|v| v.families().is_conflict())
                        .count() as u64;
                    unknown_views += typed
                        .facts()
                        .uses()
                        .iter()
                        .filter(|u| {
                            matches!(
                                u.interpretation(),
                                diana_decomp::p4::Interpretation::Unknown(_)
                            )
                        })
                        .count() as u64;
                    for issue in typed.facts().issues() {
                        let key = match issue.problem {
                            diana_decomp::p4::Problem::ConstructionPending => "ConstructionPending",
                            diana_decomp::p4::Problem::Missing(_) => "MissingFact",
                            diana_decomp::p4::Problem::FamilyConflict => "FamilyConflict",
                            diana_decomp::p4::Problem::AmbiguousRepresentation(_) => {
                                "AmbiguousRepresentation"
                            }
                            diana_decomp::p4::Problem::AmbiguousIdentity => "AmbiguousIdentity",
                            diana_decomp::p4::Problem::InitializationStateConflict => {
                                "InitializationStateConflict"
                            }
                            diana_decomp::p4::Problem::SourceUnavailable(_) => "SourceUnavailable",
                            diana_decomp::p4::Problem::NarrowingRange => "NarrowingRange",
                            diana_decomp::p4::Problem::LiteralOutOfRange => "LiteralOutOfRange",
                            diana_decomp::p4::Problem::NonzeroReferenceLiteral => {
                                "NonzeroReferenceLiteral"
                            }
                            diana_decomp::p4::Problem::NonArrayDescriptor(_) => {
                                "NonArrayDescriptor"
                            }
                            diana_decomp::p4::Problem::OriginalUseIssue(_) => "OriginalUseIssue",
                            diana_decomp::p4::Problem::OwnerDeclarationUnavailable => {
                                "OwnerDeclarationUnavailable"
                            }
                            diana_decomp::p4::Problem::ConstructorOwnerMismatch => {
                                "ConstructorOwnerMismatch"
                            }
                            diana_decomp::p4::Problem::InvalidCatchType(_) => "InvalidCatchType",
                            diana_decomp::p4::Problem::NonPrimitiveArrayComponent(_) => {
                                "NonPrimitiveArrayComponent"
                            }
                            diana_decomp::p4::Problem::ArrayComponentConflict => {
                                "ArrayComponentConflict"
                            }
                            diana_decomp::p4::Problem::ArrayPayloadUnavailable(_) => {
                                "ArrayPayloadUnavailable"
                            }
                            diana_decomp::p4::Problem::ArrayPayloadWidth { .. } => {
                                "ArrayPayloadWidth"
                            }
                        };
                        *problems.entry(key.into()).or_default() += 1;
                        if let diana_decomp::p4::Problem::Missing(fact) = issue.problem {
                            *missing.entry(format!("{fact:?}")).or_default() += 1;
                        }
                    }
                    Some((typed.ssa_facts(), typed.execution()))
                }
                Ok(MethodOutcome::TypeStopped(stop)) => {
                    type_failed += 1;
                    Some((stop.ssa_facts(), stop.execution()))
                }
                _ => None,
            };
            match &entry.result {
                Ok(MethodOutcome::DeclarationOnly) => no_code += 1,
                Ok(MethodOutcome::Typed(_) | MethodOutcome::TypeStopped(_)) => {
                    ready += 1;
                    let (f, flow) = available
                        .ok_or(RequestError::InternalInvariant("retained SSA view missing"))?;
                    points += f.points().len() as u64;
                    states += f.states().len() as u64;
                    values += f.values().len() as u64;
                    uses += f.uses().len() as u64;
                    phis += f.phis().len() as u64;
                    let unavailable_count = f
                        .uses()
                        .iter()
                        .filter(|u| {
                            !f.state(u.state())
                                .is_some_and(|s| s.availability().is_available())
                        })
                        .count() as u64;
                    unavailable += unavailable_count;
                    let issues = (flow.issues().count()
                        + flow
                            .catch_types()
                            .iter()
                            .filter(|f| f.descriptor().is_err() || f.use_issue().is_some())
                            .count()) as u64;
                    let inputs = f
                        .operation_sites()
                        .iter()
                        .filter(|s| !s.inputs_available())
                        .count() as u64;
                    local_issues += issues;
                    unresolved_inputs += inputs;
                    partial_methods +=
                        u64::from(issues != 0 || inputs != 0 || unavailable_count != 0);
                }
                Ok(MethodOutcome::SsaStopped(stop)) => {
                    failed += 1;
                    if first_failure.is_none() {
                        first_failure = Some(
                            json!({"class":class,"method":entry.key.method.0,"kind":format!("{:?}",stop.error().kind()),"error":format!("{}",stop.error())}),
                        );
                    }
                }
                other => {
                    upstream += 1;
                    if first_failure.is_none() {
                        first_failure = Some(
                            json!({"class":class,"method":entry.key.method.0,"error":match other {
                    Err(e)=>format!("{e}"),Ok(MethodOutcome::FlowStopped(s))=>format!("{}",s.error()),
                    Ok(MethodOutcome::DisassemblyStopped(s))=>format!("{:?}",s.error),_=>"upstream stopped".into()} }),
                        );
                    }
                }
            }
        }
    }
    Ok(
        json!({"classes":requested,"class":selected,"methods":methods,"no_code":no_code,"ssa":ready,"failed":failed,"upstream":upstream,"class_errors":class_errors,
        "partial_methods":partial_methods,"local_issues":local_issues,"unresolved_input_sites":unresolved_inputs,"analysis_ns":analysis_time.as_nanos(),
        "array_fill_types":array_fill_types,"unknown_array_fills":array_fill_unknown,
        "typed":typed_ready,"typed_partial_methods":typed_partial,"conflict_values":conflict_values,"type_failed":type_failed,"type_issues":type_issues,"type_problems":problems,"unknown_type_views":unknown_views,"missing_facts":missing,
        "points":points,"states":states,"values":values,"uses":uses,"phis":phis,"unavailable_uses":unavailable,
        "complete":true,"first_failure":first_failure,"elapsed_ms":started.elapsed().as_millis()}),
    )
}

fn inspection_stop(outcome: &MethodOutcome<'_>) -> Option<Value> {
    match outcome {
        MethodOutcome::DeclarationOnly => Some(json!({"declaration_only": true})),
        MethodOutcome::DisassemblyStopped(outcome) => Some(json!({
            "decoded": false, "prefix": outcome.prefix.len(),
            "error": outcome.error.as_ref().map(|error| format!("{error}")),
        })),
        MethodOutcome::FlowStopped(stop) => Some(json!({
            "decoded": true, "flowed": false,
            "error": { "kind": format!("{:?}", stop.error().kind()), "detail": format!("{}", stop.error()) },
            "retained_items": stop.evidence().items().len(),
            "partial": stop.partial().map(|partial| json!({
                "validated": false, "operations": partial.operations().len(),
                "blocks": partial.blocks().len(), "edges": partial.edges().len(),
            })),
        })),
        MethodOutcome::Typed(_) | MethodOutcome::TypeStopped(_) | MethodOutcome::SsaStopped(_) => {
            None
        }
    }
}

fn encode(instruction: &DecodedInstruction) -> Option<Vec<u16>> {
    let opcode = u16::from(instruction.opcode());
    let regs: Vec<u32> = instruction.registers().map(|register| register.0).collect();
    let literal = instruction.operands().literal();
    let offset = instruction.operands().offset().map(|delta| delta.0);
    let reference_at = |slot: usize| {
        let wanted = if slot == 0 {
            ReferenceSlot::First
        } else {
            ReferenceSlot::Second
        };
        instruction
            .references()
            .find(|reference| reference.slot == wanted)
            .map(|reference| match reference.target {
                diana_decomp::raw::OperandReference::String(id) => id.0,
                diana_decomp::raw::OperandReference::Type(id) => id.0,
                diana_decomp::raw::OperandReference::Field(id) => id.0,
                diana_decomp::raw::OperandReference::Method(id) => id.0,
                diana_decomp::raw::OperandReference::Proto(id) => id.0,
                diana_decomp::raw::OperandReference::CallSite(id) => id.0,
                diana_decomp::raw::OperandReference::MethodHandle(id) => id.0,
            })
    };
    let reference = instruction
        .references()
        .next()
        .map(|reference| match reference.target {
            diana_decomp::raw::OperandReference::String(id) => id.0,
            diana_decomp::raw::OperandReference::Type(id) => id.0,
            diana_decomp::raw::OperandReference::Field(id) => id.0,
            diana_decomp::raw::OperandReference::Method(id) => id.0,
            diana_decomp::raw::OperandReference::Proto(id) => id.0,
            diana_decomp::raw::OperandReference::CallSite(id) => id.0,
            diana_decomp::raw::OperandReference::MethodHandle(id) => id.0,
        });
    let signed = |value: i32, bits: u32| (value as u32 & ((1u32 << bits) - 1)) as u16;
    let word = |value: u32| value as u16;
    let encoded = match instruction.spec().format {
        DexFormat::F10x => vec![opcode],
        DexFormat::F12x => vec![opcode | word(regs.first()? << 8) | word(regs.get(1)? << 12)],
        DexFormat::F11n => {
            let value = match literal? {
                RawLiteral::Signed8(value) => value as u8,
                RawLiteral::Bits32(value) => value as u8,
                _ => return None,
            };
            vec![opcode | word(regs.first()? << 8) | word((u32::from(value) & 0xf) << 12)]
        }
        DexFormat::F11x => vec![opcode | word(regs.first()? << 8)],
        DexFormat::F10t => vec![opcode | signed(offset?, 8) << 8],
        DexFormat::F20t => vec![opcode, word(offset? as u32)],
        DexFormat::F22x => vec![opcode | word(regs.first()? << 8), word(*regs.get(1)?)],
        DexFormat::F21t | DexFormat::F21s | DexFormat::F21c => {
            let value = match instruction.spec().format {
                DexFormat::F21t => word(offset? as u32),
                DexFormat::F21s => match literal? {
                    RawLiteral::Signed16(value) => word(u32::from(value as u16)),
                    RawLiteral::Signed8(value) => word(u32::from(value as u8 as u16)),
                    RawLiteral::Bits32(value) => word(value),
                    RawLiteral::Bits64(value) => word(value as u32),
                },
                _ => word(reference?),
            };
            vec![opcode | word(regs.first()? << 8), value]
        }
        DexFormat::F23x => vec![
            opcode | word(regs.first()? << 8),
            word(*regs.get(1)? | (*regs.get(2)? << 8)),
        ],
        DexFormat::F22b => {
            let RawLiteral::Signed8(value) = literal? else {
                return None;
            };
            vec![
                opcode | word(regs.first()? << 8),
                word(*regs.get(1)? | ((u32::from(value as u8)) << 8)),
            ]
        }
        DexFormat::F22t | DexFormat::F22s | DexFormat::F22c => {
            let value = match instruction.spec().format {
                DexFormat::F22t => word(offset? as u32),
                DexFormat::F22s => match literal? {
                    RawLiteral::Signed16(value) => word(u32::from(value as u16)),
                    RawLiteral::Signed8(value) => word(u32::from(value as u8 as u16)),
                    RawLiteral::Bits32(value) => word(value),
                    _ => return None,
                },
                _ => word(reference?),
            };
            vec![
                opcode | word(regs.first()? << 8) | word(*regs.get(1)? << 12),
                value,
            ]
        }
        DexFormat::F30t => {
            let value = offset? as u32;
            vec![opcode, word(value), word(value >> 16)]
        }
        DexFormat::F32x => vec![opcode, word(*regs.first()?), word(*regs.get(1)?)],
        DexFormat::F31i | DexFormat::F31t => {
            let value = if matches!(instruction.spec().format, DexFormat::F31t) {
                offset? as u32
            } else {
                match literal? {
                    RawLiteral::Bits32(value) => value,
                    RawLiteral::Bits64(value) => value as u32,
                    _ => return None,
                }
            };
            vec![
                opcode | word(regs.first()? << 8),
                word(value),
                word(value >> 16),
            ]
        }
        DexFormat::F31c => {
            let value = reference?;
            vec![
                opcode | word(regs.first()? << 8),
                word(value),
                word(value >> 16),
            ]
        }
        DexFormat::F21h => {
            let value = match literal? {
                RawLiteral::Bits32(value) => value >> 16,
                RawLiteral::Bits64(value) => (value >> 48) as u32,
                _ => return None,
            };
            vec![opcode | word(regs.first()? << 8), word(value)]
        }
        DexFormat::F51l => {
            let RawLiteral::Bits64(value) = literal? else {
                return None;
            };
            vec![
                opcode | word(regs.first()? << 8),
                word(value as u32),
                word((value >> 16) as u32),
                word((value >> 32) as u32),
                word((value >> 48) as u32),
            ]
        }
        DexFormat::F35c | DexFormat::F45cc => {
            let (values, count) = instruction.operands().registers().list()?;
            let index = reference_at(0)?;
            let mut words = vec![
                opcode | word(values[4].0 << 8) | word(u32::from(count) << 12),
                word(index),
                word(values[0].0 | (values[1].0 << 4) | (values[2].0 << 8) | (values[3].0 << 12)),
            ];
            if matches!(instruction.spec().format, DexFormat::F45cc) {
                words.push(word(reference_at(1)?));
            }
            words
        }
        DexFormat::F3rc | DexFormat::F4rcc => {
            let (first, count) = instruction.operands().registers().range()?;
            let mut words = vec![
                opcode | word(u32::from(count) << 8),
                word(reference_at(0)?),
                word(first.0),
            ];
            if matches!(instruction.spec().format, DexFormat::F4rcc) {
                words.push(word(reference_at(1)?));
            }
            words
        }
    };
    Some(encoded)
}

fn encode_payload(
    payload: &DecodedPayload,
    source: &diana_decomp::input::RawMethodView<'_>,
) -> Option<Vec<u8>> {
    let mut words: Vec<u16> = Vec::new();
    match payload.kind() {
        diana_decomp::raw::LayoutKind::PackedSwitch { entries } => {
            let mut cases = payload.switch_cases(source).ok()??;
            let first = cases.next()?;
            words.push(0x0100);
            words.push(entries);
            words.push(first.key as u32 as u16);
            words.push((first.key as u32 >> 16) as u16);
            words.push(first.offset.0 as u32 as u16);
            words.push((first.offset.0 as u32 >> 16) as u16);
            for case in cases {
                words.push(case.offset.0 as u32 as u16);
                words.push((case.offset.0 as u32 >> 16) as u16);
            }
        }
        diana_decomp::raw::LayoutKind::SparseSwitch { entries } => {
            let cases = payload.switch_cases(source).ok()??;
            let cases: Vec<_> = cases.collect();
            words.push(0x0200);
            words.push(entries);
            for case in &cases {
                words.push(case.key as u32 as u16);
                words.push((case.key as u32 >> 16) as u16);
            }
            for case in cases {
                words.push(case.offset.0 as u32 as u16);
                words.push((case.offset.0 as u32 >> 16) as u16);
            }
        }
        diana_decomp::raw::LayoutKind::ArrayData {
            element_width,
            element_count,
        } => {
            words.push(0x0300);
            words.push(element_width.0 as u16);
            words.push(element_count as u16);
            words.push((element_count >> 16) as u16);
            let elements = payload.array_elements(source).ok()??;
            let mut bytes = Vec::new();
            for element in elements {
                for index in 0..element_width.0 {
                    bytes.push((element.bits >> (index * 8)) as u8);
                }
            }
            if bytes.len() % 2 != 0 {
                bytes.push(0);
            }
            let mut packed = Vec::new();
            for pair in bytes.as_chunks::<2>().0 {
                packed.push(u16::from_le_bytes(*pair));
            }
            words.extend(packed);
        }
        _ => return None,
    }
    let mut bytes = Vec::new();
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    Some(bytes)
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
    for (position, word) in words.enumerate() {
        let name = if position == 0 { "class" } else { "method" };
        if let Ok(value) = word.parse::<u64>() {
            object.insert(name.into(), Value::Number(value.into()));
        }
    }
    Ok(Value::Object(object))
}

fn render_text(value: &Value, indent: usize, output: &mut String) {
    let pad = " ".repeat(indent);
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                match value {
                    Value::Object(inner) if !inner.is_empty() => {
                        output.push_str(&format!("{pad}{key}:\n"));
                        render_text(value, indent + 2, output);
                    }
                    Value::Array(items) if !items.is_empty() => {
                        output.push_str(&format!("{pad}{key}:\n"));
                        render_text(value, indent + 2, output);
                    }
                    _ => {
                        output.push_str(&format!("{pad}{key}: "));
                        render_text(value, indent, output);
                        output.push('\n');
                    }
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                output.push_str(&format!("{pad}- "));
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        output.push('\n');
                        render_text(item, indent + 2, output);
                    }
                    _ => {
                        render_text(item, indent, output);
                        output.push('\n');
                    }
                }
            }
        }
        Value::String(text) => output.push_str(text),
        other => output.push_str(&other.to_string()),
    }
}

fn write_value(output: &mut impl Write, value: &Value, format: Format) -> std::io::Result<()> {
    if format == Format::Json {
        return writeln!(output, "{value}");
    }
    if value.get("ok").and_then(Value::as_bool) == Some(true) {
        if let Some(result) = value.get("result") {
            let mut text = String::new();
            render_text(result, 0, &mut text);
            if !text.is_empty() {
                writeln!(output, "{text}")?;
            }
        }
        writeln!(output)
    } else {
        let detail = value
            .get("error")
            .and_then(|error| error.get("detail"))
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        let kind = value
            .get("error")
            .and_then(|error| error.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("error");
        writeln!(output, "error: {kind}: {detail}")
    }
}

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let mut dex = None;
    let mut exec = Vec::new();
    let mut explicit_format = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dex" | "-d" => {
                let Some(path) = args.next() else {
                    eprintln!("dbg_shell: --dex needs a path");
                    return std::process::ExitCode::from(2);
                };
                dex = Some(PathBuf::from(path));
            }
            "--format" | "-f" => {
                let Some(value) = args.next() else {
                    eprintln!("dbg_shell: --format needs json or text");
                    return std::process::ExitCode::from(2);
                };
                explicit_format = Some(match value.as_str() {
                    "json" => Format::Json,
                    "text" => Format::Text,
                    _ => {
                        eprintln!("dbg_shell: --format must be json or text");
                        return std::process::ExitCode::from(2);
                    }
                });
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
                     stdin: one command per line, JSON or `p1 <class> <method>`/`p2 <class> <method>`
                     default: text for stdin shell, json for --exec; --format overrides"
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
    let format = explicit_format.unwrap_or(if exec.is_empty() {
        Format::Text
    } else {
        Format::Json
    });
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
            let _ = write_value(&mut stdout, &value, format);
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
        if write_value(&mut stdout, &value, format).is_err() {
            break;
        }
        if request.get("cmd").and_then(Value::as_str) == Some("quit") {
            break;
        }
    }
    std::process::ExitCode::SUCCESS
}
