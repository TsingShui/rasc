//! Command-line front end: argument parsing and result publication.
//!
//! The commands borrow the public analysis modules; they add no second decoding
//! path. Output is written as a complete artifact: a temporary file in the target
//! directory is renamed into place, so a failed run never leaves a half file.

mod at;
mod output;
mod table;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::analysis::axml::ManifestLimits;
use crate::analysis::dex::{ClassFilter, ClassKind};
use crate::analysis::input::{InputLimits, InputOptions};
use crate::analysis::manifest;
use crate::analysis::query::{Query, QueryKind, QueryLimits, QueryPlan, Row};
use crate::analysis::request::RequestScope;

use output::{Failure, Format, publish};
use table::Record;

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
    /// List class definitions
    Classes {
        input: PathBuf,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        extends: Option<String>,
        #[arg(long)]
        implements: Option<String>,
        #[arg(long)]
        subinterface: Option<String>,
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
        #[arg(long)]
        jobs: Option<usize>,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List defined fields
    Fields {
        input: PathBuf,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        class: Option<String>,
        #[arg(long)]
        jobs: Option<usize>,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List defined methods
    Methods {
        input: PathBuf,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        class: Option<String>,
        #[arg(long)]
        jobs: Option<usize>,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List original string values
    Strings {
        input: PathBuf,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        jobs: Option<usize>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
enum KindArg {
    Class,
    Interface,
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
        Command::Classes {
            input,
            filter,
            extends,
            implements,
            subinterface,
            kind,
            jobs,
            format,
            output,
        } => query_command(
            &input,
            QueryKind::Classes,
            QueryArgs {
                filter,
                class: None,
                jobs,
                class_filter: ClassFilter {
                    extends: extends.as_deref().map(selector_units),
                    implements: implements.as_deref().map(selector_units),
                    subinterface: subinterface.as_deref().map(selector_units),
                    kind: kind.map(|value| match value {
                        KindArg::Class => ClassKind::Class,
                        KindArg::Interface => ClassKind::Interface,
                    }),
                },
            },
            format,
            output.as_deref(),
        ),
        Command::Fields {
            input,
            filter,
            class,
            jobs,
            format,
            output,
        } => query_command(
            &input,
            QueryKind::Fields,
            QueryArgs {
                filter,
                class,
                jobs,
                class_filter: ClassFilter::default(),
            },
            format,
            output.as_deref(),
        ),
        Command::Methods {
            input,
            filter,
            class,
            jobs,
            format,
            output,
        } => query_command(
            &input,
            QueryKind::Methods,
            QueryArgs {
                filter,
                class,
                jobs,
                class_filter: ClassFilter::default(),
            },
            format,
            output.as_deref(),
        ),
        Command::Strings {
            input,
            filter,
            jobs,
            format,
            output,
        } => query_command(
            &input,
            QueryKind::Strings,
            QueryArgs {
                filter,
                class: None,
                jobs,
                class_filter: ClassFilter::default(),
            },
            format,
            output.as_deref(),
        ),
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

struct QueryArgs {
    filter: Option<String>,
    class: Option<String>,
    jobs: Option<usize>,
    class_filter: ClassFilter,
}

fn selector_units(text: &str) -> Vec<u16> {
    crate::analysis::dex::class_descriptor(text)
}

fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().min(8))
        .unwrap_or(1)
}

fn query_command(
    input: &std::path::Path,
    kind: QueryKind,
    args: QueryArgs,
    format: Format,
    output: Option<&std::path::Path>,
) -> Result<(), Failure> {
    let scope = RequestScope::open(input, &options()).map_err(Failure::input)?;
    let jobs = match args.jobs {
        Some(0) => {
            return Err(Failure {
                code: 2,
                message: "--jobs must be greater than zero".to_owned(),
            });
        }
        Some(jobs) => jobs,
        None => default_jobs(),
    };
    let plan = QueryPlan {
        kind,
        filter: args
            .filter
            .as_deref()
            .map(|text| text.encode_utf16().collect()),
        class: args.class.as_deref().map(selector_units),
        class_filter: args.class_filter,
        limits: QueryLimits::default(),
        jobs,
    };
    let query = Query::new(&scope, plan).map_err(|error| Failure::query(error, 1))?;
    let images = query.images().to_vec();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut rows = Vec::new();
    query
        .run(&cancel, &mut |row| {
            rows.push(row);
            Ok(())
        })
        .map_err(|error| Failure::query(error, 1))?;
    let mut records = Vec::with_capacity(rows.len());
    for row in &rows {
        let record = match row {
            Row::Class(row) => Record::new(at::class(&images[row.image.0], row)),
            Row::Method(row) => Record::new(at::method(&images[row.image.0], row)),
            Row::Field(row) => Record::new(at::field(&images[row.image.0], row)),
            Row::String(row) => Record::new(at::string(&images[row.image.0], row))
                .with_value(table::escape_units(&row.units))
                .with_entry(images[row.image.0].label.clone()),
            Row::Entry(_) => continue,
        };
        records.push(record);
    }
    let columns: &[&str] = match kind {
        QueryKind::Strings => &["at", "value", "entry"],
        _ => &["at"],
    };
    let command = match kind {
        QueryKind::Classes => "classes",
        QueryKind::Fields => "fields",
        QueryKind::Methods => "methods",
        QueryKind::Strings => "strings",
        QueryKind::Entries => "entries",
    };
    table::publish_records(&records, columns, command, format, output)
}
