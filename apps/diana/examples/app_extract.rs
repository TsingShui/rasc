//! Development caller for the application extract module (E2E extract seams).
//!
//! Usage: app_extract <input> <entry> [--entry-index N] [-o target] [--force]
//!        [--max-inflated-bytes N] [--raw]
use std::path::{Path, PathBuf};

use diana::analysis::axml::ManifestLimits;
use diana::analysis::extract::{self, ExtractError, Request};
use diana::analysis::input::ArchiveProblem;
use diana::analysis::input::{InputLimits, InputOptions};
use diana::analysis::request::RequestScope;
use serde_json::json;

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("input");
    let entry = args.next().expect("entry");
    let mut index = None;
    let mut output: Option<PathBuf> = None;
    let mut force = false;
    let mut inflated = None;
    let mut raw = false;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--entry-index" => index = Some(args.next().expect("index").parse().expect("number")),
            "-o" => output = Some(PathBuf::from(args.next().expect("target"))),
            "--force" => force = true,
            "--raw" => raw = true,
            "--max-inflated-bytes" => {
                inflated = Some(args.next().expect("bound").parse().expect("number"))
            }
            other => panic!("unknown flag {other}"),
        }
    }
    let target = output.expect("target path");
    let scope = RequestScope::open(
        Path::new(&input),
        &InputOptions {
            limits: InputLimits {
                max_inflated_bytes: inflated,
                ..InputLimits::default()
            },
            allowed_root: None,
        },
    );
    let result = match scope {
        Ok(scope) => extract::prepare(
            &scope,
            &Request {
                name: &entry,
                index,
                raw,
                limits: ManifestLimits::default(),
            },
        )
        .and_then(|extracted| {
            extract::publish(&extracted, &target, Path::new(&input), force).map(|()| extracted)
        }),
        Err(error) => Err(ExtractError::Input(error)),
    };
    match result {
        Ok(extracted) => {
            println!(
                "{}",
                json!({
                    "outcome": "success",
                    "entry": extracted.entry,
                    "decoded": extracted.decoded,
                    "bytes": extracted.bytes.len(),
                    "warnings": extracted.warnings.len(),
                })
            );
        }
        Err(error) => {
            println!(
                "{}",
                json!({"outcome": "error", "error_kind": kind(&error), "detail": error.to_string()})
            );
        }
    }
}

fn kind(error: &ExtractError) -> &'static str {
    match error {
        ExtractError::Ambiguous { .. } => "ambiguous_entry",
        ExtractError::Missing(_) => "missing_entry",
        ExtractError::IndexMismatch { .. } => "index_mismatch",
        ExtractError::UnsafeTarget => "unsafe_target",
        ExtractError::TargetExists => "target_exists",
        ExtractError::NotArchive => "not_archive",
        ExtractError::Io(_) => "io",
        ExtractError::Entry(problem) => match problem {
            ArchiveProblem::DamagedEntry => "crc_mismatch",
            ArchiveProblem::DecompressionFailed => "decompression_failed",
            ArchiveProblem::UnsupportedCompression(_) => "unsupported_compression",
            ArchiveProblem::EncryptedEntry => "encrypted_entry",
            ArchiveProblem::InflatedBytesExceeded { .. }
            | ArchiveProblem::EntryCountExceeded { .. }
            | ArchiveProblem::TotalEntryBytesExceeded { .. }
            | ArchiveProblem::Allocation(_) => "budget_exceeded",
            _ => "archive_error",
        },
        ExtractError::Input(error) => input_kind(error),
        ExtractError::Engine(_) => "engine_error",
    }
}

fn input_kind(error: &diana::analysis::input::InputError) -> &'static str {
    match error {
        diana::analysis::input::InputError::BudgetExceeded { .. } => "budget_exceeded",
        _ => "input_error",
    }
}
