//! Long-lived, explicitly addressed analysis inputs.
//!
//! Opening copies a permitted ordinary file into a private immutable snapshot.
//! Queries therefore never retain a mapping of a user file that another process
//! may truncate while this process is alive.
mod cache;
mod error;
mod query;

pub use error::{Result, SessionError};
pub use query::{ClassList, FindRefsKind, ManifestResult, ReferenceList, SourceResult, StringList};

use crate::analysis::archive::{BytesSource, ZipEntry};
use cache::{InflateCache, InflateKey};
use cap_std::fs::Dir;
use memmap2::Mmap;
use rmcp::schemars::{self, JsonSchema};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use tempfile::NamedTempFile;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const DEFAULT_MAX_TARGETS: usize = 2;
const DEFAULT_MAX_INPUT_BYTES: u64 = 2 << 30;
const DEFAULT_MAX_ARCHIVE_ENTRIES: usize = 100_000;
const DEFAULT_MAX_DIRECTORY_BYTES: usize = 64 << 20;
const DEFAULT_MAX_INFLATED_ENTRY: usize = crate::analysis::archive::DEFAULT_MAX_INFLATED_ENTRY;
const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 2;
const DEFAULT_CACHE_BYTES: usize = 512 << 20;
const DEFAULT_MAX_RESULT_ITEMS: usize = 100_000;
const DEFAULT_MAX_RESULT_BYTES: usize = 16 << 20;

fn default_analysis_threads() -> usize {
    std::thread::available_parallelism().map_or(8, std::num::NonZero::get)
}

#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub roots: Vec<PathBuf>,
    pub max_targets: usize,
    pub max_input_bytes: u64,
    pub max_archive_entries: usize,
    pub max_directory_bytes: usize,
    pub max_inflated_entry: usize,
    pub analysis_threads: usize,
    pub max_concurrent_requests: usize,
    pub inflate_cache_bytes: usize,
    pub max_result_items: usize,
    pub max_result_bytes: usize,
    #[doc(hidden)]
    pub force_parallel_entries: bool,
}

impl SessionConfig {
    pub fn for_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            max_targets: DEFAULT_MAX_TARGETS,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_archive_entries: DEFAULT_MAX_ARCHIVE_ENTRIES,
            max_directory_bytes: DEFAULT_MAX_DIRECTORY_BYTES,
            max_inflated_entry: DEFAULT_MAX_INFLATED_ENTRY,
            analysis_threads: default_analysis_threads(),
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            inflate_cache_bytes: DEFAULT_CACHE_BYTES,
            max_result_items: DEFAULT_MAX_RESULT_ITEMS,
            max_result_bytes: DEFAULT_MAX_RESULT_BYTES,
            force_parallel_entries: false,
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self::for_roots(vec![PathBuf::from(".")])
    }
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct OpenedTarget {
    pub target_id: String,
    pub kind: InputKind,
    pub size: u64,
    pub entry_count: usize,
    pub dex_entry_count: usize,
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Apk,
    Dex,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TargetStatus {
    pub target_id: String,
    pub kind: InputKind,
    pub size: u64,
    pub entry_count: usize,
    pub dex_entry_count: usize,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SessionStatus {
    pub targets: Vec<TargetStatus>,
    pub max_targets: usize,
    pub max_input_bytes: u64,
    pub analysis_threads: usize,
    pub max_concurrent_requests: usize,
    pub inflate_cache_bytes: usize,
    pub inflate_cache_entries: usize,
    pub inflate_cache_loads: u64,
    pub inflate_cache_hits: u64,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct EntryInfo {
    pub entry_index: usize,
    pub name: String,
    pub compression: u16,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub is_dex: bool,
}

struct AllowedRoot {
    /// Both the spelling supplied by the client and the platform-canonical spelling
    /// (`/var` versus `/private/var` on macOS) address the same capability.
    paths: Vec<PathBuf>,
    dir: Dir,
}

pub(crate) struct ResultBudget {
    max_items: usize,
    max_bytes: usize,
    items: usize,
    bytes: usize,
}

impl ResultBudget {
    fn new(max_items: usize, max_bytes: usize) -> Self {
        // Exact serialized size of the MCP success envelope containing `{\"items\":[]}`.
        Self {
            max_items,
            max_bytes,
            items: 0,
            bytes: 50,
        }
    }

    pub(crate) fn add<T: Serialize>(&mut self, item: &T) -> Result<()> {
        let item_bytes = serde_json::to_vec(item)
            .map_err(SessionError::invalid)?
            .len();
        self.add_serialized_len(item_bytes)
    }

    pub(crate) fn add_serialized_len(&mut self, item_bytes: usize) -> Result<()> {
        if self.items == self.max_items {
            return Err(result_limit_error(self.max_items, self.max_bytes));
        }
        let separator = usize::from(self.items != 0);
        if self
            .bytes
            .saturating_add(separator)
            .saturating_add(item_bytes)
            > self.max_bytes
        {
            return Err(result_limit_error(self.max_items, self.max_bytes));
        }
        self.items += 1;
        self.bytes += separator + item_bytes;
        Ok(())
    }
}

pub(crate) fn result_limit_error(max_items: usize, max_bytes: usize) -> SessionError {
    SessionError::limit(format!(
        "complete result exceeds the configured budget ({max_items} items or {max_bytes} serialized bytes); use the streaming CLI or a more precise domain query"
    ))
}

/// Process-local collection of immutable targets.
pub struct AnalysisSession {
    config: SessionConfig,
    roots: Vec<AllowedRoot>,
    targets: RwLock<HashMap<String, Arc<Target>>>,
    inflate_cache: InflateCache,
    analysis_pool: rayon::ThreadPool,
}

pub(crate) enum EntryBytes<'a> {
    Snapshot(Cow<'a, [u8]>),
    Inflated(Arc<Vec<u8>>),
}

impl std::ops::Deref for EntryBytes<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Snapshot(bytes) => bytes,
            Self::Inflated(bytes) => bytes,
        }
    }
}

