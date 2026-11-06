//! Deliver the same Engine/P9 publication in either presentation.

use std::path::Path;

use diana_decomp::p9::RenderOutcome;
use serde::Serialize;

use super::output::{Failure, Format, publish_with};
use crate::analysis::decompile::{self, Decompiled};
use crate::analysis::request::RequestScope;
use crate::analysis::selector::{self, Selector};

pub enum Kind {
    Class { jobs: u32 },
    Method,
}

#[derive(Serialize)]
struct Publication<'a> {
    schema_version: u32,
    command: &'static str,
    #[serde(flatten)]
    analysis: &'a Decompiled,
}

pub fn run(
    input: &Path,
    text: &str,
    kind: Kind,
    format: Format,
    output: Option<&Path>,
) -> Result<(), Failure> {
    let parsed = match kind {
        Kind::Class { .. } => selector::class(text).map(Selector::Class),
        Kind::Method => selector::method(text).map(Selector::Method),
    };
    let selector = parsed.map_err(|error| Failure {
        code: error.code(),
        message: error.to_string(),
    })?;
    let scope = RequestScope::open(input, &super::options()).map_err(Failure::input)?;
    let (jobs, command) = match kind {
        Kind::Class { jobs } => (
            usize::try_from(jobs).map_err(|_| Failure {
                code: 2,
                message: "method worker count is not representable on this platform".to_owned(),
            })?,
            "get-class",
        ),
        Kind::Method => (1, "get-method"),
    };
    let decompiled =
        decompile::run_with_jobs(&scope, &selector, jobs).map_err(|error| Failure {
            code: error.code(),
            message: error.to_string(),
        })?;
    publish_with(output, input, |mut writer| match format {
        Format::Text => match &decompiled.rendered {
            RenderOutcome::Ready(ready) => ready.write_text(&mut writer).map_err(|error| Failure {
                code: 1,
                message: error.to_string(),
            }),
            RenderOutcome::Stopped(stop) => writer
                .write_all(stop.text_prefix().as_bytes())
                .map_err(|error| Failure {
                    code: 1,
                    message: format!("text output failed: {error}"),
                }),
        },
        Format::Json => serde_json::to_writer(
            writer,
            &Publication {
                schema_version: 3,
                command,
                analysis: &decompiled,
            },
        )
        .map_err(|error| Failure {
            code: 1,
            message: format!("JSON output failed: {error}"),
        }),
    })?;
    match &decompiled.rendered {
        RenderOutcome::Ready(_) => Ok(()),
        RenderOutcome::Stopped(stop) => Err(Failure {
            code: 1,
            message: format!(
                "rendering stopped ({:?}); output is an incomplete prefix: {}",
                stop.error().kind(),
                stop.error()
            ),
        }),
    }
}
