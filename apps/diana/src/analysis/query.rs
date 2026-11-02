use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use diana_decomp::{ControlError, FailureKind, Limits, RequestError, RunContext};

use super::apk::DexRoot;
use super::archive::EntryRecord;
use super::dex::{self, ClassRow, FieldRow, ImageIndex, MethodRow, StringRow, contains};
use super::input::{EntryIndex, InputError};
use super::request::RequestScope;

impl From<ControlError> for QueryError {
    fn from(error: ControlError) -> Self {
        Self::Request(RequestError::Control(error))
    }
}

/// One physical ZIP record, listed in directory order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryRow {
    pub image: ImageIndex,
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

/// Explicit query bounds. Unset fields impose no numeric cap; a configured bound
/// is enforced and never truncated into a successful prefix.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueryLimits {
    pub max_rows: Option<u64>,
    pub max_batch_rows: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryLimit {
    Rows,
}

impl std::fmt::Display for QueryLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Rows => "rows",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error(transparent)]
    Input(#[from] InputError),
    /// A checked DEX read failure; its own kind survives unchanged.
    #[error(transparent)]
    Engine(#[from] diana_decomp::input::InputError),
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error("{limit} exceeded: requested {requested}, maximum {maximum}")]
    Budget {
        limit: QueryLimit,
        requested: u64,
        maximum: u64,
    },
    #[error("query worker stopped before delivering its rows")]
    WorkerLost,
    #[error("entries lists physical archive records only; a bare DEX has none")]
    NotArchive,
    #[error("the job count must be positive")]
    InvalidJobs,
    #[error("query work was cancelled")]
    Cancelled,
}

impl QueryError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(error) => error.kind(),
            Self::Engine(error) => error.kind(),
            Self::Request(error) => error.kind(),
            Self::Budget { .. } => FailureKind::BudgetExceeded,
            Self::Cancelled => FailureKind::Cancelled,
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
    pub limits: QueryLimits,
    pub jobs: usize,
}

/// Prepared query: root images are resolved once, rows stream in completion order.
pub struct Query<'a> {
    roots: Vec<DexRoot<'a>>,
    images: Vec<ImageInfo>,
    kind: QueryKind,
    filter: Option<Vec<u16>>,
    class: Option<Vec<u16>>,
    limits: QueryLimits,
    jobs: usize,
    entries: Vec<EntryRecord>,
}

impl<'a> Query<'a> {
    /// Resolves the root images and their display labels. No DEX analysis runs
    /// here, and no row is produced yet.
    pub fn new(scope: &'a RequestScope, plan: QueryPlan) -> Result<Self, QueryError> {
        if plan.jobs == 0 {
            return Err(QueryError::InvalidJobs);
        }
        let roots = scope.roots()?;
        let mut images = Vec::new();
        for (ordinal, root) in roots.iter().enumerate() {
            images.try_reserve(1).map_err(|_| QueryError::WorkerLost)?;
            images.push(ImageInfo {
                index: ImageIndex(ordinal),
                label: root.label().to_owned(),
                entry: root.entry().map(EntryRecord::index),
            });
        }
        let entries = scope
            .input()
            .archive()
            .map(|directory| directory.entries().to_vec())
            .unwrap_or_default();
        Ok(Self {
            roots,
            images,
            kind: plan.kind,
            filter: plan.filter,
            class: plan.class,
            limits: plan.limits,
            jobs: plan.jobs,
            entries,
        })
    }

    pub fn images(&self) -> &[ImageInfo] {
        &self.images
    }

