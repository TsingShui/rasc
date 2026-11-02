//! Development caller for the application query modules (E2E application seams).
//!
//! Usage: app_query <defined-methods|direct-method-xref> <input> [--filter F] [--target REF]
use std::sync::atomic::AtomicBool;

use diana::analysis::input::{InputLimits, InputOptions};
use diana::analysis::query::{Query, QueryKind, QueryLimits, QueryPlan, Row};
use diana::analysis::request::RequestScope;
use diana::analysis::selector;
use diana::analysis::xref::{Matched, Target, Xref, XrefPlan};
use serde_json::{Value, json};

fn main() {
    let mut args = std::env::args().skip(1);
    let kind = args.next().expect("query kind");
    let input = args.next().expect("input path");
    let mut filter = None;
    let mut target = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--filter" => filter = args.next(),
            "--target" => target = args.next(),
            other => panic!("unknown flag {other}"),
        }
    }
    let scope = RequestScope::open(
        std::path::Path::new(&input),
        &InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        },
    )
    .expect("lease");
    let cancel = AtomicBool::new(false);
    let rows = match kind.as_str() {
        "defined-methods" => {
            let query = Query::new(
                &scope,
                QueryPlan {
                    kind: QueryKind::Methods,
                    filter: filter.as_deref().map(|text| text.encode_utf16().collect()),
                    class: None,
                    class_filter: Default::default(),
                    limits: QueryLimits::default(),
                    jobs: 4,
                },
            )
            .expect("query");
            let mut rows = Vec::new();
            query
                .run(&cancel, &mut |row| {
                    if let Row::Method(row) = row {
                        rows.push(json!({
                            "owner": text(&row.class_descriptor),
                            "name": text(&row.name),
                            "proto": text(&row.prototype),
                            "method_id_idx": row.method.0,
                            "pc": Value::Null,
                            "operand_slot": Value::Null,
                        }));
                    }
                    Ok(())
                })
                .expect("run");
            rows
        }
        "direct-method-xref" => {
            let reference = target.expect("--target");
            let selector = selector::method(&reference).expect("method reference");
            let xref = Xref::new(
                &scope,
                XrefPlan {
                    target: Target::Method {
                        owner: selector.class.descriptor,
                        name: selector.name,
                        prototype: selector.prototype,
                    },
                    class: None,
                    limits: QueryLimits::default(),
                    jobs: 4,
                },
            )
            .expect("xref");
            let mut rows = Vec::new();
            xref.run(&cancel, &mut |row| {
                if !matches!(row.matched, Matched::Method { .. }) {
                    return Ok(());
                }
                rows.push(json!({
                    "owner": text(&row.class_descriptor),
                    "name": text(&row.method_name),
                    "proto": text(&row.method_prototype),
                    "method_id_idx": row.method.0,
                    "pc": row.code_unit,
                    "operand_slot": 0,
                }));
                Ok(())
            })
            .expect("run");
            rows
        }
        other => panic!("unknown query kind {other}"),
    };
    println!("{}", json!({"schema_version":1,"rows":rows}));
}

fn text(units: &[u16]) -> String {
    diana::analysis::at::escape_units(units)
}
