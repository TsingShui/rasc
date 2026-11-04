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

use diana_decomp::input::{ProgramIndex, RawPrintable};
use diana_decomp::p1::{DecodedInstruction, DecodedItem, DecodedPayload};
use diana_decomp::raw::{Format as DexFormat, RawLiteral, ReferenceSlot};
use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, DexId, DexImage, Engine, FailureKind, ImageOrigin, InputId,
    MethodId, MethodOutcome, Request, RequestError, RunContext,
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
        let cancel = || false;
        let mut control = RunContext::new(&cancel);
        let index = ProgramIndex::open(&self.bytes, &mut control).map_err(RequestError::Input)?;
        let engine = Engine::open(
            vec![DexImage {
                bytes: &self.bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
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
                    control.checkpoint()?;
                    let facts = match diana_decomp::p0::collect(
                        &index,
                        ClassKey {
                            dex: DexId(0),
                            class_def: ClassDefId(class),
                        },
                        &mut control,
                    ) {
                        Ok(facts) => facts,
                        Err(error) if error.kind() == FailureKind::Cancelled => return Err(error),
                        Err(_) => {
                            class_errors += 1;
                            continue;
                        }
                    };
                    let base = facts.descriptor().units();
                    let _ = base;
                    for entry in facts.methods() {
                        control.checkpoint()?;
                        methods += 1;
                        let method = entry.method();
                        let source = match index.method_source(&facts, method, &mut control) {
                            Ok(source) => source,
                            Err(_) => {
                                method_errors += 1;
                                continue;
                            }
                        };
                        if matches!(source.raw(), RawPrintable::Declaration) {
                            no_code += 1;
                            continue;
                        }
                        let outcome = match index.decode_method_source(&source, &mut control) {
                            Ok(outcome) => outcome,
                            Err(error) if error.kind() == FailureKind::Cancelled => {
                                return Err(RequestError::from(error));
                            }
                            Err(_) => {
                                method_errors += 1;
                                continue;
                            }
                        };
                        let Some(decoded) = outcome.decoded else {
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
                let outcome = match analysis.methods.swap_remove(index).result? {
                    MethodOutcome::Decoded(outcome) => *outcome,
                    MethodOutcome::DeclarationOnly => {
                        return Ok(json!({"declaration_only": true}));
                    }
                };
                let Some(decoded) = outcome.decoded else {
                    return Ok(json!({
                        "decoded": false,
                        "prefix": outcome.prefix.len(),
                        "error": outcome.error.map(|error| format!("{error}")),
                    }));
                };
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
                let outcome = match analysis.methods.swap_remove(index).result? {
                    MethodOutcome::Decoded(outcome) => *outcome,
                    MethodOutcome::DeclarationOnly => {
                        return Ok(json!({"declaration_only": true}));
                    }
                };
                let Some(decoded) = outcome.decoded else {
                    return Ok(json!({
                        "class_def": class,
                        "method": method,
                        "decoded": false,
                        "prefix": outcome.prefix.len(),
                        "error": outcome.error.map(|error| format!("{error}")),
                    }));
                };
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
                    let clauses = decoded
                        .catch_clauses_at(diana_decomp::CodeUnitOffset(boundary), &mut control)?;
                    if !clauses.is_empty() {
                        catch_targets.push(json!({"try_start": boundary, "clauses": clauses}));
                    }
                }
                Ok(json!({
                    "class_def": class,
                    "method": method,
                    "catch_targets": catch_targets,
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
                     stdin: one command per line, JSON or `p1 <class> <method>`
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