pub(crate) struct Target {
    id: String,
    kind: InputKind,
    size: u64,
    _snapshot: NamedTempFile,
    mapped: Mmap,
    entries: Vec<ZipEntry>,
    dex_entries: Vec<usize>,
    cancel: CancellationToken,
}

impl AnalysisSession {
    pub fn new(config: SessionConfig) -> Result<Self> {
        if config.max_targets == 0 {
            return Err(SessionError::limit("max_targets must be greater than zero"));
        }
        if config.analysis_threads == 0 {
            return Err(SessionError::limit(
                "analysis_threads must be greater than zero",
            ));
        }
        if config.max_concurrent_requests == 0 {
            return Err(SessionError::limit(
                "max_concurrent_requests must be greater than zero",
            ));
        }
        if config.max_result_items == 0 || config.max_result_bytes == 0 {
            return Err(SessionError::limit(
                "result item and byte budgets must be greater than zero",
            ));
        }
        let mut roots = Vec::with_capacity(config.roots.len());
        for root in &config.roots {
            let supplied = absolute_clean(root)?;
            let path = supplied.canonicalize().map_err(|error| {
                SessionError::new(
                    "PATH_NOT_ALLOWED",
                    format!("cannot open root {}: {error}", root.display()),
                )
            })?;
            let file = File::open(&path).map_err(|error| {
                SessionError::new(
                    "PATH_NOT_ALLOWED",
                    format!("cannot open root {}: {error}", path.display()),
                )
            })?;
            let metadata = file.metadata().map_err(SessionError::invalid)?;
            if !metadata.is_dir() {
                return Err(SessionError::new(
                    "PATH_NOT_ALLOWED",
                    format!("root is not a directory: {}", path.display()),
                ));
            }
            let mut paths = vec![supplied];
            if !paths.contains(&path) {
                paths.push(path);
            }
            roots.push(AllowedRoot {
                paths,
                dir: Dir::from_std_file(file),
            });
        }
        if roots.is_empty() {
            return Err(SessionError::new(
                "PATH_NOT_ALLOWED",
                "at least one input root is required",
            ));
        }
        let inflate_cache = InflateCache::new(config.inflate_cache_bytes);
        let analysis_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(config.analysis_threads)
            .build()
            .map_err(SessionError::invalid)?;
        Ok(Self {
            config,
            roots,
            targets: RwLock::new(HashMap::new()),
            inflate_cache,
            analysis_pool,
        })
    }

