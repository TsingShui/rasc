//! Physical entry extraction with an optional binary-XML decode and safe publish.
//!
//! The output is derived only from the selected physical record: the decompressed
//! payload is validated for size and CRC before any decode, a binary XML payload is
//! decoded by default, and the file is published atomically from a private
//! temporary name in the target directory.

use std::path::{Path, PathBuf};

use super::archive::read_entry;
use super::axml::{ManifestLimits, ManifestWarning, decode};
use super::input::{ArchiveProblem, EntryIndex, InputError, InputKind};
use super::request::RequestScope;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Engine(#[from] diana_decomp::input::InputError),
    #[error("extract requires a ZIP/APK input")]
    NotArchive,
    #[error("the input has no entry named {0}")]
    Missing(String),
    #[error("entry name {name} matches physical indices {indices:?}; pass --entry-index")]
    Ambiguous { name: String, indices: Vec<usize> },
    #[error("entry index {index} does not name {name}")]
    IndexMismatch { name: String, index: usize },
    #[error("entry cannot be read: {0:?}")]
    Entry(ArchiveProblem),
    #[error("output path is a directory, a symbolic link, or aliases the input")]
    UnsafeTarget,
    #[error("output already exists; pass --force to replace an ordinary file")]
    TargetExists,
    #[error("cannot publish the output: {0}")]
    Io(#[from] std::io::Error),
}

impl ExtractError {
    pub fn code(&self) -> u8 {
        match self {
            Self::Ambiguous { .. } => 4,
            Self::Missing(_) => 3,
            _ => 1,
        }
    }
}

/// A selected entry and its published bytes.
#[derive(Debug)]
pub struct Extracted {
    pub entry: String,
    pub bytes: Vec<u8>,
    pub decoded: bool,
    pub warnings: Vec<ManifestWarning>,
}

pub struct Request<'a> {
    pub name: &'a str,
    pub index: Option<usize>,
    /// Keep the original decompressed payload even when it is binary XML.
    pub raw: bool,
    pub limits: ManifestLimits,
}

/// Selects one physical entry, validates its payload and decodes binary XML unless
/// `raw` was requested.
pub fn prepare(scope: &RequestScope, request: &Request) -> Result<Extracted, ExtractError> {
    if scope.input().kind() != InputKind::Archive {
        return Err(ExtractError::NotArchive);
    }
    let directory = scope
        .input()
        .archive()
        .ok_or(ExtractError::Entry(ArchiveProblem::InconsistentDirectory))?;
    let mut matches = Vec::new();
    for entry in directory.entries() {
        if entry.name() == request.name {
            matches.push(entry);
        }
    }
    let selected = match (request.index, matches.as_slice()) {
        (Some(index), _) => {
            let Some(entry) = directory.entry(EntryIndex(index)) else {
                return Err(ExtractError::Missing(request.name.to_owned()));
            };
            if entry.name() != request.name {
                return Err(ExtractError::IndexMismatch {
                    name: request.name.to_owned(),
                    index,
                });
            }
            entry
        }
        (None, []) => return Err(ExtractError::Missing(request.name.to_owned())),
        (None, [entry]) => entry,
        (None, many) => {
            return Err(ExtractError::Ambiguous {
                name: request.name.to_owned(),
                indices: many.iter().map(|entry| entry.index().0).collect(),
            });
        }
    };
    let payload = read_entry(scope.input().bytes(), selected, scope.input().limits())
        .map_err(ExtractError::Entry)?;
    let original = payload.bytes();
    if request.raw || !is_binary_xml(original) {
        return Ok(Extracted {
            entry: selected.name().to_owned(),
            bytes: original.to_vec(),
            decoded: false,
            warnings: Vec::new(),
        });
    }
    let decoded = decode(original, request.limits)
        .map_err(|_| ExtractError::Entry(ArchiveProblem::DamagedEntry))?;
    Ok(Extracted {
        entry: selected.name().to_owned(),
        bytes: decoded.xml().as_bytes().to_vec(),
        decoded: true,
        warnings: decoded.warnings().to_vec(),
    })
}

fn is_binary_xml(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x03 && bytes[1] == 0x00
}

/// Publishes the prepared payload. The target must be an ordinary file path; an
/// existing file needs `force`, and a directory, a symbolic link, the input itself
/// or a hardlink alias are rejected.
pub fn publish(
    extracted: &Extracted,
    target: &Path,
    input: &Path,
    force: bool,
) -> Result<(), ExtractError> {
    if target.is_dir() {
        return Err(ExtractError::UnsafeTarget);
    }
    match std::fs::symlink_metadata(target) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(ExtractError::UnsafeTarget);
            }
            if aliases_input(target, input) {
                return Err(ExtractError::UnsafeTarget);
            }
            if !force {
                return Err(ExtractError::TargetExists);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(ExtractError::Io(error)),
    }
    let directory = target.parent().unwrap_or_else(|| Path::new("."));
    let (mut temporary, temporary_path) = temporary_in(directory)?;
    let written = (|| -> std::io::Result<()> {
        use std::io::Write;
        temporary.write_all(&extracted.bytes)?;
        temporary.flush()
    })();
    drop(temporary);
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(ExtractError::Io(error));
    }
    if force {
        if let Err(error) = std::fs::rename(&temporary_path, target) {
            let _ = std::fs::remove_file(&temporary_path);
            return Err(ExtractError::Io(error));
        }
        return Ok(());
    }
    // Publication without `force` must not have a check-then-rename race: linking
    // the temporary name to the target fails atomically when it already exists.
    match std::fs::hard_link(&temporary_path, target) {
        Ok(()) => {
            let _ = std::fs::remove_file(&temporary_path);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&temporary_path);
            Err(ExtractError::TargetExists)
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temporary_path);
            Err(ExtractError::Io(error))
        }
    }
}

#[cfg(unix)]
fn aliases_input(target: &Path, input: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(target_meta) = std::fs::metadata(target) else {
        return false;
    };
    let Ok(input_meta) = std::fs::metadata(input) else {
        return false;
    };
    if target_meta.ino() == input_meta.ino() && target_meta.dev() == input_meta.dev() {
        return true;
    }
    target_meta.nlink() > 1
}

#[cfg(not(unix))]
fn aliases_input(_target: &Path, _input: &Path) -> bool {
    false
}

fn temporary_in(directory: &Path) -> std::io::Result<(std::fs::File, PathBuf)> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    for _ in 0..32 {
        let ordinal = COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = directory.join(format!(
            ".diana-extract-{}-{ordinal}.tmp",
            std::process::id()
        ));
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
