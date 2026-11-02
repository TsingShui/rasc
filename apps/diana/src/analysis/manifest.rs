use super::archive::{EntryRecord, read_entry};
use super::axml::{DecodedManifest, ManifestError, ManifestLimits, decode};
use super::input::{ArchiveProblem, InputError, InputKind};
use super::request::RequestScope;

/// The physical entry name the manifest command selects. Selection uses physical
/// directory order, so a later duplicate wins regardless of worker completion.
const MANIFEST_NAME: &str = "AndroidManifest.xml";

#[derive(Debug, thiserror::Error)]
pub enum ManifestSelectionError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error("manifest requires a ZIP/APK input")]
    NotArchive,
    #[error("the input has no {MANIFEST_NAME} entry")]
    Missing,
    #[error("manifest entry cannot be read: {0:?}")]
    Entry(ArchiveProblem),
    #[error(transparent)]
    Decode(#[from] ManifestError),
}

impl ManifestSelectionError {
    pub fn kind(&self) -> diana_decomp::FailureKind {
        match self {
            Self::Input(error) => error.kind(),
            Self::Decode(error) => error.kind(),
            Self::Entry(problem) => manifest_entry_kind(problem),
            Self::NotArchive | Self::Missing => diana_decomp::FailureKind::Unsupported,
        }
    }
}

fn manifest_entry_kind(problem: &ArchiveProblem) -> diana_decomp::FailureKind {
    use diana_decomp::FailureKind as Kind;
    match problem {
        ArchiveProblem::EntryCountExceeded { .. }
        | ArchiveProblem::TotalEntryBytesExceeded { .. }
        | ArchiveProblem::InflatedBytesExceeded { .. }
        | ArchiveProblem::Allocation(_) => Kind::BudgetExceeded,
        ArchiveProblem::UnsupportedCompression(_)
        | ArchiveProblem::EncryptedEntry
        | ArchiveProblem::UnsupportedZip64
        | ArchiveProblem::NameEncoding => Kind::Unsupported,
        _ => Kind::Malformed,
    }
}

/// The selected manifest entry and its decoded text XML.
#[derive(Debug)]
pub struct Manifest {
    entry: EntryRecord,
    decoded: DecodedManifest,
}

impl Manifest {
    pub fn entry(&self) -> &EntryRecord {
        &self.entry
    }
    pub fn xml(&self) -> &str {
        self.decoded.xml()
    }
    /// Recoverable defects. A non-empty list means the text is partial: it stays
    /// balanced XML, but the named records were skipped or replaced.
    pub fn warnings(&self) -> &[super::axml::ManifestWarning] {
        self.decoded.warnings()
    }
    pub fn is_partial(&self) -> bool {
        self.decoded.is_partial()
    }
}

/// Selects the last physical `AndroidManifest.xml` entry and decodes it.
///
/// A bare DEX, an archive without that entry, an unreadable entry and a damaged
/// AXML document keep their own classified reasons. The decoder never guesses a
/// missing record or a value outside the input.
pub fn read(
    scope: &RequestScope,
    limits: ManifestLimits,
) -> Result<Manifest, ManifestSelectionError> {
    if scope.input().kind() != InputKind::Archive {
        return Err(ManifestSelectionError::NotArchive);
    }
    let directory = scope
        .input()
        .archive()
        .ok_or(ManifestSelectionError::Entry(
            ArchiveProblem::InconsistentDirectory,
        ))?;
    let Some(entry) = directory
        .entries()
        .iter()
        .rev()
        .find(|entry| entry.name() == MANIFEST_NAME)
        .cloned()
    else {
        return Err(ManifestSelectionError::Missing);
    };
    let payload = read_entry(scope.input().bytes(), &entry, scope.input().limits())
        .map_err(ManifestSelectionError::Entry)?;
    let decoded = decode(payload.bytes(), limits)?;
    Ok(Manifest { entry, decoded })
}