    pub fn open(&self, requested: impl AsRef<Path>) -> Result<OpenedTarget> {
        if self.targets.read().expect("target lock poisoned").len() >= self.config.max_targets {
            return Err(SessionError::limit(format!(
                "at most {} targets may be open",
                self.config.max_targets
            )));
        }
        let requested = absolute_clean(requested.as_ref())?;
        let mut selected: Option<(&AllowedRoot, PathBuf, usize)> = None;
        for root in &self.roots {
            for path in &root.paths {
                let Ok(relative) = requested.strip_prefix(path) else {
                    continue;
                };
                let depth = path.components().count();
                if selected.as_ref().is_none_or(|(_, _, best)| depth > *best) {
                    selected = Some((root, relative.to_owned(), depth));
                }
            }
        }
        let (root, relative, _) = selected.ok_or_else(|| {
            SessionError::new(
                "PATH_NOT_ALLOWED",
                format!("{} is outside the configured roots", requested.display()),
            )
        })?;
        if relative.as_os_str().is_empty() {
            return Err(SessionError::new(
                "INVALID_INPUT",
                "input path is a directory",
            ));
        }
        let mut source = root.dir.open(relative).map_err(|error| {
            SessionError::new(
                "PATH_NOT_ALLOWED",
                format!("cannot open {}: {error}", requested.display()),
            )
        })?;
        let before = source.metadata().map_err(SessionError::invalid)?;
        if !before.is_file() {
            return Err(SessionError::new(
                "INVALID_INPUT",
                "input is not a regular file",
            ));
        }
        if before.len() > self.config.max_input_bytes {
            return Err(SessionError::limit(format!(
                "input is {} bytes, over the {} byte limit",
                before.len(),
                self.config.max_input_bytes
            )));
        }

        let mut snapshot = NamedTempFile::new().map_err(SessionError::invalid)?;
        let copied = copy_bounded(
            &mut source,
            snapshot.as_file_mut(),
            self.config.max_input_bytes,
        )?;
        snapshot
            .as_file_mut()
            .flush()
            .map_err(SessionError::invalid)?;
        let after = source.metadata().map_err(SessionError::invalid)?;
        if copied != before.len()
            || after.len() != before.len()
            || after.modified().ok() != before.modified().ok()
        {
            return Err(SessionError::new(
                "INPUT_CHANGED",
                "input changed while its private snapshot was being made",
            ));
        }
        // SAFETY: the private file remains owned by Target and is never modified again.
        let mapped = unsafe { Mmap::map(snapshot.as_file()) }.map_err(SessionError::invalid)?;
        let (kind, entries, dex_entries) = inspect_snapshot(
            &mapped,
            self.config.max_archive_entries,
            self.config.max_directory_bytes,
        )?;
        let id = Uuid::new_v4().to_string();
        let target = Arc::new(Target {
            id: id.clone(),
            kind,
            size: copied,
            _snapshot: snapshot,
            mapped,
            entries,
            dex_entries,
            cancel: CancellationToken::new(),
        });
        let opened = target.opened();
        let mut targets = self.targets.write().expect("target lock poisoned");
        if targets.len() >= self.config.max_targets {
            return Err(SessionError::limit(format!(
                "at most {} targets may be open",
                self.config.max_targets
            )));
        }
        targets.insert(id, target);
        Ok(opened)
    }

