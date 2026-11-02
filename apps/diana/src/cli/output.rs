//! Result publication: text or JSON bytes written as one atomic artifact.

use std::io::Write;
use std::path::Path;

use clap::ValueEnum;

use crate::analysis::input::InputError;
use crate::analysis::query::QueryError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Format {
    Text,
    Json,
}

/// A classified command failure with its process exit code.
pub struct Failure {
    pub code: u8,
    pub message: String,
}

impl Failure {
    pub fn input(error: InputError) -> Self {
        Self {
            code: 1,
            message: error.to_string(),
        }
    }
    pub fn query(error: QueryError, code: u8) -> Self {
        Self {
            code,
            message: error.to_string(),
        }
    }
}

/// Writes a complete payload to stdout or atomically to the requested file.
pub fn publish(
    output: Option<&Path>,
    bytes: &[u8],
    format: Format,
    maximum: Option<u64>,
) -> Result<(), Failure> {
    crate::analysis::limits::enforce_output(bytes, maximum, "max_report_output_bytes").map_err(
        |error| Failure {
            code: 1,
            message: error.to_string(),
        },
    )?;
    match output {
        Some(path) => {
            let directory = path.parent().unwrap_or_else(|| Path::new("."));
            let (mut temporary, temporary_path) =
                tempfile_in(directory).map_err(|error| Failure {
                    code: 1,
                    message: format!("cannot create a temporary output file: {error}"),
                })?;
            temporary
                .write_all(bytes)
                .and_then(|()| temporary.flush())
                .map_err(|error| {
                    let _ = std::fs::remove_file(&temporary_path);
                    Failure {
                        code: 1,
                        message: format!("cannot write the result: {error}"),
                    }
                })?;
            drop(temporary);
            std::fs::rename(&temporary_path, path).map_err(|error| {
                let _ = std::fs::remove_file(&temporary_path);
                Failure {
                    code: 1,
                    message: format!("cannot publish the result: {error}"),
                }
            })
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            let broken = |error: &std::io::Error| error.kind() == std::io::ErrorKind::BrokenPipe;
            if let Err(error) = stdout.write_all(bytes) {
                // A closed pipe is the normal end of a Unix filter, not a failure.
                if broken(&error) {
                    return Ok(());
                }
                return Err(Failure {
                    code: 1,
                    message: format!("cannot write to stdout: {error}"),
                });
            }
            if format == Format::Text
                && !bytes.ends_with(b"\n")
                && let Err(error) = stdout.write_all(b"\n")
                && !broken(&error)
            {
                return Err(Failure {
                    code: 1,
                    message: format!("cannot write to stdout: {error}"),
                });
            }
            match stdout.flush() {
                Ok(()) => Ok(()),
                Err(error) if broken(&error) => Ok(()),
                Err(error) => Err(Failure {
                    code: 1,
                    message: format!("cannot write to stdout: {error}"),
                }),
            }
        }
    }
}

fn tempfile_in(directory: &Path) -> std::io::Result<(std::fs::File, std::path::PathBuf)> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    for _ in 0..32 {
        let ordinal = COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = directory.join(format!(".diana-{}-{ordinal}.tmp", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((file, candidate)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no unique temporary name",
    ))
}
