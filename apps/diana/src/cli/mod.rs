//! Command-line front end: argument parsing and result publication.
//!
//! The commands borrow the public analysis modules; they add no second decoding
//! path. Output is written as a complete artifact: a temporary file in the target
//! directory is renamed into place, so a failed run never leaves a half file.

mod extract_cli;
mod get;
mod output;
mod table;
mod xref;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::analysis::at;
use crate::analysis::axml::ManifestLimits;
use crate::analysis::dex::{ClassFilter, ClassKind};
use crate::analysis::input::InputOptions;
use crate::analysis::manifest;
use crate::analysis::query::{Query, QueryKind, QueryLimits, QueryPlan, Row};
use crate::analysis::request::RequestScope;

use output::{Failure, Format, publish};
use table::Record;

#[derive(Parser)]
#[command(name = "diana", about = "Native APK/DEX analysis", version)]
struct Cli {
    #[command(flatten)]
    limits: LimitArgs,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone, Copy, Debug, Default)]
struct LimitArgs {
    #[arg(long, global = true)]
    max_input_bytes: Option<u64>,
    #[arg(long, global = true)]
    max_inflated_bytes: Option<u64>,
    #[arg(long, global = true)]
    max_request_work_steps: Option<u64>,
    #[arg(long, global = true)]
    max_request_allocation_bytes: Option<u64>,
    #[arg(long, global = true)]
    max_work_steps: Option<u64>,
    #[arg(long, global = true)]
    max_method_output_bytes: Option<u64>,
    #[arg(long, global = true)]
    max_report_output_bytes: Option<u64>,
}

impl LimitArgs {
    fn limits(&self) -> crate::analysis::limits::RequestLimits {
        crate::analysis::limits::RequestLimits {
            max_input_bytes: self.max_input_bytes,
            max_inflated_bytes: self.max_inflated_bytes,
            max_request_work_steps: self.max_request_work_steps,
            max_request_allocation_bytes: self.max_request_allocation_bytes,
            max_work_steps: self.max_work_steps,
            max_method_output_bytes: self.max_method_output_bytes,
            max_report_output_bytes: self.max_report_output_bytes,
        }
    }
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
    /// Decompile one class
    GetClass {
        input: PathBuf,
        selector: String,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Decompile one method
    GetMethod {
        input: PathBuf,
        selector: String,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Extract one physical entry
    Extract {
        input: PathBuf,
        entry: String,
        #[arg(long)]
        entry_index: Option<usize>,
        #[arg(short, long)]
        output: PathBuf,
        /// Keep the original payload bytes even for binary XML
        #[arg(long)]
        raw: bool,
        /// Replace an existing ordinary output file
        #[arg(long)]
        force: bool,
        /// Exit non-zero when a decoded document is partial
        #[arg(long)]
        strict: bool,
    },
    /// Find original reference sites
    Xref {
        input: PathBuf,
        /// string | type | method | field
        kind: String,
        target: String,
        #[arg(long)]
        class: Option<String>,
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
    let limits = cli.limits.limits();
    let result = match cli.command {
        Command::Entries {
            input,
            format,
            output,
        } => entries(&input, format, output.as_deref(), limits),
        Command::Manifest {
            input,
            format,
            output,
            strict,
        } => manifest_command(&input, format, output.as_deref(), strict, limits),
        Command::GetClass {
            input,
            selector,
            format,
            output,
        } => get::run(
            &input,
            &selector,
            get::Kind::Class,
            format,
            output.as_deref(),
            limits,
        ),
        Command::GetMethod {
            input,
            selector,
            format,
            output,
        } => get::run(
            &input,
            &selector,
            get::Kind::Method,
            format,
            output.as_deref(),
            limits,
        ),
        Command::Extract {
            input,
            entry,
            entry_index,
            output,
            raw,
            force,
            strict,
        } => extract_cli::run(
            &input,
            &entry,
            entry_index,
            &output,
            raw,
            force,
            strict,
            limits,
        ),
        Command::Xref {
            input,
            kind,
            target,
            class,
            jobs,
            format,
            output,
        } => xref::run(
            &input,
            &kind,
            &target,
            class.as_deref(),
            match jobs {
                Some(0) => {
                    return std::process::ExitCode::from(2);
                }
                Some(jobs) => jobs,
                None => default_jobs(),
            },
            format,
            output.as_deref(),
            limits,
        ),
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
            limits,
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
            limits,
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
            limits,
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
            limits,
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

fn options(limits: crate::analysis::limits::RequestLimits) -> InputOptions {
    InputOptions {
        limits: limits.input(),
        allowed_root: None,
    }
}

fn entries(
    input: &std::path::Path,
    format: Format,
    output: Option<&std::path::Path>,
    limits: crate::analysis::limits::RequestLimits,
) -> Result<(), Failure> {
    let scope = RequestScope::open(input, &options(limits)).map_err(Failure::input)?;
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
    publish(
        output,
        text.as_bytes(),
        format,
        limits.max_report_output_bytes,
    )
}

fn manifest_command(
    input: &std::path::Path,
    format: Format,
    output: Option<&std::path::Path>,
    strict: bool,
    limits: crate::analysis::limits::RequestLimits,
) -> Result<(), Failure> {
    let scope = RequestScope::open(input, &options(limits)).map_err(Failure::input)?;
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
    publish(
        output,
        text.as_bytes(),
        format,
        limits.max_report_output_bytes,
    )?;
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
    limits: crate::analysis::limits::RequestLimits,
) -> Result<(), Failure> {
    let scope = RequestScope::open(input, &options(limits)).map_err(Failure::input)?;
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
        let image_index = match row {
            Row::Class(row) => row.image.0,
            Row::Method(row) => row.image.0,
            Row::Field(row) => row.image.0,
            Row::String(row) => row.image.0,
            Row::Entry(row) => row.image.0,
        };
        let image = &images[image_index];
        let entry = image.entry.map(|entry| entry.0);
        let record = match row {
            Row::Class(row) => Record::new(at::class(entry, 0, row.class_def.0, &row.descriptor)),
            Row::Method(row) => Record::new(at::method(
                entry,
                0,
                row.class_def.0,
                row.method.0,
                &row.class_descriptor,
                &row.name,
                &row.prototype,
            )),
            Row::Field(row) => Record::new(at::field(
                entry,
                0,
                row.class_def.0,
                row.field.0,
                &row.class_descriptor,
                &row.name,
                &row.field_type,
            )),
            Row::String(row) => Record::new(at::string(entry, 0, row.string.0))
                .with_value(at::escape_units(&row.units))
                .with_entry(image.label.clone()),
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
    table::publish_records(
        &records,
        columns,
        command,
        format,
        output,
        limits.max_report_output_bytes,
    )
}