    pub fn close(&self, target_id: &str) -> Result<bool> {
        let target = self
            .targets
            .write()
            .expect("target lock poisoned")
            .remove(target_id);
        if let Some(target) = target {
            target.cancel.cancel();
            self.inflate_cache.remove_target(target_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn status(&self) -> SessionStatus {
        let targets = self.targets.read().expect("target lock poisoned");
        let mut statuses = targets
            .values()
            .map(|target| target.status())
            .collect::<Vec<_>>();
        statuses.sort_by(|left, right| left.target_id.cmp(&right.target_id));
        let cache = self.inflate_cache.stats();
        SessionStatus {
            targets: statuses,
            max_targets: self.config.max_targets,
            max_input_bytes: self.config.max_input_bytes,
            analysis_threads: self.config.analysis_threads,
            max_concurrent_requests: self.config.max_concurrent_requests,
            inflate_cache_bytes: cache.bytes,
            inflate_cache_entries: cache.entries,
            inflate_cache_loads: cache.loads,
            inflate_cache_hits: cache.hits,
        }
    }

    pub fn target_status(&self, target_id: &str) -> Result<TargetStatus> {
        Ok(self.target(target_id)?.status())
    }

    pub fn entries(&self, target_id: &str) -> Result<Vec<EntryInfo>> {
        let target = self.target(target_id)?;
        let mut items = Vec::new();
        let mut budget = self.result_budget();
        let selected = target
            .dex_entries
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        for (index, entry) in target.entries.iter().enumerate() {
            let item = EntryInfo {
                entry_index: index,
                name: entry.name.clone(),
                compression: entry.compression,
                compressed_size: entry.compressed_size,
                uncompressed_size: entry.uncompressed_size,
                is_dex: selected.contains(&index),
            };
            budget.add(&item)?;
            items.push(item);
        }
        Ok(items)
    }

    pub fn manifest(&self, target_id: &str) -> Result<ManifestResult> {
        self.manifest_cancellable(target_id, None)
    }

    pub(crate) fn manifest_cancellable(
        &self,
        target_id: &str,
        request_cancel: Option<&CancellationToken>,
    ) -> Result<ManifestResult> {
        let target = self.target(target_id)?;
        check_target_cancel(&target, request_cancel)?;
        if matches!(target.kind, InputKind::Dex) {
            return Err(SessionError::new(
                "INVALID_INPUT",
                "a bare DEX has no manifest",
            ));
        }
        let (index, entry) = target
            .entries
            .iter()
            .enumerate()
            .rev()
            .find(|(_, entry)| entry.name == "AndroidManifest.xml")
            .ok_or_else(|| SessionError::new("INVALID_INPUT", "AndroidManifest.xml not found"))?;
        let data = self.entry_bytes(&target, index, entry, request_cancel)?;
        check_target_cancel(&target, request_cancel)?;
        let xml = crate::analysis::manifest::decode(&data).map_err(SessionError::invalid)?;
        check_target_cancel(&target, request_cancel)?;
        let result = ManifestResult { xml };
        self.ensure_result(&result)?;
        Ok(result)
    }

    pub(crate) fn dex_bytes<'a>(
        &self,
        target: &'a Target,
        entry_index: usize,
        request_cancel: Option<&CancellationToken>,
    ) -> Result<EntryBytes<'a>> {
        if !target.dex_entries.contains(&entry_index) {
            return Err(SessionError::new(
                "INVALID_INPUT",
                "entry is not a selected root DEX",
            ));
        }
        let entry = target
            .entries
            .get(entry_index)
            .ok_or_else(|| SessionError::new("INVALID_INPUT", "entry index is out of range"))?;
        self.entry_bytes(target, entry_index, entry, request_cancel)
    }

    fn entry_bytes<'a>(
        &self,
        target: &'a Target,
        entry_index: usize,
        entry: &ZipEntry,
        request_cancel: Option<&CancellationToken>,
    ) -> Result<EntryBytes<'a>> {
        match entry.compression {
            0 => crate::analysis::archive::stored_entry(target, entry)
                .map(EntryBytes::Snapshot)
                .map_err(SessionError::invalid),
            8 => self
                .inflate_cache
                .get_or_load(
                    InflateKey {
                        target: target.id.clone(),
                        entry: entry_index,
                    },
                    entry.uncompressed_size,
                    &target.cancel,
                    request_cancel,
                    || {
                        crate::analysis::archive::inflate_entry(
                            target,
                            entry,
                            self.config.max_inflated_entry,
                        )
                        .map_err(SessionError::invalid)
                    },
                )
                .map(EntryBytes::Inflated),
            method => Err(SessionError::new(
                "INVALID_INPUT",
                format!("unsupported compression method {method} for {}", entry.name),
            )),
        }
    }

    pub(crate) fn max_concurrent_requests(&self) -> usize {
        self.config.max_concurrent_requests
    }

    pub(crate) fn result_budget(&self) -> ResultBudget {
        ResultBudget::new(self.config.max_result_items, self.config.max_result_bytes)
    }

    pub(crate) fn ensure_result<T: Serialize>(&self, value: &T) -> Result<()> {
        // The success envelope adds 38 bytes around the serialized `data` value.
        let bytes = serde_json::to_vec(value)
            .map_err(SessionError::invalid)?
            .len()
            .saturating_add(38);
        if bytes > self.config.max_result_bytes {
            return Err(result_limit_error(
                self.config.max_result_items,
                self.config.max_result_bytes,
            ));
        }
        Ok(())
    }

    pub(crate) fn target(&self, target_id: &str) -> Result<Arc<Target>> {
        self.targets
            .read()
            .expect("target lock poisoned")
            .get(target_id)
            .cloned()
            .ok_or_else(|| SessionError::new("TARGET_NOT_FOUND", "target is not open"))
    }
}