    /// Streams every matching row. Rows arrive in worker completion order; the
    /// caller must not rely on a sequence. Exceeding a configured bound or a
    /// cancellation stops the request with an error, and the delivered prefix is
    /// not a complete result.
    pub fn run(
        &self,
        cancel: &AtomicBool,
        sink: &mut impl FnMut(Row) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(QueryError::Cancelled);
        }
        let mut delivered = Counter::default();
        if self.kind == QueryKind::Entries {
            if self.images.is_empty() {
                return Ok(());
            }
            if self.entries.is_empty() {
                return Err(QueryError::NotArchive);
            }
            for entry in &self.entries {
                cancel_check(cancel)?;
                let row = EntryRow {
                    image: ImageIndex(0),
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
                delivered.push(1, self.limits)?;
                sink(Row::Entry(row))?;
            }
            return Ok(());
        }
        let workers = self.jobs.min(self.roots.len().max(1));
        let (sender, receiver) = sync_channel::<Message>(workers);
        let mut failure = None;
        thread::scope(|scope| {
            for worker in 0..workers {
                let sender = sender.clone();
                let roots = &self.roots;
                let plan = (self.kind, &self.filter, &self.class, self.limits);
                scope.spawn(move || {
                    worker_loop(worker, workers, roots, plan, sender, cancel);
                });
            }
            drop(sender);
            consume(
                receiver,
                cancel,
                self.limits,
                &mut delivered,
                sink,
                &mut failure,
            );
        });
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn cancel_check(cancel: &AtomicBool) -> Result<(), QueryError> {
    if cancel.load(Ordering::Relaxed) {
        Err(QueryError::Cancelled)
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct Counter {
    rows: u64,
}

impl Counter {
    fn push(&mut self, count: u64, limits: QueryLimits) -> Result<(), QueryError> {
        self.rows = self.rows.checked_add(count).ok_or(QueryError::WorkerLost)?;
        if let Some(maximum) = limits.max_rows
            && self.rows > maximum
        {
            return Err(QueryError::Budget {
                limit: QueryLimit::Rows,
                requested: self.rows,
                maximum,
            });
        }
        Ok(())
    }
}

enum Message {
    Batch(Vec<Row>),
    Failed(QueryError),
}

fn worker_loop(
    worker: usize,
    workers: usize,
    roots: &[DexRoot<'_>],
    plan: (QueryKind, &Option<Vec<u16>>, &Option<Vec<u16>>, QueryLimits),
    sender: SyncSender<Message>,
    cancel: &AtomicBool,
) {
    let (kind, filter, class, limits) = plan;
    let batch_rows = limits.max_batch_rows.unwrap_or(1).max(1);
    for (ordinal, root) in roots.iter().enumerate() {
        if ordinal % workers != worker {
            continue;
        }
        let mut buffer = Batch {
            rows: Vec::new(),
            sender: &sender,
            cancel,
            batch_rows,
        };
        let cancel_read = || cancel.load(Ordering::Relaxed);
        let mut control = RunContext::new(Limits::default(), &cancel_read);
        let filter = filter.as_deref();
        let class = class.as_deref();
        let result = match kind {
            QueryKind::Entries => Ok(()),
            QueryKind::Classes => match dex::open(root.bytes(), &mut control) {
                Ok(index) => {
                    let mut emit = |row: dex::ClassRow| buffer.push(Row::Class(row));
                    dex::classes(&index, ImageIndex(ordinal), filter, &mut control, &mut emit)
                }
                Err(error) => Err(error),
            },
            QueryKind::Methods => match dex::open(root.bytes(), &mut control) {
                Ok(index) => {
                    let mut emit = |row: dex::MethodRow| buffer.push(Row::Method(row));
                    dex::methods(
                        &index,
                        ImageIndex(ordinal),
                        class,
                        filter,
                        &mut control,
                        &mut emit,
                    )
                }
                Err(error) => Err(error),
            },
            QueryKind::Fields => match dex::open(root.bytes(), &mut control) {
                Ok(index) => {
                    let mut emit = |row: dex::FieldRow| buffer.push(Row::Field(row));
                    dex::fields(
                        &index,
                        ImageIndex(ordinal),
                        class,
                        filter,
                        &mut control,
                        &mut emit,
                    )
                }
                Err(error) => Err(error),
            },
            QueryKind::Strings => match dex::open(root.bytes(), &mut control) {
                Ok(index) => {
                    let mut emit = |row: dex::StringRow| buffer.push(Row::String(row));
                    dex::strings(&index, ImageIndex(ordinal), filter, &mut control, &mut emit)
                }
                Err(error) => Err(error),
            },
        };
        let result = match result {
            Ok(()) => buffer.flush(),
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            let _ = sender.send(Message::Failed(error));
            return;
        }
    }
}

/// Row buffer that keeps the pending batch bounded by the explicit batch size.
struct Batch<'a> {
    rows: Vec<Row>,
    sender: &'a SyncSender<Message>,
    cancel: &'a AtomicBool,
    batch_rows: u64,
}

impl Batch<'_> {
    fn push(&mut self, row: Row) -> Result<(), QueryError> {
        self.rows.push(row);
        if u64::try_from(self.rows.len()).unwrap_or(u64::MAX) >= self.batch_rows {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), QueryError> {
        if self.rows.is_empty() {
            return Ok(());
        }
        cancel_check(self.cancel)?;
        let rows = std::mem::take(&mut self.rows);
        self.sender
            .send(Message::Batch(rows))
            .map_err(|_| QueryError::WorkerLost)
    }
}

fn consume(
    receiver: Receiver<Message>,
    cancel: &AtomicBool,
    limits: QueryLimits,
    delivered: &mut Counter,
    sink: &mut impl FnMut(Row) -> Result<(), QueryError>,
    failure: &mut Option<QueryError>,
) {
    loop {
        if let Err(error) = cancel_check(cancel) {
            *failure = Some(error);
            break;
        }
        match receiver.recv() {
            Ok(Message::Batch(rows)) => {
                let count = u64::try_from(rows.len()).unwrap_or(u64::MAX);
                if let Err(error) = delivered.push(count, limits) {
                    *failure = Some(error);
                    break;
                }
                for row in rows {
                    if let Err(error) = sink(row) {
                        *failure = Some(error);
                        return;
                    }
                }
            }
            Ok(Message::Failed(error)) => {
                *failure = Some(error);
                break;
            }
            Err(_) => break,
        }
    }
    cancel.store(true, Ordering::Relaxed);
}
