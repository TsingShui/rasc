//! Development caller that measures member-level Java coverage on one input.
//!
//! Usage: app_coverage <input> <classes|all> [members.ndjson [shard-index shard-count]]
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::{Duration, Instant};

use diana::analysis::images::ImageSet;
use diana::analysis::input::InputOptions;
use diana::analysis::request::RequestScope;
use diana_decomp::{
    ClassKey, ClassOutput, DexId, DexImage, Diagnostic, DiagnosticCause, Engine, FailureKind,
    ImageOrigin, InputId, MethodId, MethodOutput, Options, OutputReason, Request, RunContext,
};
use serde_json::{Value, json};

struct SelectedClass {
    key: ClassKey,
    descriptor: Vec<u16>,
    methods: HashSet<MethodId>,
    enumeration_error: Option<String>,
}

#[derive(Default)]
struct Totals {
    class_status: BTreeMap<String, u64>,
    member_status: BTreeMap<String, u64>,
    diagnostic_codes: BTreeMap<String, u64>,
    failure_kinds: BTreeMap<String, u64>,
    rendered_bytes: u64,
    render_errors: u64,
    missing_methods: u64,
    unexpected_methods: u64,
    enumeration_errors: u64,
    class_errors: u64,
    unobserved_methods: u64,
}

fn count(map: &mut BTreeMap<String, u64>, key: impl Into<String>) {
    *map.entry(key.into()).or_default() += 1;
}

