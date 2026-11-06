//! `xref`: original operand reference sites, one row per site.

use std::path::Path;

use super::output::{Failure, Format};
use super::table::publish_rows;
use crate::analysis::at;
use crate::analysis::input::InputOptions;
use crate::analysis::request::RequestScope;
use crate::analysis::selector;
use crate::analysis::xref::{Matched, Target, Xref, XrefPlan};

#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &Path,
    kind: &str,
    target: &str,
    class: Option<&str>,
    jobs: usize,
    format: Format,
    output: Option<&Path>,
) -> Result<(), Failure> {
    let target = parse_target(kind, target)?;
    let class = match class {
        Some(text) => Some(
            selector::class(text)
                .map_err(|error| Failure {
                    code: error.code(),
                    message: error.to_string(),
                })?
                .descriptor,
        ),
        None => None,
    };
    let scope =
        RequestScope::open(input, &InputOptions { allowed_root: None }).map_err(Failure::input)?;
    let xref = Xref::new(
        &scope,
        XrefPlan {
            target,
            class,

            jobs,
        },
    )
    .map_err(|error| Failure::query(error, 1))?;
    let images = xref.images().to_vec();

    let mut rows = Vec::new();
    xref.run(&mut |row| {
        rows.push(row);
        Ok(())
    })
    .map_err(|error| Failure::query(error, 1))?;
    let mut output_rows = Vec::new();
    for row in &rows {
        let image = &images[row.image.0];
        let context = format!(
            "{}@cu:{:#06x}",
            at::method(
                image.entry.map(|entry| entry.0),
                row.logical.0,
                row.class_def.0,
                row.method.0,
                &row.class_descriptor,
                &row.method_name,
                &row.method_prototype,
            ),
            row.code_unit
        );
        output_rows.push(vec![
            image.label.clone(),
            context,
            matched_text(&row.matched),
            row.use_.to_owned(),
        ]);
    }
    publish_rows(
        &output_rows,
        &["entry", "context", "match", "use"],
        "xref",
        format,
        input,
        output,
    )
}

fn parse_target(kind: &str, text: &str) -> Result<Target, Failure> {
    let invalid = |message: String| Failure { code: 2, message };
    match kind {
        "string" => Ok(Target::String(text.encode_utf16().collect())),
        "type" => Ok(Target::Type(
            selector::class(text)
                .map_err(|error| invalid(error.to_string()))?
                .descriptor,
        )),
        "method" => {
            let selector = selector::method(text).map_err(|error| invalid(error.to_string()))?;
            Ok(Target::Method {
                owner: selector.class.descriptor,
                name: selector.name,
                prototype: selector.prototype,
            })
        }
        "field" => {
            let selector = selector::field(text).map_err(|error| invalid(error.to_string()))?;
            Ok(Target::Field {
                owner: selector.class.descriptor,
                name: selector.name,
                field_type: selector.field_type,
            })
        }
        other => Err(invalid(format!(
            "unknown xref kind {other:?}; expected string|type|method|field"
        ))),
    }
}

fn matched_text(matched: &Matched) -> String {
    match matched {
        Matched::String(units) | Matched::Type(units) => at::escape_units(units),
        Matched::Method {
            owner,
            name,
            prototype,
        } => format!(
            "{}->{}{}",
            at::escape_units(owner),
            at::escape_units(name),
            at::escape_units(prototype)
        ),
        Matched::Field {
            owner,
            name,
            field_type,
        } => format!(
            "{}->{}:{}",
            at::escape_units(owner),
            at::escape_units(name),
            at::escape_units(field_type)
        ),
    }
}
