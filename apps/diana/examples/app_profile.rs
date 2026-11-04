//! Development profiling caller for the phase timeline.
//!
//! Usage: app_profile <input> <classes> [--once]
use std::time::Instant;

use diana::analysis::decompile;
use diana::analysis::input::InputOptions;
use diana::analysis::query::{Query, QueryKind, QueryPlan};
use diana::analysis::request::RequestScope;
use diana::analysis::selector::{ClassSelector, Selector};

fn main() {
    fastrace::set_reporter(
        fastrace::collector::ConsoleReporter,
        fastrace::collector::Config::default(),
    );
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("input");
    let count: usize = args.next().expect("classes").parse().expect("classes");
    let options = InputOptions { allowed_root: None };
    let scope = RequestScope::open(std::path::Path::new(&input), &options).expect("open input");
    let images = diana::analysis::images::ImageSet::open(&scope).expect("images");
    let root = fastrace::Span::root("profile", fastrace::collector::SpanContext::random());
    let _guard = root.set_local_parent();

    let query = Query::new(
        &scope,
        QueryPlan {
            kind: QueryKind::Classes,
            filter: None,
            class: None,
            class_filter: Default::default(),

            jobs: 1,
        },
    )
    .expect("query");

    let mut all = Vec::new();
    query
        .run(&mut |row| {
            if let diana::analysis::query::Row::Class(class) = row {
                all.push((class.image.0, class.class_def.0, class.descriptor));
            }
            Ok(())
        })
        .expect("list classes");
    let step = (all.len() / count.max(1)).max(1);
    for (index, class_def, descriptor) in all.iter().step_by(step).take(count) {
        let image = images.images().get(*index);
        let logical = image.map_or(0, |image| image.logical);
        let entry = image
            .and_then(|image| image.entry)
            .and_then(|entry| u32::try_from(entry.0).ok());
        let selector = Selector::Class(ClassSelector {
            entry,
            logical: Some(logical),
            class_def: Some(*class_def),
            descriptor: descriptor.clone(),
        });
        let started = Instant::now();
        let result = decompile::run(&scope, &selector).expect("decompile");
        eprintln!(
            "class {class_def} {} {:.3}s",
            result.status,
            started.elapsed().as_secs_f64()
        );
    }
    drop(_guard);
    drop(root);
    fastrace::flush();
}
