//! Command-line front end: argument parsing and result publication.
//!
//! The commands borrow the public analysis modules; they add no second decoding
//! path. Output is written as a complete artifact: a temporary file in the target
//! directory is renamed into place, so a failed run never leaves a half file.

mod output;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::analysis::axml::ManifestLimits;
use crate::analysis::input::{InputLimits, InputOptions};
use crate::analysis::manifest;
use crate::analysis::query::{Query, QueryKind, QueryLimits, QueryPlan, Row};
use crate::analysis::request::RequestScope;

use output::{Failure, Format, publish};

#[derive(Parser)]
#[command(name = "diana", about = "Native APK/DEX analysis", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List physical archive entries
    Entries {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Decode the binary AndroidManifest.xml
    Manifest {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Exit non-zero when the document is partial
        #[arg(long)]
        strict: bool,
    },
}

/// Runs one command and maps its classified failure to an exit code.
pub fn run() -> std::process::ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Entries {
            input,
            format,
            output,
        } => entries(&input, format, output.as_deref()),
        Command::Manifest {
            input,
            format,
            output,
            strict,
        } => manifest_command(&input, format, output.as_deref(), strict),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("diana: {}", failure.message);
            std::process::ExitCode::from(failure.code)
        }
    }
}

fn options() -> InputOptions {
    InputOptions {
        limits: InputLimits::default(),
        allowed_root: None,
    }
}

fn entries(
    input: &std::path::Path,
    format: Format,
    output: Option<&std::path::Path>,
) -> Result<(), Failure> {
    let scope = RequestScope::open(input, &options()).map_err(Failure::input)?;
    let query = Query::new(
        &scope,
        QueryPlan {
            kind: QueryKind::Entries,
            filter: None,
            class: None,
            class_filter: Default::default(),
            limits: QueryLimits::default(),
            jobs: 1,
        },
    )
    .map_err(|error| Failure::query(error, 1))?;
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut rows = Vec::new();
    query
        .run(&cancel, &mut |row| {
            rows.push(row);
            Ok(())
        })
        .map_err(|error| Failure::query(error, 1))?;
    let text = match format {
        Format::Text => {
            let mut text = String::from("name\tcompressed_size\tuncompressed_size\tcompression\n");
            for row in &rows {
                if let Row::Entry(row) = row {
                    text.push_str(&format!(
                        "{}\t{}\t{}\t{}\n",
                        row.name, row.compressed_size, row.uncompressed_size, row.method
                    ));
                }
            }
            text
        }
        Format::Json => {
            let entries: Vec<_> = rows
                .iter()
                .filter_map(|row| match row {
                    Row::Entry(row) => Some(serde_json::json!({
                        "name": row.name,
                        "compressed_size": row.compressed_size,
                        "uncompressed_size": row.uncompressed_size,
                        "compression": row.method,
                    })),
                    _ => None,
                })
                .collect();
            serde_json::json!({
                "schema_version": 1,
                "command": "entries",
                "entries": entries,
            })
            .to_string()
        }
    };
    publish(output, text.as_bytes(), format)
}

fn manifest_command(
    input: &std::path::Path,
    format: Format,
    output: Option<&std::path::Path>,
    strict: bool,
) -> Result<(), Failure> {
    let scope = RequestScope::open(input, &options()).map_err(Failure::input)?;
    let manifest = match manifest::read(&scope, ManifestLimits::default()) {
        Ok(manifest) => manifest,
        Err(error) => {
            let code = match error {
                manifest::ManifestSelectionError::Missing => 3,
                _ => 1,
            };
            return Err(Failure {
                code,
                message: error.to_string(),
            });
        }
    };
    for warning in manifest.warnings() {
        eprintln!(
            "diana: warning: byte {}: {}",
            warning.offset,
            warning.kind.describe()
        );
    }
    let text = match format {
        Format::Text => manifest.xml().to_owned(),
        Format::Json => serde_json::json!({
            "schema_version": 1,
            "command": "manifest",
            "entry": manifest.entry().name(),
            "status": if manifest.is_partial() { "partial" } else { "complete" },
            "xml": manifest.xml(),
            "warnings": manifest.warnings().iter().map(|warning| serde_json::json!({
                "code": warning.kind.code(),
                "offset": warning.offset,
                "detail": warning.kind.describe(),
            })).collect::<Vec<_>>(),
        })
        .to_string(),
    };
    publish(output, text.as_bytes(), format)?;
    if strict && manifest.is_partial() {
        return Err(Failure {
            code: 1,
            message: "manifest is partial and --strict was requested".to_owned(),
        });
    }
    Ok(())
}
