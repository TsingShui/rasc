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
                all.push((
                    class.image.0,
                    class.logical.0,
                    class.class_def.0,
                    class.descriptor,
                ));
            }
            Ok(())
        })
        .expect("list classes");
    let step = (all.len() / count.max(1)).max(1);
    for (index, logical, class_def, descriptor) in all.iter().step_by(step).take(count) {
        let entry = query.images()[*index].entry.map_or(0, |entry| entry.0);
        let entry = Some(u32::try_from(entry).expect("physical entry index"));
        let selector = Selector::Class(ClassSelector {
            entry,
            logical: Some(*logical),
            class_def: Some(*class_def),
            descriptor: descriptor.clone(),
        });
        let started = Instant::now();
        let result = decompile::run(&scope, &selector).expect("decompile");
        eprintln!(
            "class {class_def} {} {:.3}s",
            match &result.rendered {
                diana_decomp::p9::RenderOutcome::Ready(_) => "rendered",
                diana_decomp::p9::RenderOutcome::Stopped(_) => "render_stopped",
            },
            started.elapsed().as_secs_f64()
        );
    }
    drop(_guard);
    drop(root);
    fastrace::flush();
}
