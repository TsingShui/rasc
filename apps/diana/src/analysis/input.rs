use std::io::{ErrorKind as IoErrorKind, Read};
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::archive::ArchiveDirectory;

/// A byte length in the original input file, distinct from entry and source
/// lengths that also appear in the application.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct InputByteLen(pub u64);

/// A position in a physical ZIP record, not a DEX class-def or method index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntryIndex(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputKind {
    /// The whole leased snapshot is one DEX container.
    BareDex,
    /// The snapshot is a ZIP archive whose entries are available.
    Archive,
    /// A leased regular file that is neither of the above.
    Opaque,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProblem {
    /// No end-of-central-directory record in the admissible tail.
    MissingDirectory,
    /// A record signature, count or length field contradicts the actual bytes.
    InconsistentDirectory,
    /// A record ends outside the leased bytes.
    OutOfRange,
    /// An entry name is not valid UTF-8 metadata.
    NameEncoding,
    /// The requested entry does not exist at that physical index.
    UnknownEntry,
    /// A stored entry's payload, size or CRC contradicts its record.
    DamagedEntry,
    /// The entry uses a compression method this module cannot read.
    UnsupportedCompression(u16),
    /// The entry declares encryption.
    EncryptedEntry,
    /// ZIP64 record fields are outside the supported container profile.
    UnsupportedZip64,
    /// The deflate stream is truncated, over-long or otherwise undecodable.
    DecompressionFailed,
    /// Reserved table capacity could not be allocated.
    Allocation(std::collections::TryReserveError),
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("input I/O failed: {0}")]
    Io(#[source] std::io::Error),
    #[error("input is not a regular file: {0}")]
    NotRegularFile(PathBuf),
    #[error("input resolves outside the allowed root: {0}")]
    OutsideRoot(PathBuf),
    #[error("input changed while it was read")]
    Changed,
    #[error("archive is malformed: {0:?}")]
    MalformedArchive(ArchiveProblem),
    #[error("input size is not representable on this platform")]
    PlatformCapacity,
    #[error("input allocation failed: {0}")]
    Allocation(#[source] std::collections::TryReserveError),
    #[error("input is not a DEX container or archive")]
    NotDex,
}

impl InputError {
    /// One failure kind for callers that must classify an input-side error.
    /// I/O, path policy and an unsupported container are `Unsupported`; damaged
    /// archive records are `Malformed`; an exceeded configured bound or an actual
    /// allocation failure is `BudgetExceeded`.
    pub fn kind(&self) -> diana_decomp::FailureKind {
        use diana_decomp::FailureKind as Kind;
        match self {
            Self::PlatformCapacity | Self::Allocation(_) => Kind::ResourceExhausted,
            Self::Io(_)
            | Self::NotRegularFile(_)
            | Self::OutsideRoot(_)
            | Self::Changed
            | Self::NotDex => Kind::Unsupported,
            Self::MalformedArchive(problem) => match problem {
                ArchiveProblem::Allocation(_) => Kind::ResourceExhausted,
                ArchiveProblem::UnsupportedCompression(_)
                | ArchiveProblem::EncryptedEntry
                | ArchiveProblem::UnsupportedZip64
                | ArchiveProblem::NameEncoding => Kind::Unsupported,
                ArchiveProblem::MissingDirectory
                | ArchiveProblem::InconsistentDirectory
                | ArchiveProblem::OutOfRange
                | ArchiveProblem::UnknownEntry
                | ArchiveProblem::DamagedEntry
                | ArchiveProblem::DecompressionFailed => Kind::Malformed,
            },
        }
    }
}

impl From<std::io::Error> for InputError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Borrowed bytes inside a lease or one of its entry views.
pub trait BytesSource {
    fn bytes(&self) -> &[u8];
}

/// The identity of one leased input. It is request-local and never a cache key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputIdentity {
    path: PathBuf,
    len: InputByteLen,
}

