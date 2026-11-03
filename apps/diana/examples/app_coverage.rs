//! Development caller that measures member-level Java coverage on one input.
//!
//! Usage: app_coverage <input> <classes>
use std::collections::BTreeMap;
use std::time::Instant;

use diana::analysis::decompile;
use diana::analysis::input::{InputLimits, InputOptions};
use diana::analysis::query::{Query, QueryKind, QueryLimits, QueryPlan};
use diana::analysis::request::RequestScope;
use diana::analysis::selector::{ClassSelector, Selector};
use diana_decomp::{ClassOutput, Limits, MethodOutput, RunContext};
use serde_json::json;

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("input");
    let sample: usize = args.next().expect("classes").parse().expect("classes");
    let options = InputOptions {
        limits: InputLimits::default(),
        allowed_root: None,
    };
    let scope = RequestScope::open(std::path::Path::new(&input), &options).expect("open input");
    let images = diana::analysis::images::ImageSet::open(&scope).expect("images");
    let mut controls = RunContext::new(Limits::default(), &|| false);
    let query = Query::new(
        &scope,
        QueryPlan {
            kind: QueryKind::Classes,
            filter: None,
            class: None,
            class_filter: Default::default(),
            limits: QueryLimits::default(),
            jobs: 1,
        },
    )
    .expect("query");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut all = Vec::new();
    query
        .run(&cancel, &mut |row| {
            if let diana::analysis::query::Row::Class(class) = row {
                all.push((class.image.0, class.class_def.0, class.descriptor));
            }
            Ok(())
        })
        .expect("list classes");
    let step = (all.len() / sample.max(1)).max(1);
    let selected: Vec<_> = all.iter().step_by(step).take(sample).cloned().collect();
    let mut member_status: BTreeMap<String, u64> = BTreeMap::new();
    let mut class_status: BTreeMap<String, u64> = BTreeMap::new();
    let mut diagnostic_codes: BTreeMap<String, u64> = BTreeMap::new();
    let mut durations = Vec::new();
    for (index, class_def, descriptor) in &selected {
        let image = images.images().get(*index as usize);
        let logical = image.map_or(0, |image| image.logical);
        let entry = image.and_then(|image| image.entry).and_then(|entry| u32::try_from(entry.0).ok());
        let selector = Selector::Class(ClassSelector {
            entry,
            logical: Some(logical),
            class_def: Some(*class_def),
            descriptor: descriptor.clone(),
        });
        let started = Instant::now();
        match decompile::run(&scope, &selector, &mut controls) {
            Ok(result) => {
                durations.push(started.elapsed());
                *class_status.entry(result.status.to_owned()).or_default() += 1;
                for diagnostic in result.report.diagnostics() {
                    *diagnostic_codes
                        .entry(format!("{:?}", diagnostic.code()))
                        .or_default() += 1;
                }
                match result.report.class() {
                    ClassOutput::JavaUnit { members, .. } | ClassOutput::Mixed { members, .. } => {
                        for member in members {
                            let name = match member {
                                MethodOutput::Java { .. } => "java",
                                MethodOutput::JavaInitializer { .. } => "java_initializer",
                                MethodOutput::DeclarationOnly { .. } => "declaration",
                                MethodOutput::Generated { .. } => "generated",
                                MethodOutput::Debug { .. } => "debug",
                                MethodOutput::Unavailable { .. } => "unavailable",
                            };
                            *member_status.entry(name.to_owned()).or_default() += 1;
                        }
                    }
                }
            }
            Err(error) => {
                durations.push(started.elapsed());
                *class_status
                    .entry(format!("error:{}", error.code()))
                    .or_default() += 1;
            }
        }
    }
    let mut sorted = durations.clone();
    sorted.sort();
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
    let total: f64 = durations.iter().map(|value| value.as_secs_f64()).sum();
    println!(
        "{}",
        json!({
            "input": input,
            "input_class_count": all.len(),
            "sampled": selected.len(),
            "class_status": class_status,
            "member_status": member_status,
            "diagnostic_codes": diagnostic_codes,
            "seconds": {"total": total, "median_ms": median.as_secs_f64() * 1000.0,
                        "max_ms": sorted.last().copied().unwrap_or_default().as_secs_f64() * 1000.0},
        })
    );
}