impl BytesSource for Target {
    fn source_len(&self) -> usize {
        self.mapped.len()
    }

    fn range(&self, offset: usize, len: usize) -> anyhow::Result<std::borrow::Cow<'_, [u8]>> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("range overflow"))?;
        self.mapped
            .get(offset..end)
            .map(std::borrow::Cow::Borrowed)
            .ok_or_else(|| anyhow::anyhow!("range out of bounds"))
    }
}

impl Target {
    fn opened(&self) -> OpenedTarget {
        OpenedTarget {
            target_id: self.id.clone(),
            kind: self.kind,
            size: self.size,
            entry_count: self.entries.len(),
            dex_entry_count: self.dex_entries.len(),
        }
    }

    fn status(&self) -> TargetStatus {
        TargetStatus {
            target_id: self.id.clone(),
            kind: self.kind,
            size: self.size,
            entry_count: self.entries.len(),
            dex_entry_count: self.dex_entries.len(),
        }
    }
}

fn inspect_snapshot(
    mapped: &Mmap,
    max_entries: usize,
    max_directory_bytes: usize,
) -> Result<(InputKind, Vec<ZipEntry>, Vec<usize>)> {
    let bytes: &[u8] = mapped;
    if crate::analysis::apk::is_bare_dex(bytes) {
        let entries = vec![ZipEntry::bare("classes.dex", mapped.len())];
        return Ok((InputKind::Dex, entries, vec![0]));
    }
    let entries = crate::analysis::archive::parse_zip_entries_bounded(
        bytes,
        |_| true,
        max_entries,
        max_directory_bytes,
    )
    .map_err(SessionError::invalid)?;
    let dex_entries = crate::analysis::apk::select_dex_entries(&entries);
    if dex_entries.is_empty() {
        return Err(SessionError::new(
            "INVALID_INPUT",
            "archive contains no root DEX entry",
        ));
    }
    Ok((InputKind::Apk, entries, dex_entries))
}

fn copy_bounded(source: &mut impl Read, destination: &mut impl Write, max: u64) -> Result<u64> {
    let mut limited = source.take(max.saturating_add(1));
    let copied = std::io::copy(&mut limited, destination).map_err(SessionError::invalid)?;
    if copied > max {
        return Err(SessionError::limit(format!(
            "input exceeds the {max} byte limit"
        )));
    }
    Ok(copied)
}

fn check_target_cancel(target: &Target, request_cancel: Option<&CancellationToken>) -> Result<()> {
    if target.cancel.is_cancelled() || request_cancel.is_some_and(CancellationToken::is_cancelled) {
        return Err(SessionError::new("CANCELLED", "analysis was cancelled"));
    }
    Ok(())
}