impl InputIdentity {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn len(&self) -> InputByteLen {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len.0 == 0
    }
}

/// An owned immutable snapshot of one input and its restricted entry metadata.
///
/// The snapshot is complete before the lease exists, so a later truncation or
/// replacement of the source file does not change it. Dropping the lease releases
/// only this owned copy; the user's file is never modified or deleted.
#[derive(Debug)]
pub struct InputLease {
    identity: InputIdentity,
    kind: InputKind,
    bytes: Vec<u8>,
    archive: Option<ArchiveDirectory>,
}

impl BytesSource for InputLease {
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl InputLease {
    pub fn identity(&self) -> &InputIdentity {
        &self.identity
    }
    pub fn kind(&self) -> InputKind {
        self.kind
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn archive(&self) -> Option<&ArchiveDirectory> {
        self.archive.as_ref()
    }
}

/// Path policy and explicit bounds for one lease.
#[derive(Clone, Debug, Default)]
pub struct InputOptions {
    /// When set, the resolved input must stay inside this directory. The default
    /// is to follow the platform's normal path resolution for a regular file.
    pub allowed_root: Option<PathBuf>,
}

/// Leases one regular file after applying the path and size policy.
///
/// Non-regular files (directories, devices, FIFOs, sockets) are rejected from
/// metadata before any open, so a FIFO cannot block the request. A file that
/// changes while it is read yields `Changed` instead of a partial snapshot.
pub fn open_input(path: &Path, options: &InputOptions) -> Result<InputLease, InputError> {
    let resolved = std::fs::canonicalize(path).map_err(InputError::Io)?;
    if let Some(root) = &options.allowed_root {
        let root = std::fs::canonicalize(root).map_err(InputError::Io)?;
        if !resolved.starts_with(&root) {
            return Err(InputError::OutsideRoot(resolved));
        }
    }
    let metadata = std::fs::metadata(&resolved).map_err(InputError::Io)?;
    if !metadata.is_file() {
        return Err(InputError::NotRegularFile(resolved));
    }
    let len = metadata.len();
    let mut file = std::fs::File::open(&resolved).map_err(InputError::Io)?;
    if file.metadata().map_err(InputError::Io)?.len() != len {
        return Err(InputError::Changed);
    }
    let bytes = read_snapshot(&mut file, len)?;
    if file.metadata().map_err(InputError::Io)?.len() != len {
        return Err(InputError::Changed);
    }
    let kind = classify(&bytes);
    let archive = match kind {
        InputKind::Archive => {
            Some(ArchiveDirectory::parse(&bytes).map_err(InputError::MalformedArchive)?)
        }
        _ => None,
    };
    Ok(InputLease {
        identity: InputIdentity {
            path: resolved,
            len: InputByteLen(len),
        },
        kind,
        bytes,
        archive,
    })
}

fn read_snapshot(file: &mut impl Read, len: u64) -> Result<Vec<u8>, InputError> {
    let size = usize::try_from(len).map_err(|_| InputError::PlatformCapacity)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(InputError::Allocation)?;
    let mut remaining = size;
    let mut buffer = [0u8; 64 * 1024];
    while remaining != 0 {
        let want = remaining.min(buffer.len());
        match file.read(&mut buffer[..want]) {
            Ok(0) => return Err(InputError::Changed),
            Ok(read) => {
                bytes.extend_from_slice(&buffer[..read]);
                remaining -= read;
            }
            Err(error) if error.kind() == IoErrorKind::Interrupted => {}
            Err(error) => return Err(InputError::Io(error)),
        }
    }
    if bytes.len() != size {
        return Err(InputError::Changed);
    }
    Ok(bytes)
}

fn classify(bytes: &[u8]) -> InputKind {
    if bytes.starts_with(b"dex\n") {
        InputKind::BareDex
    } else if bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x06\x06")
    {
        InputKind::Archive
    } else {
        InputKind::Opaque
    }
}
