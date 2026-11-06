use diana_decomp::{FailureKind, RequestError};

use super::apk::DexRoot;
use super::archive::EntryRecord;
use super::dex::{self, ClassRow, FieldRow, ImageIndex, MethodRow, StringRow, contains};
use super::input::{EntryIndex, InputError};
use super::request::RequestScope;

/// One physical ZIP record, listed in directory order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryRow {
    pub entry: EntryIndex,
    pub name: String,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub method: u16,
}

/// One result row. Every variant keeps the original DEX/entry identity that a
/// caller can pass back, independent of display order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Row {
    Entry(EntryRow),
    Class(ClassRow),
    Method(MethodRow),
    Field(FieldRow),
    String(StringRow),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryKind {
    Entries,
    Classes,
    Methods,
    Fields,
    Strings,
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error(transparent)]
    Input(#[from] InputError),
    /// A checked DEX read failure; its own kind survives unchanged.
    #[error(transparent)]
    Engine(#[from] diana_decomp::input::InputError),
    /// A checked method-read failure; its own kind survives unchanged.
    #[error(transparent)]
    Method(#[from] diana_decomp::input::MethodError),
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error("query worker stopped before delivering its rows")]
    WorkerLost,
    #[error("entries lists physical archive records only; a bare DEX has none")]
    NotArchive,
    #[error("the job count must be positive")]
    InvalidJobs,
}

impl QueryError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(error) => error.kind(),
            Self::Engine(error) => error.kind(),
            Self::Method(error) => error.kind(),
            Self::Request(error) => error.kind(),
            Self::WorkerLost => FailureKind::InternalInvariant,
            Self::NotArchive | Self::InvalidJobs => FailureKind::Unsupported,
        }
    }
}

/// One root image's display metadata. It is not an original identity by itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageInfo {
    pub index: ImageIndex,
    pub label: String,
    pub entry: Option<EntryIndex>,
}

/// A query request over one leased input.
#[derive(Clone, Debug)]
pub struct QueryPlan {
    pub kind: QueryKind,
    /// Literal UTF-16 substring over the primary column text.
    pub filter: Option<Vec<u16>>,
    /// Exact class descriptor restricting methods/fields to one definition.
    pub class: Option<Vec<u16>>,
    /// Direct hierarchy relations restricting `classes`.
    pub class_filter: super::dex::ClassFilter,
    pub jobs: usize,
}

/// Prepared query: root images are resolved once, rows stream in completion order.
pub struct Query<'a> {
    roots: Vec<DexRoot<'a>>,
    images: Vec<ImageInfo>,
    kind: QueryKind,
    filter: Option<Vec<u16>>,
    class: Option<Vec<u16>>,
    class_filter: super::dex::ClassFilter,
    jobs: usize,
    entries: Option<&'a [EntryRecord]>,
}

impl<'a> Query<'a> {
    /// Resolves the root images and their display labels. No DEX analysis runs
    /// here, and no row is produced yet.
    pub fn new(scope: &'a RequestScope, plan: QueryPlan) -> Result<Self, QueryError> {
        if plan.jobs == 0 {
            return Err(QueryError::InvalidJobs);
        }
        let roots = if plan.kind == QueryKind::Entries {
            Vec::new()
        } else {
            scope.roots()?
        };
        let mut images = Vec::new();
        for (ordinal, root) in roots.iter().enumerate() {
            images.try_reserve(1).map_err(|_| QueryError::WorkerLost)?;
            images.push(ImageInfo {
                index: ImageIndex(ordinal),
                label: root.label().to_owned(),
                entry: root.entry().map(EntryRecord::index),
            });
        }
        let entries = scope.input().archive().map(|directory| directory.entries());
        Ok(Self {
            roots,
            images,
            kind: plan.kind,
            filter: plan.filter,
            class: plan.class,
            class_filter: plan.class_filter,
            jobs: plan.jobs,
            entries,
        })
    }

    pub fn images(&self) -> &[ImageInfo] {
        &self.images
    }

    /// Streams every matching row. Rows arrive in worker completion order; the
    /// caller must not rely on a sequence. Producer/sink failures return an error;
    /// a delivered prefix is not a complete result.
    pub fn run(
        &self,
        sink: &mut impl FnMut(Row) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if self.kind == QueryKind::Entries {
            let entries = self.entries.ok_or(QueryError::NotArchive)?;
            for entry in entries {
                let row = EntryRow {
                    entry: entry.index(),
                    name: entry.name().to_owned(),
                    compressed_size: entry.compressed_size().0,
                    uncompressed_size: entry.uncompressed_size().0,
                    method: entry.method(),
                };
                if let Some(needle) = &self.filter
                    && !contains(&row.name.encode_utf16().collect::<Vec<_>>(), needle)
                {
                    continue;
                }

                sink(Row::Entry(row))?;
            }
            return Ok(());
        }
        let kind = self.kind;
        super::runner::run(
            self.roots.len(),
            self.jobs,
            |ordinal, emit| {
                let root = &self.roots[ordinal];

                let image = ImageIndex(ordinal);
                let filter = self.filter.as_deref();
                let class = self.class.as_deref();
                dex::visit_logical(root.bytes(), |logical, index| match kind {
                    QueryKind::Entries => Ok(()),
                    QueryKind::Classes => {
                        dex::classes(&index, image, logical, filter, &self.class_filter, emit)
                    }
                    QueryKind::Methods => dex::methods(&index, image, logical, class, filter, emit),
                    QueryKind::Fields => dex::fields(&index, image, logical, class, filter, emit),
                    QueryKind::Strings => dex::strings(&index, image, logical, filter, emit),
                })
            },
            sink,
        )
    }
}