fn absolute_clean(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(SessionError::invalid)?
            .join(path)
    };
    let mut clean = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                clean.push(component)
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !clean.pop() {
                    return Err(SessionError::new(
                        "PATH_NOT_ALLOWED",
                        "path escapes its root",
                    ));
                }
            }
        }
    }
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::archive::tests::build_zip;
    use crate::analysis::dex::tests::{const_string_fixture, dex041_container};
    use std::fs;

    fn session_at(root: &Path) -> AnalysisSession {
        AnalysisSession::new(SessionConfig::for_roots(vec![root.to_owned()])).unwrap()
    }

    #[test]
    fn opens_private_bare_dex_snapshot_and_closes_handle() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("fixture.dex");
        let dex = const_string_fixture(1);
        fs::write(&path, &dex).unwrap();
        let session = session_at(root.path());
        let opened = session.open(&path).unwrap();
        assert!(matches!(opened.kind, InputKind::Dex));
        fs::remove_file(&path).unwrap();
        assert_eq!(
            session.entries(&opened.target_id).unwrap()[0].name,
            "classes.dex"
        );
        assert!(session.close(&opened.target_id).unwrap());
        assert!(!session.close(&opened.target_id).unwrap());
        assert_eq!(
            session.entries(&opened.target_id).unwrap_err().code,
            "TARGET_NOT_FOUND"
        );
    }

    #[test]
    fn selects_root_dexes_and_preserves_all_entry_identities() {
        let root = tempfile::tempdir().unwrap();
        let dex = const_string_fixture(1);
        let zip = build_zip(&[
            ("other.dex", &dex, false),
            ("classes.dex", &dex, true),
            ("classes.dex", &dex, false),
            ("classes2.dex", &dex, false),
        ]);
        let path = root.path().join("fixture.apk");
        fs::write(&path, zip).unwrap();
        let session = session_at(root.path());
        let opened = session.open(path).unwrap();
        assert_eq!(opened.entry_count, 4);
        assert_eq!(opened.dex_entry_count, 2);
        let entries = session.entries(&opened.target_id).unwrap();
        assert_eq!(entries.iter().filter(|entry| entry.is_dex).count(), 2);
        assert!(!entries[0].is_dex);
        assert!(entries[1].is_dex);
        assert!(!entries[2].is_dex);
        assert!(entries[3].is_dex);
    }

    #[test]
    fn bare_and_stored_dex_never_enter_inflate_cache() {
        let root = tempfile::tempdir().unwrap();
        let dex = const_string_fixture(1);
        fs::write(root.path().join("bare.dex"), &dex).unwrap();
        fs::write(
            root.path().join("stored.apk"),
            build_zip(&[("classes.dex", &dex, false)]),
        )
        .unwrap();
        let mut config = SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.max_targets = 2;
        let session = AnalysisSession::new(config).unwrap();
        let bare = session
            .open(root.path().join("bare.dex"))
            .unwrap()
            .target_id;
        let stored = session
            .open(root.path().join("stored.apk"))
            .unwrap()
            .target_id;
        session.classes(&bare).unwrap();
        session.strings(&stored).unwrap();
        session.classes(&stored).unwrap();
        let status = session.status();
        assert_eq!(status.inflate_cache_entries, 0);
        assert_eq!(status.inflate_cache_bytes, 0);
        assert_eq!(status.inflate_cache_loads, 0);
        assert_eq!(status.inflate_cache_hits, 0);
    }

    #[test]
    fn deflated_dex_is_cached_across_different_queries_and_close_removes_it() {
        let root = tempfile::tempdir().unwrap();
        let dex = const_string_fixture(2);
        fs::write(
            root.path().join("deflated.apk"),
            build_zip(&[("classes.dex", &dex, true)]),
        )
        .unwrap();
        let session = session_at(root.path());
        let id = session
            .open(root.path().join("deflated.apk"))
            .unwrap()
            .target_id;
        session.classes(&id).unwrap();
        let cold = session.status();
        assert_eq!(cold.inflate_cache_entries, 1);
        assert_eq!(cold.inflate_cache_loads, 1);
        assert_eq!(cold.inflate_cache_hits, 0);
        session.strings(&id).unwrap();
        let hot = session.status();
        assert_eq!(hot.inflate_cache_entries, 1);
        assert_eq!(hot.inflate_cache_loads, 1);
        assert!(hot.inflate_cache_hits > 0);
        session.close(&id).unwrap();
        let closed = session.status();
        assert_eq!(closed.inflate_cache_entries, 0);
        assert_eq!(closed.inflate_cache_bytes, 0);
    }

    #[test]
    fn manifest_observes_request_cancellation() {
        let root = tempfile::tempdir().unwrap();
        let manifest = b"<?xml version=\"1.0\"?><manifest package=\"example\"/>";
        let dex = const_string_fixture(1);
        fs::write(
            root.path().join("fixture.apk"),
            build_zip(&[
                ("classes.dex", &dex, false),
                ("AndroidManifest.xml", manifest, true),
            ]),
        )
        .unwrap();
        let session = session_at(root.path());
        let id = session
            .open(root.path().join("fixture.apk"))
            .unwrap()
            .target_id;
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            session
                .manifest_cancellable(&id, Some(&cancel))
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        assert_eq!(session.status().inflate_cache_loads, 0);
    }

    #[test]
    fn accepts_dex_041_without_eager_logical_copies() {
        let root = tempfile::tempdir().unwrap();
        let container = dex041_container(&[1, 1]);
        fs::write(root.path().join("container.dex"), container).unwrap();
        let session = session_at(root.path());
        let opened = session.open(root.path().join("container.dex")).unwrap();
        assert_eq!(opened.dex_entry_count, 1);
    }

    #[test]
    fn rejects_outside_paths_and_target_overcommit() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(root.path().join("a.dex"), const_string_fixture(1)).unwrap();
        fs::write(root.path().join("b.dex"), const_string_fixture(1)).unwrap();
        let mut config = SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.max_targets = 1;
        let session = AnalysisSession::new(config).unwrap();
        assert_eq!(
            session.open(outside.path()).unwrap_err().code,
            "PATH_NOT_ALLOWED"
        );
        session.open(root.path().join("a.dex")).unwrap();
        assert_eq!(
            session.open(root.path().join("b.dex")).unwrap_err().code,
            "RESOURCE_LIMIT"
        );
    }

    #[test]
    fn rejects_symlink_escape_from_an_allowed_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside.dex"), const_string_fixture(1)).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(outside.path(), root.path().join("escape")).unwrap();
        let session = session_at(root.path());
        let error = session
            .open(root.path().join("escape/outside.dex"))
            .unwrap_err();
        assert_eq!(error.code, "PATH_NOT_ALLOWED");
    }

    #[test]
    fn entry_result_budget_is_checked_while_building() {
        let root = tempfile::tempdir().unwrap();
        let dex = const_string_fixture(1);
        let zip = build_zip(&[("classes.dex", &dex, false), ("classes2.dex", &dex, false)]);
        fs::write(root.path().join("fixture.apk"), zip).unwrap();
        let mut config = SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.max_result_items = 1;
        let session = AnalysisSession::new(config).unwrap();
        let id = session
            .open(root.path().join("fixture.apk"))
            .unwrap()
            .target_id;
        assert_eq!(session.entries(&id).unwrap_err().code, "RESOURCE_LIMIT");
    }

    #[test]
    fn rejects_archive_directory_limits_before_allocating_every_entry() {
        let root = tempfile::tempdir().unwrap();
        let dex = const_string_fixture(1);
        let zip = build_zip(&[("classes.dex", &dex, false), ("classes2.dex", &dex, false)]);
        fs::write(root.path().join("fixture.apk"), zip).unwrap();
        let mut config = SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.max_archive_entries = 1;
        let session = AnalysisSession::new(config).unwrap();
        assert_eq!(
            session
                .open(root.path().join("fixture.apk"))
                .unwrap_err()
                .code,
            "INVALID_INPUT"
        );
    }
}