fn reason(diagnostic: &Diagnostic) -> Value {
    let mut result = json!({
        "code": format!("{:?}", diagnostic.code()),
        "kind": format!("{:?}", diagnostic.cause.kind()),
    });
    match &diagnostic.cause {
        DiagnosticCause::Unsupported(OutputReason::JavaCheckFailed {
            phase, check, pc, ..
        }) => {
            result["phase"] = json!(phase);
            result["check"] = json!(check);
            result["code_unit"] = json!(pc.map(|pc| pc.0));
        }
        DiagnosticCause::Request(error) => result["detail"] = json!(error.to_string()),
        _ => result["detail"] = json!(format!("{:?}", diagnostic.cause)),
    }
    result
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("input required")?;
    let selection = args.next().ok_or("classes or all required")?;
    let sample = if selection == "all" {
        None
    } else {
        let count: usize = selection.parse()?;
        if count == 0 {
            return Err("classes must be positive".into());
        }
        Some(count)
    };
    let mut records = args
        .next()
        .map(File::create)
        .transpose()?
        .map(BufWriter::new);
    let shard_index: usize = args
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(0);
    let shard_count: usize = args
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1);
    if shard_count == 0 || shard_index >= shard_count {
        return Err("shard-index must be less than a positive shard-count".into());
    }
    let wall = Instant::now();
    let options = InputOptions { allowed_root: None };
    let scope = RequestScope::open(std::path::Path::new(&input), &options)?;
    let images = ImageSet::open(&scope)?;
    let mut control = RunContext::new_parallel(&|| false);
    let mut dex_images = Vec::new();
    for image in images.images() {
        dex_images.push(DexImage {
            bytes: images.bytes(image),
            header_offset: image.header_offset,
            origin: ImageOrigin {
                input: InputId(u64::try_from(image.entry.map_or(0, |entry| entry.0))?),
            },
        });
    }
    let jobs = std::thread::available_parallelism().map_or(1, |count| count.get());
    let engine = Engine::open(dex_images, Options::default(), &mut control)?.with_method_jobs(jobs);
    let mut all = Vec::new();
    for (position, image) in images.images().iter().enumerate() {
        let dex = DexId(u32::try_from(position)?);
        let program = images.program(image, &mut control)?;
        program.visit_classes(&mut control, |id, class, control| {
            let class = class?;
            let mut methods = HashSet::new();
            let enumeration = class.visit_methods(control, |method, _| {
                methods.insert(method.method_idx);
                Ok(())
            });
            let enumeration_error = match enumeration {
                Ok(()) => None,
                Err(error) if error.kind() == FailureKind::Malformed => Some(error.to_string()),
                Err(error) => return Err(error),
            };
            all.push(SelectedClass {
                key: ClassKey { dex, class_def: id },
                descriptor: class.descriptor.units(control)?,
                methods,
                enumeration_error,
            });
            Ok(())
        })?;
    }
    let selection_count = sample.unwrap_or(all.len()).min(all.len());
    let step = sample.map_or(1, |sample| (all.len() / sample).max(1));
    let selected_count =
        selection_count / shard_count + usize::from(shard_index < selection_count % shard_count);
    let prepare_seconds = wall.elapsed().as_secs_f64();
    eprintln!(
        "prepared {} logical DEX images, {} classes; selected {} classes; {:.3}s",
        images.images().len(),
        all.len(),
        selected_count,
        prepare_seconds
    );
    let mut totals = Totals::default();
    let mut durations = Vec::new();
    let mut slow = Vec::new();
    let mut requested = 0usize;
    let mut original_method_count = 0usize;
    let mut last_progress = Instant::now();
    let mut termination = None;
    for (_, class) in all
        .iter()
        .step_by(step)
        .take(selection_count)
        .enumerate()
        .filter(|(position, _)| position % shard_count == shard_index)
    {
        requested += 1;
        original_method_count += class.methods.len();
        if class.enumeration_error.is_some() {
            totals.enumeration_errors += 1;
        }
        let started = Instant::now();
        let result = engine.decompile(
            Request {
                class: class.key,
                method: None,
            },
            &mut control,
        );
        let elapsed = started.elapsed();
        durations.push(elapsed);
        slow.push((elapsed, class.key.dex.0, class.key.class_def.0));
        match result {
            Ok(report) => {
                let status = match report.class() {
                    ClassOutput::JavaUnit { .. } => "java_unit",
                    ClassOutput::Mixed { .. } => "mixed",
                };
                count(&mut totals.class_status, status);
                for diagnostic in report.diagnostics() {
                    count(
                        &mut totals.diagnostic_codes,
                        format!("{:?}", diagnostic.code()),
                    );
                }
                // Each class_def is requested once. Nested reports are rendered,
                // but their members are counted only in their own class request.
                let mut actual_methods = HashSet::new();
                for member in report.class().members() {
                    actual_methods.insert(member.key().method);
                    let (status, body, diagnostic, facts) = match member {
                        MethodOutput::Java { facts, .. } => ("java", "code", None, Some(facts)),
                        MethodOutput::JavaInitializer { facts, .. } => {
                            ("java_initializer", "initializer", None, Some(facts))
                        }
                        MethodOutput::DeclarationOnly { facts, .. } => {
                            ("declaration", "declaration", None, Some(facts))
                        }
                        MethodOutput::Generated { facts, .. } => {
                            ("generated", "generated", None, Some(facts))
                        }
                        MethodOutput::Debug { reason, facts, .. } => {
                            ("debug", "original", Some(*reason), facts.as_ref())
                        }
                        MethodOutput::Unavailable { reason, facts, .. } => {
                            ("unavailable", "none", Some(*reason), facts.as_ref())
                        }
                    };
                    count(&mut totals.member_status, status);
                    let diagnostic = diagnostic.map(|id| {
                        let diagnostic = &report.diagnostics()[id.0];
                        count(
                            &mut totals.member_status,
                            format!("{status}:{:?}", diagnostic.code()),
                        );
                        count(
                            &mut totals.failure_kinds,
                            format!("{:?}", diagnostic.cause.kind()),
                        );
                        let value = reason(diagnostic);
                        if let Some(check) = value["check"].as_str() {
                            count(&mut totals.member_status, format!("check:{check}"));
                        }
                        value
                    });
                    if let Some(records) = records.as_mut() {
                        writeln!(
                            records,
                            "{}",
                            json!({
                                "dex": class.key.dex.0, "class": class.key.class_def.0,
                                "descriptor_units": class.descriptor, "method": member.key().method.0,
                                "name_units": facts.map(|facts| &facts.name),
                                "prototype_units": facts.map(|facts| &facts.prototype),
                                "status": status, "body": body, "reason": diagnostic,
                            })
                        )?;
                    }
                }
                totals.missing_methods += class.methods.difference(&actual_methods).count() as u64;
                totals.unexpected_methods +=
                    actual_methods.difference(&class.methods).count() as u64;
                match diana_decomp::render(&report, &mut control) {
                    Ok(rendered) => totals.rendered_bytes += u64::try_from(rendered.source.len())?,
                    Err(error) => {
                        totals.render_errors += 1;
                        eprintln!(
                            "render error dex={} class={}: {error}",
                            class.key.dex.0, class.key.class_def.0
                        );
                        if matches!(
                            error.kind(),
                            FailureKind::Cancelled | FailureKind::ResourceExhausted
                        ) {
                            termination = Some(error.to_string());
                        }
                    }
                }
            }
            Err(error) => {
                totals.class_errors += 1;
                totals.unobserved_methods += u64::try_from(class.methods.len())?;
                count(
                    &mut totals.class_status,
                    format!("error:{:?}", error.kind()),
                );
                eprintln!(
                    "class error dex={} class={}: {error}",
                    class.key.dex.0, class.key.class_def.0
                );
                if let Some(records) = records.as_mut() {
                    writeln!(
                        records,
                        "{}",
                        json!({"scope":"class", "dex":class.key.dex.0, "class":class.key.class_def.0, "reason":error.to_string(), "kind":format!("{:?}",error.kind()), "known_methods":class.methods.len()})
                    )?;
                }
                if matches!(
                    error.kind(),
                    FailureKind::Cancelled | FailureKind::ResourceExhausted
                ) {
                    termination = Some(error.to_string());
                }
            }
        }
        if requested.is_multiple_of(100) || last_progress.elapsed() >= Duration::from_secs(5) {
            if let Some(records) = records.as_mut() {
                records.flush()?;
            }
            eprintln!(
                "progress {requested}/{selected_count}; wall {:.1}s; java {}; debug {}; unavailable {}; errors {}",
                wall.elapsed().as_secs_f64(),
                totals.member_status.get("java").copied().unwrap_or(0),
                totals.member_status.get("debug").copied().unwrap_or(0),
                totals
                    .member_status
                    .get("unavailable")
                    .copied()
                    .unwrap_or(0),
                totals.class_errors
            );
            last_progress = Instant::now();
        }
        if termination.is_some() {
            break;
        }
    }
    if let Some(records) = records.as_mut() {
        records.flush()?;
    }
    slow.sort_by_key(|(elapsed, _, _)| std::cmp::Reverse(*elapsed));
    for (elapsed, dex, class) in slow.iter().take(5) {
        eprintln!(
            "slow {:>9.3}s dex={dex} class={class}",
            elapsed.as_secs_f64()
        );
    }
    durations.sort();
    let median = durations
        .get(durations.len() / 2)
        .copied()
        .unwrap_or_default();
    let total: f64 = durations.iter().map(|value| value.as_secs_f64()).sum();
    println!(
        "{}",
        json!({
            "input_class_count":all.len(), "logical_dex_count":images.images().len(),
            "sampled":requested, "selected":selected_count, "selection":selection,
            "shard_index":shard_index, "shard_count":shard_count,
            "complete":requested == selected_count && termination.is_none(), "termination":termination,
            "engine_scope":"shared", "method_jobs":jobs, "member_counting":"selected class roots only; nested members counted on their own request",
            "original_method_count":original_method_count, "missing_methods":totals.missing_methods,
            "unexpected_methods":totals.unexpected_methods, "enumeration_errors":totals.enumeration_errors,
            "class_errors":totals.class_errors, "render_errors":totals.render_errors,
            "unobserved_methods":totals.unobserved_methods,
            "rendered_bytes_including_nested_and_debug":totals.rendered_bytes,
            "class_status":totals.class_status, "member_status":totals.member_status,
            "diagnostic_codes":totals.diagnostic_codes, "failure_kinds":totals.failure_kinds,
            "seconds":{"wall":wall.elapsed().as_secs_f64(), "prepare":prepare_seconds, "total":total,
                "median_ms":median.as_secs_f64()*1000.0, "max_ms":durations.last().copied().unwrap_or_default().as_secs_f64()*1000.0},
        })
    );
    Ok(())
}
