//! APK container layer: ZIP discovery, inflation, DEX 041 views and the
//! scheduler that runs one command's work over every entry.
//!
//! Everything that reads the archive goes through [`map_dex_entries`], which owns
//! the byte source, the worker pool, entry ordering and the `--debug` inflate
//! timings. The source itself is [`Archive`]: a mapping on native, and under WASI -
//! which has no `mmap` - a file read in ranges on demand.
//! Results are flattened in central-directory order, so output is deterministic
//! no matter which worker stole which entry. Nothing here shells out to Python, a
//! JVM or an external decompiler.

use crate::dex;
use crate::query::Query;
use crate::zip::{ZipEntry, inflate_entry};
use anyhow::{Context, Result, bail};
#[cfg(not(target_family = "wasm"))]
use memmap2::Mmap;
use rayon::prelude::*;
use std::cell::OnceCell;
#[cfg(not(target_family = "wasm"))]
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The bytes of an APK.
///
/// Native maps the file, so the OS keeps the pages it needs and a 342 MiB archive costs
/// no heap; `mmap` is also what lets a class lookup touch only the entries it has to.
/// WASI has no `mmap`, so there the archive is read in ranges on demand instead: the parser
/// asks for the EOCD tail, one central directory entry at a time, and an entry's compressed
/// bytes, and nothing else is ever read. That is also the shape a host with a byte source of
/// its own would plug into, and the one a browser can serve.
enum Archive {
    #[cfg(not(target_family = "wasm"))]
    Mapped(Mmap),
    #[cfg(target_os = "wasi")]
    Ranged {
        // `Mutex` because `range` takes `&self`: `std::os::wasi::fs::FileExt::read_at`
        // is still unstable, so the file has to be seeked. It also keeps the value
        // `Sync`, which the parallel entry walk type-checks even where it is serial.
        file: std::sync::Mutex<std::fs::File>,
        len: usize,
    },
}

impl Archive {
    fn open(path: &Path) -> Result<Self> {
        #[cfg(not(target_family = "wasm"))]
        let archive = {
            let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
            // SAFETY: unchanged from the inline mapping this replaces. The mapping
            // must not outlive the file, which is why it is owned by this value.
            Archive::Mapped(unsafe { Mmap::map(&file) }.context("map APK")?)
        };
        #[cfg(target_os = "wasi")]
        let archive = {
            let file =
                std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
            let len = file
                .metadata()
                .with_context(|| format!("stat {}", path.display()))?
                .len() as usize;
            Archive::Ranged {
                file: std::sync::Mutex::new(file),
                len,
            }
        };
        Ok(archive)
    }
}

/// One positioned read, with a short-read loop: a wasi `read` may return fewer bytes
/// than asked for.
#[cfg(target_os = "wasi")]
fn read_exact_at(
    file: &std::sync::Mutex<std::fs::File>,
    mut buffer: &mut [u8],
    mut offset: u64,
) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom};

    // A poisoned lock would mean a panic elsewhere; `panic = "abort"` in release makes
    // that unreachable, and recovering is safer than panicking in a parser.
    let mut file = file.lock().unwrap_or_else(|poison| poison.into_inner());
    while !buffer.is_empty() {
        file.seek(SeekFrom::Start(offset))
            .with_context(|| format!("seek to {offset}"))?;
        let read = file
            .read(buffer)
            .with_context(|| format!("read at {offset}"))?;
        if read == 0 {
            bail!("unexpected end of file at {offset}");
        }
        buffer = &mut buffer[read..];
        offset += read as u64;
    }
    Ok(())
}

impl crate::zip::BytesSource for Archive {
    fn source_len(&self) -> usize {
        match self {
            #[cfg(not(target_family = "wasm"))]
            Archive::Mapped(mapped) => mapped.len(),
            #[cfg(target_os = "wasi")]
            Archive::Ranged { len, .. } => *len,
        }
    }

    fn range(&self, offset: usize, len: usize) -> Result<std::borrow::Cow<'_, [u8]>> {
        let end = offset.checked_add(len).context("range overflow")?;
        if end > self.source_len() {
            bail!("range out of bounds");
        }
        match self {
            #[cfg(not(target_family = "wasm"))]
            Archive::Mapped(mapped) => Ok(std::borrow::Cow::Borrowed(&mapped[offset..end])),
            #[cfg(target_os = "wasi")]
            Archive::Ranged { file, .. } => {
                let mut buffer = vec![0u8; len];
                read_exact_at(file, &mut buffer, offset as u64)?;
                Ok(std::borrow::Cow::Owned(buffer))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClassEntry {
    pub descriptor: String,
    /// The DEX entry the class was found in, shared by every class of that entry.
    ///
    /// A class index is hundreds of thousands of rows on a real APK and they all repeat
    /// one of a few dozen names, so cloning the name per row (an allocation plus a copy
    /// each) was pure overhead. `Arc<str>` compares by content like `String` does, so
    /// the sort order and the rendered bytes are unchanged.
    pub dex_name: std::sync::Arc<str>,
    /// The first [`KEY_BYTES`] of `descriptor`, big-endian, so that ordering rows can
    /// usually decide inside the row instead of chasing the descriptor's heap bytes.
    key: u64,
}

/// How many descriptor bytes the inline sort key keeps.
///
/// Measured on a 567,192-class index: 8 bytes separate 73.7% of the rows from every
/// other row, 16 bytes only 74.3%, so the longer key buys nothing for twice the bytes
/// per row in the sort's move traffic. The rest of the comparisons fall through to the
/// descriptor itself, which is what makes the key order-preserving rather than
/// approximate.
const KEY_BYTES: usize = 8;

/// The sort key of a descriptor: its first bytes, zero-padded, big-endian.
///
/// Zero padding is order-preserving because a descriptor never contains a NUL byte: a
/// row whose descriptor ends before the padding compares as smaller there, which is
/// exactly how byte-slice comparison treats a prefix.
fn descriptor_key(descriptor: &str) -> u64 {
    let bytes = descriptor.as_bytes();
    let take = bytes.len().min(KEY_BYTES);
    let mut key = [0u8; KEY_BYTES];
    key[..take].copy_from_slice(&bytes[..take]);
    u64::from_be_bytes(key)
}

impl ClassEntry {
    pub fn new(descriptor: String, dex_name: std::sync::Arc<str>) -> Self {
        let key = descriptor_key(&descriptor);
        Self {
            descriptor,
            dex_name,
            key,
        }
    }
}

impl Ord for ClassEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key
            .cmp(&other.key)
            .then_with(|| self.descriptor.cmp(&other.descriptor))
            .then_with(|| self.dex_name.cmp(&other.dex_name))
    }
}

impl PartialOrd for ClassEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ClassEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for ClassEntry {}

impl ClassEntry {
    pub fn java_name(&self) -> &str {
        self.descriptor
            .strip_prefix('L')
            .and_then(|name| name.strip_suffix(';'))
            .unwrap_or(&self.descriptor)
    }

    pub fn package(&self) -> &str {
        self.java_name()
            .rsplit_once('/')
            .map_or("", |(package, _)| package)
    }

    pub fn simple_name(&self) -> &str {
        self.java_name()
            .rsplit_once('/')
            .map_or(self.java_name(), |(_, name)| name)
    }
}

/// One entry's bytes, inflated on first use.
///
/// Inflating lazily is what lets a string-only command (the class index) read a
/// prefix instead of the whole entry: the prefix path never touches [`Self::data`],
/// so the code section - two thirds of a DEX - is never decompressed.
struct InflatedDex<'a> {
    entry: &'a ZipEntry,
    source: &'a Archive,
    started: Instant,
    /// Per-entry inflation ceiling for this command, read from the host policy once.
    max_inflated: usize,
    data: OnceCell<(Vec<u8>, Duration)>,
}

impl InflatedDex<'_> {
    /// The whole entry, inflating it on first use.
    fn data(&self) -> Result<&[u8]> {
        if self.data.get().is_none() {
            let started = Instant::now();
            let bytes = inflate_entry(self.source, self.entry, self.max_inflated)?;
            let _ = self.data.set((bytes, started.elapsed()));
        }
        Ok(&self.data.get().expect("filled above").0)
    }

    /// Time spent inflating, once it happened.
    fn inflate_elapsed(&self) -> Duration {
        self.data
            .get()
            .map_or(Duration::ZERO, |(_, elapsed)| *elapsed)
    }

    /// The first `aim` bytes of the entry (or all of it when it is shorter); see
    /// [`crate::zip::inflate_prefix`].
    fn read_prefix(&self, aim: usize) -> Result<Vec<u8>> {
        crate::zip::inflate_prefix(self.source, self.entry, self.max_inflated, aim)
    }
}

/// How [`map_dex_entries`] orders the entries it hands to the worker pool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EntryOrder {
    /// Central-directory order.
    Natural,
    /// Smallest entries first, so the cheapest ones answer first.
    SmallestFirst,
}

/// Opens `path`, inflates every `classes*.dex` entry in parallel and returns the
/// flattened results of `map`, in central-directory order.
///
/// `order` picks the sequence entries are handed out in. `stop` lets a caller
/// give up on entries that have not started yet: anything already in flight on a
/// worker still runs to completion, so it shortens the work that follows an early
/// hit rather than cancelling it.
fn map_dex_entries<T: Send>(
    path: &Path,
    threads: usize,
    order: EntryOrder,
    stop: Option<&AtomicBool>,
    map: impl for<'a> Fn(InflatedDex<'a>) -> Result<Vec<T>> + Sync,
) -> Result<Vec<T>> {
    if threads == 0 {
        bail!("worker count must be greater than zero");
    }
    let archive = Archive::open(path)?;
    let mut entries = parse_dex_entries(&archive)?;
    match order {
        EntryOrder::Natural => {}
        EntryOrder::SmallestFirst => entries.sort_by_key(|entry| entry.compressed_size),
    }
    // One entry, walked the same way whether the caller drives it in parallel or not.
    // The host may have lowered the per-entry ceiling (a browser holding this instance);
    // read it once and hand it to every entry.
    let max_inflated = crate::zip::max_inflated_entry();
    let run_entry = |entry: &ZipEntry| -> Result<Vec<T>> {
        if stop.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Ok(Vec::new());
        }
        map(InflatedDex {
            entry,
            source: &archive,
            started: Instant::now(),
            max_inflated,
            data: OnceCell::new(),
        })
    };
    // `wasm32-wasip1` has no threads: rayon cannot even build a pool there, because
    // `std::thread::spawn` returns ENOTSUP. Entries are then walked serially. The `-threads`
    // variant of the target exists and would give `std::thread` back, but it needs a shared
    // memory import and a host that implements `wasi_thread_spawn` - Wasmtime behind a flag
    // today, and no browser WASI shim at all.
    // Results stay in central-directory order either way and the caller sorts, so the
    // output does not depend on which path ran.
    let results: Vec<Result<Vec<T>>> = if cfg!(target_family = "wasm") {
        entries.iter().map(run_entry).collect()
    } else {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()?;
        pool.install(|| entries.par_iter().map(run_entry).collect())
    };
    let mut out = Vec::new();
    for result in results {
        out.extend(result?);
    }
    Ok(out)
}

/// Renders one hit the way `findrefs` prints it.
///
/// Rendering happens here rather than in the CLI because this is the parallel
/// phase: a wide query produces hundreds of thousands of lines, and formatting
/// them on the main thread costs more than the whole instruction scan.
/// One reference hit, structured.
///
/// The text mode renders these and the record mode prints them, so a host reads the same
/// rows a person does. The order of the fields is the order the rows are sorted in, and
/// it is the same order the rendered lines sort in - the separator and the `matched=(`
/// between them are constants - which is what keeps the two modes' row order identical.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReferenceHit {
    pub dex_name: String,
    pub member: String,
    /// The text mode's `matched=(…)` contents, without the wrapper.
    pub matched: String,
}

impl ReferenceHit {
    /// The line the text mode prints for this hit.
    pub fn render(&self) -> String {
        // The same line `format!` with `join` produced, built once instead of three times:
        // `join` allocated an intermediate string and `format!` the line, and a wide query
        // is hundreds of thousands of rows (170,923 for `field INSTANCE` on the 343 MiB
        // corpus). The row's shape is still written in exactly one place - here.
        let mut line =
            String::with_capacity(self.dex_name.len() + self.member.len() + self.matched.len() + 24);
        line.push_str(&self.dex_name);
        line.push_str(" | ");
        line.push_str(&self.member);
        line.push_str(" | matched=(");
        line.push_str(&self.matched);
        line.push(')');
        line
    }
}

/// Rendered reference hits for `query`, aggregated in central-directory order.
///
/// The lines are unsorted: the caller sorts them, which is what keeps the printed
/// order independent of how the work was stolen.
/// Every reference hit, in central-directory order inside an entry and method order within
/// an entry; the caller sorts, because the sort is what decides the printed order.
pub fn find_reference_hits(
    path: &Path,
    query: &Query,
    threads: usize,
    debug: bool,
) -> Result<Vec<ReferenceHit>> {
    // One probe decides the prefix policy for the archive, for the pre-check below.
    let policy: OnceLock<PrefixPolicy> = OnceLock::new();
    // A query that names one class is the case where a per-entry pre-check pays: the
    // class usually lives in one entry of dozens, so every other entry can be skipped
    // after a prefix read instead of an inflate and a full scan. Wide queries match
    // almost every entry, where the same prefix read is pure overhead - so they are not
    // pre-checked at all (see `Query::names_one_class`).
    let needs_targets = query.names_one_class();
    let rows = map_dex_entries(path, threads, EntryOrder::SmallestFirst, None, |inflated| {
        if needs_targets
            && let Some(prefix) = prefix_probe(&inflated, &policy)?
            && dex::prefix_has_targets(&prefix, query)? == Some(false)
        {
            if debug {
                crate::diag::diagnose(format_args!(
                    "[APK] '{}' skipped (no target in prefix) total={:.2} us",
                    inflated.entry.name,
                    inflated.started.elapsed().as_secs_f64() * 1_000_000.0
                ));
            }
            return Ok(Vec::new());
        }
        let mut rows = Vec::new();
        for logical in dex::container::logical_dexes(&inflated.entry.name, inflated.data()?)? {
            for row in dex::find_references(&logical.data, query)? {
                rows.push(ReferenceHit {
                    dex_name: logical.name.clone(),
                    member: row.member,
                    matched: row.matched,
                });
            }
        }
        if debug {
            crate::diag::diagnose(format_args!(
                "[APK] '{}' inflate={:.2} us process={:.2} us",
                inflated.entry.name,
                inflated.inflate_elapsed().as_secs_f64() * 1_000_000.0,
                (inflated.started.elapsed() - inflated.inflate_elapsed()).as_secs_f64()
                    * 1_000_000.0
            ));
        }
        Ok(rows)
    })?;
    Ok(rows)
}

pub fn list_classes(path: &Path, threads: usize, debug: bool) -> Result<Vec<ClassEntry>> {
    let started = Instant::now();
    // One probe decides the prefix policy for the whole archive (see PrefixPolicy).
    let policy: OnceLock<PrefixPolicy> = OnceLock::new();
    let mut classes = map_dex_entries(path, threads, EntryOrder::Natural, None, |inflated| {
        if let Some(entries) = prefix_class_entries(&inflated, &policy)? {
            return Ok(entries);
        }
        let mut classes = Vec::new();
        for logical in dex::container::logical_dexes(&inflated.entry.name, inflated.data()?)? {
            for descriptor in dex::class_names(&logical.data)? {
                classes.push(ClassEntry::new(descriptor, logical.name.as_str().into()));
            }
        }
        Ok(classes)
    })?;
    let extracted = started.elapsed();
    // The comparator is the type's total order, so an unstable parallel sort produces
    // exactly the order a stable sort would - the index has no ties to preserve. wasm
    // target has no threads and takes the serial sort; the order is the same either way.
    if cfg!(target_family = "wasm") {
        classes.sort_unstable();
    } else {
        classes.par_sort_unstable();
    }
    let sorted = started.elapsed();
    classes.dedup_by(|left, right| left.descriptor == right.descriptor);
    if debug {
        crate::diag::diagnose(format_args!(
            "[classes] entries={} extract={:.2} ms sort={:.2} ms dedup={:.2} ms",
            classes.len(),
            extracted.as_secs_f64() * 1e3,
            (sorted - extracted).as_secs_f64() * 1e3,
            (started.elapsed() - sorted).as_secs_f64() * 1e3
        ));
    }
    Ok(classes)
}

/// What one probe of an archive's DEX told us about the whole archive.
///
/// DEX files in one APK come from the same build tool, so their layout is
/// homogeneous: measured on a 343 MiB APK every entry needs 26-65% of its bytes for
/// a string-only command, on a 243 MiB APK every entry needs 90-95%. One probe
/// therefore decides for all of them: once the string data is known to sit near the
/// end, the remaining entries skip the prefix path instead of paying for a probe
/// each. The reverse verdict (the string data is early) leaves every entry to make
/// its own exact decision, because the fraction varies per entry.
#[derive(Clone, Copy)]
struct PrefixPolicy {
    worth_it: bool,
}

/// Whether this entry defines `descriptor`, answered from a prefix when possible.
///
/// Mirrors [`dex::prefix::defines_class`]'s `None` contract: the caller then runs the
/// full path for this entry. The prefix path never inflates the code section, which is
/// where a lookup for a class that lives elsewhere was spending all its time.
fn prefix_defines_class(
    inflated: &InflatedDex<'_>,
    descriptor: &[u8],
    policy: &OnceLock<PrefixPolicy>,
) -> Result<Option<bool>> {
    let Some(prefix) = prefix_probe(inflated, policy)? else {
        return Ok(None);
    };
    dex::prefix::defines_class(&prefix, descriptor)
}

/// A prefix of this entry that reaches past the last string data item, or `None` when
/// the prefix path is not for it.
///
/// Three reads, each sized from what the previous one showed: the header says where the
/// string_ids table ends, the table says where the last string data item starts, and the
/// string data is the last thing before the code section. On the 343 MiB sample that
/// last offset is 26-65% of an entry (median 31%), so a string-only read or a class
/// lookup decodes roughly a third of what the full path would - and only the tables are
/// decoded twice, which is why the second read stops at string_ids rather than walking
/// on to class_defs.
fn prefix_probe(
    inflated: &InflatedDex<'_>,
    policy: &OnceLock<PrefixPolicy>,
) -> Result<Option<Vec<u8>>> {
    /// Below this there is nothing to win (the entry is small anyway).
    const MIN_GAIN: usize = 256 * 1024;
    /// Bytes kept beyond the last string offset, for the string's own data.
    const SLACK: usize = 64 * 1024;
    /// A prefix is only worth a second read below this share of the entry.
    const WORTH_BELOW: f64 = 0.70;
    /// Enough of the header to name every id table (the DEX header is 0x70 bytes).
    const HEADER_PROBE: usize = 8 * 1024;

    let size = inflated.entry.uncompressed_size;
    if inflated.entry.compression != 8 || size <= MIN_GAIN {
        return Ok(None);
    }
    if let Some(known) = policy.get()
        && !known.worth_it
    {
        return Ok(None);
    }
    // A 041 container overlays a header per member and needs the whole address space,
    // so the full path keeps handling those.
    let header = inflated.read_prefix(HEADER_PROBE)?;
    if header.get(..8) == Some(b"dex\n041\0") {
        return Ok(None);
    }
    let Some(strings_end) = dex::prefix::string_ids_end(&header) else {
        return Ok(None);
    };
    if strings_end > size {
        return Ok(None);
    }
    let offsets = inflated.read_prefix(strings_end + SLACK)?;
    // The table did not arrive complete (a crafted header, or an entry whose tables
    // really do run to the end): the caller falls back to the full path, and the
    // archive-wide policy stays undecided instead of being set from a broken entry.
    let Some(needed) = dex::prefix::string_data_end(&offsets) else {
        return Ok(None);
    };
    let fraction = needed as f64 / size as f64;
    let worth_it = fraction < WORTH_BELOW;
    let _ = policy.set(PrefixPolicy { worth_it });
    if !worth_it {
        return Ok(None);
    }
    Ok(Some(inflated.read_prefix(needed + SLACK)?))
}

/// The class index of a plain deflate DEX from a prefix of it.
///
/// `None` means "not applicable, or not provably complete": the caller then inflates
/// the whole entry. `dex::prefix::class_names` answers only when the prefix holds all
/// three tables and every descriptor it decodes, so a `Some` is exact, not a guess.
fn prefix_class_entries(
    inflated: &InflatedDex<'_>,
    policy: &OnceLock<PrefixPolicy>,
) -> Result<Option<Vec<ClassEntry>>> {
    let Some(prefix) = prefix_probe(inflated, policy)? else {
        return Ok(None);
    };
    let Some(names) = dex::prefix::class_names(&prefix)? else {
        return Ok(None);
    };
    let dex_name: std::sync::Arc<str> = inflated.entry.name.as_str().into();
    Ok(Some(
        names
            .into_iter()
            .map(|descriptor| ClassEntry::new(descriptor, std::sync::Arc::clone(&dex_name)))
            .collect(),
    ))
}

/// Locates `descriptor` and hands the entry that defines it to `map`, while the archive is
/// still mapped.
///
/// The mapping ends with the walk, so a caller that wants to keep the bytes has to copy
/// them - which is what the tests here do explicitly. The decompiler does not: it consumes
/// them inside the walk, and on a real APK the entry is 7-12 MiB, so the copy plus its
/// allocation used to be pure added cost on every `getclass`.
fn map_class_hit<T: Send>(
    path: &Path,
    descriptor: &str,
    threads: usize,
    debug: bool,
    map: impl for<'a> Fn(&'a [u8]) -> Result<T> + Sync,
) -> Result<Option<(String, T)>> {
    let stop = AtomicBool::new(false);
    // One probe decides the prefix policy for the archive (see PrefixPolicy).
    let policy: OnceLock<PrefixPolicy> = OnceLock::new();
    let hits = map_dex_entries(
        path,
        threads,
        EntryOrder::SmallestFirst,
        Some(&stop),
        |inflated| {
            // Entries that do not define the class only need the prefix, so the code
            // section is never decompressed for them. `None` means "cannot tell".
            match prefix_defines_class(&inflated, descriptor.as_bytes(), &policy)? {
                Some(false) => {
                    if debug {
                        crate::diag::diagnose(format_args!(
                            "[APK] '{}' hit=false total={:.2} us",
                            inflated.entry.name,
                            inflated.started.elapsed().as_secs_f64() * 1_000_000.0
                        ));
                    }
                    return Ok(Vec::new());
                }
                Some(true) => {
                    stop.store(true, Ordering::Relaxed);
                    if debug {
                        crate::diag::diagnose(format_args!(
                            "[APK] '{}' hit=true total={:.2} us",
                            inflated.entry.name,
                            inflated.started.elapsed().as_secs_f64() * 1_000_000.0
                        ));
                    }
                    return Ok(vec![(inflated.entry.name.clone(), map(inflated.data()?)?)]);
                }
                None => {}
            }
            for logical in dex::container::logical_dexes(&inflated.entry.name, inflated.data()?)? {
                if dex::defines_class(&logical.data, descriptor.as_bytes())? {
                    stop.store(true, Ordering::Relaxed);
                    if debug {
                        crate::diag::diagnose(format_args!(
                            "[APK] '{}' hit=true total={:.2} us",
                            inflated.entry.name,
                            inflated.started.elapsed().as_secs_f64() * 1_000_000.0
                        ));
                    }
                    return Ok(vec![(logical.name, map(&logical.data)?)]);
                }
            }
            if debug {
                crate::diag::diagnose(format_args!(
                    "[APK] '{}' hit=false total={:.2} us",
                    inflated.entry.name,
                    inflated.started.elapsed().as_secs_f64() * 1_000_000.0
                ));
            }
            Ok(Vec::new())
        },
    )?;
    Ok(hits.into_iter().next())
}

/// Decompiles `descriptor` to Java-like source, returning the DEX entry it was
/// found in and the source. `None` when no DEX defines the class.
pub fn decompile_class(
    path: &Path,
    descriptor: &str,
    threads: usize,
    debug: bool,
) -> Result<Option<(String, String)>> {
    map_class_hit(path, descriptor, threads, debug, |entry| {
        // DEX 041 containers carry an extra-long header and a container-wide
        // checksum, which the decompiler rejects; hand it a normalized view.
        let normalized = dex::container::standard_header_view(entry);
        let data = normalized.as_deref().unwrap_or(entry);
        crate::emitter::render(data, descriptor)
    })
}

/// Every entry the archive holds, in central-directory order.
///
/// No filtering and no de-duplication: this is a listing of what the file contains, so
/// two entries with the same name appear twice, in the order the directory states them.
/// A bare DEX is one entry, the same one every other command walks.

/// Whether a string's raw MUTF-8 bytes contain an ASCII needle, ignoring case.
///
/// `None` means this pair cannot be answered from the bytes — the value is not ASCII, or
/// the needle is not — and the caller decodes and folds instead. It is sound for the pair
/// it does answer: MUTF-8 is ASCII-compatible, so no byte of a multi-byte sequence is an
/// ASCII byte, and an ASCII needle can only ever match ASCII characters.
fn raw_contains_ignore_case(raw: &[u8], needle_lower: &str) -> Option<bool> {
    if !needle_lower.is_ascii() || !raw.is_ascii() {
        return None;
    }
    let needle = needle_lower.as_bytes();
    if needle.is_empty() {
        return Some(true);
    }
    if needle.len() > raw.len() {
        return Some(false);
    }
    Some(raw.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(left, right)| left.to_ascii_lowercase() == *right)
    }))
}

/// Whether `haystack` contains `needle`, ignoring case, without allocating.
///
/// The straightforward version - lowercase the value, then search - allocates once per
/// string, and a search over the corpus is half a million of them. ASCII is the case
/// that happens (descriptors, member names, URLs, messages) and it lowercases byte by
/// byte, so it needs no copy at all; anything non-ASCII falls back to the Unicode fold,
/// which is what a host's own `toLowerCase().includes()` did and therefore what the
/// engine has to reproduce.
fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    if haystack.is_ascii() && needle_lower.is_ascii() {
        let hay = haystack.as_bytes();
        let needle = needle_lower.as_bytes();
        if needle.is_empty() {
            return true;
        }
        if needle.len() > hay.len() {
            return false;
        }
        return hay.windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle)
                .all(|(left, right)| left.to_ascii_lowercase() == *right)
        });
    }
    haystack.to_lowercase().contains(needle_lower)
}

/// One string, and which DEX it came from.
#[derive(Clone, Debug)]
pub struct StringEntry {
    pub dex_name: std::sync::Arc<str>,
    /// The index in that DEX's string table, which is the order the table stores.
    pub index: usize,
    pub value: String,
}

/// Every string in every root DEX, in central-directory order and table order within
/// an entry, or the ones `filter` matches.
///
/// Not sorted: a string table is indexed, the index is what a `findrefs` row resolves
/// through, and the order the file declares is the only order that means anything.
///
/// `filter` is a case-insensitive substring of the value, `limit` keeps the first that
/// many matches in the order this function returns them, and `offset` starts that page
/// later in the same order.
pub fn list_strings(
    path: &Path,
    threads: usize,
    debug: bool,
    filter: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<StringEntry>> {
    if threads == 0 {
        bail!("worker count must be greater than zero");
    }
    let needle = filter.map(str::to_lowercase);
    let take = limit.unwrap_or(usize::MAX);
    let skip = offset.unwrap_or(0);
    // The walk may stop once it holds everything the page needs, which is the skipped matches
    // plus the page itself. Stopping at `take` would truncate the offset away and hand a
    // scrollback the first page again. The anchor for this replacement is line-exact on
    // purpose: the two deeper sites are indented forms of the same text, so a substring
    // replace catches three of them - which is how the first attempt at this aborted.
    let fill = skip.saturating_add(take);
    // The early stop is only sound for a serial walk. Entries are walked in parallel on the
    // native side and their results are ordered afterwards, so a shared counter decides *which*
    // entries contributed before the ordering happens: with threads > 1, `--limit` returned a
    // different page than with one thread. Collecting everything and letting the caller's sort
    // plus the truncation below choose restores determinism across `--threads`.
    let stop_early = threads == 1;
    // How many matches the walk has produced so far. It is shared because the native
    // walk runs entries in parallel, and it only ever stops work early: the rows are
    // truncated to `take` in print order either way, so both hosts print the same bytes.
    let found = std::sync::atomic::AtomicUsize::new(0);
    let started = Instant::now();
    let rows: Vec<StringEntry> = map_dex_entries(path, threads, EntryOrder::Natural, None, |inflated| {
        let stop = std::sync::atomic::Ordering::Relaxed;
        // A page that is already full has nothing to learn from the entries below it, and
        // *inflating* one to find that out is the expensive half of the walk: this check
        // has to come before `data()`, not inside the loop it feeds. It did not, and the
        // skipped entries were still inflated - which is most of what a searched page cost.
        if stop_early && found.load(stop) >= fill {
            return Ok(Vec::new());
        }
        let dex_name: std::sync::Arc<str> = inflated.entry.name.as_str().into();
        let mut rows = Vec::new();
        for logical in dex::container::logical_dexes(&inflated.entry.name, inflated.data()?)? {
            if stop_early && found.load(stop) >= fill {
                break;
            }
            dex::for_each_string(&logical.data, |index, raw| {
                if stop_early && found.load(stop) >= fill {
                    return Ok(false);
                }
                let matches = match needle.as_deref() {
                    None => true,
                    // An ASCII needle against ASCII bytes is a comparison, not a decode;
                    // anything else takes the Unicode fold.
                    Some(needle) => match raw_contains_ignore_case(raw, needle) {
                        Some(found) => found,
                        None => contains_ignore_case(&dex::decode_string(raw), needle),
                    },
                };
                if matches {
                    found.fetch_add(1, stop);
                    rows.push(StringEntry {
                        dex_name: std::sync::Arc::clone(&dex_name),
                        index,
                        value: dex::decode_string(raw),
                    });
                }
                Ok(true)
            })?;
        }
        Ok(rows)
    })?;
    let mut rows = rows;
    if skip > 0 {
        rows.drain(..skip.min(rows.len()));
    }
    rows.truncate(take);
    if debug {
        crate::diag::diagnose(format_args!(
            "[strings] rows={} total={:.2} ms",
            rows.len(),
            started.elapsed().as_secs_f64() * 1e3
        ));
    }
    Ok(rows)
}

/// The DEX field layout of `descriptor`, from the first input and entry that defines it.
///
/// Inputs are tried in the order given, and within one archive the entries keep their
/// central-directory order, so "first" is a property of the arguments rather than of
/// which worker happened to finish first. `None` means no input defines the class,
/// which is a different answer from an empty field list.
pub fn field_plan(paths: &[PathBuf], descriptor: &str, threads: usize) -> Result<Option<String>> {
    for path in paths {
        let plans = map_dex_entries(path, threads, EntryOrder::Natural, None, |inflated| {
            let mut plans = Vec::new();
            for logical in dex::container::logical_dexes(&inflated.entry.name, inflated.data()?)? {
                // A DEX 041 container carries an extra-long header; the reader wants the
                // standard view, exactly as `getclass` does.
                let normalized = dex::container::standard_header_view(&logical.data);
                let data = normalized.as_deref().unwrap_or(&logical.data);
                if let Some(plan) = dex::fields::field_plan(data, descriptor)? {
                    plans.push(plan.render_json());
                }
            }
            Ok(plans)
        })?;
        if let Some(plan) = plans.into_iter().next() {
            return Ok(Some(plan));
        }
    }
    Ok(None)
}

pub fn list_entries(path: &Path) -> Result<Vec<ZipEntry>> {
    let archive = Archive::open(path)?;
    if is_bare_dex(&archive) {
        // `BytesSource` is the archive's byte-count, and every build implements it.
        return Ok(vec![ZipEntry::bare(
            "classes.dex",
            crate::zip::BytesSource::source_len(&archive),
        )]);
    }
    crate::zip::parse_zip_entries(&archive, |_| true)
}

pub fn read_entry(path: &Path, wanted: &str) -> Result<Vec<u8>> {
    let archive = Archive::open(path)?;
    // CPython's `zipfile` resolves a name to the *last* central-directory record
    // carrying it, and the reference implementation reads the manifest through
    // it; matching that keeps a crafted archive with two AndroidManifest.xml
    // entries resolving the same way in both tools.
    let entry = crate::zip::parse_zip_entries(&archive, |name| name == wanted.as_bytes())?
        .into_iter()
        .last()
        .with_context(|| format!("{wanted} not found in APK"))?;
    inflate_entry(&archive, &entry, crate::zip::max_inflated_entry())
}

/// Whether the source is a DEX file rather than an archive.
///
/// A DEX names itself in its first eight bytes (`dex\n039\0`). That magic is the whole
/// test, and it is enough: a ZIP starts with `PK`, so no input can be both, and a file
/// that merely begins with those bytes is reporting itself as a DEX.
fn is_bare_dex<S: crate::zip::BytesSource + ?Sized>(source: &S) -> bool {
    let Ok(head) = source.range(0, 8) else {
        return false;
    };
    head.len() == 8
        && head.starts_with(b"dex\n")
        && head[4..7].iter().all(u8::is_ascii_digit)
        && head[7] == 0
}

/// Root DEX entries, `classes*.dex` preferred.
///
/// The reference implementation falls back to any root `*.dex` when an APK names
/// its DEX files differently, and matching that keeps both tools reporting the
/// same entries for the same archive.
///
/// A bare DEX is one entry. Every command addresses an archive, and rather than
/// teach each of them a second input shape, the file is presented as the entry it
/// would have been inside one - named `classes.dex`, which is the name a host
/// shows and the name the reference would have found.
fn parse_dex_entries<S: crate::zip::BytesSource + ?Sized>(source: &S) -> Result<Vec<ZipEntry>> {
    if is_bare_dex(source) {
        return Ok(vec![ZipEntry::bare("classes.dex", source.source_len())]);
    }
    let named = crate::zip::parse_zip_entries(source, |name| {
        name.starts_with(b"classes") && name.ends_with(b".dex") && !name.contains(&b'/')
    })?;
    if !named.is_empty() {
        return Ok(dedup_names(named));
    }
    Ok(dedup_names(crate::zip::parse_zip_entries(
        source,
        |name| name.ends_with(b".dex") && !name.contains(&b'/'),
    )?))
}

/// Keeps the first entry for each name.
///
/// A ZIP may hold two entries with the same name (crafted or repacked archives
/// do). The reference implementation keeps the first `classes*.dex` it meets
/// while scanning the central directory, so a class defined only in a duplicate
/// entry stays invisible there; scanning both copies would instead report every
/// row twice and pick up classes the reference never sees.
fn dedup_names(entries: Vec<crate::zip::ZipEntry>) -> Vec<crate::zip::ZipEntry> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(entry.name.clone()))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::tests::{build_zip, temp_apk};

    /// The rendered lines for a query, which is what the text mode prints.
    fn find_references(
        path: &Path,
        query: &Query,
        threads: usize,
        debug: bool,
    ) -> Result<Vec<String>> {
        Ok(find_reference_hits(path, query, threads, debug)?
            .iter()
            .map(ReferenceHit::render)
            .collect())
    }

    /// Real-data check: one class's plan against values taken from the same APK with
    /// the DEX-metadata reader CheapTrick used before this moved here.
    ///
    /// Gated like the other real-data checks - an absent fixture must not read as a
    /// pass. Run: `RASC_REAL_APK=… cargo test -- --ignored real_apk_fields_plan`.
    #[test]
    #[ignore = "需要真实多 DEX APK；设 RASC_REAL_APK 后用 --ignored 显式运行"]
    fn real_apk_fields_plan_reports_a_known_class() {
        let path = std::env::var("RASC_REAL_APK").expect("set RASC_REAL_APK");
        let plan = field_plan(
            &[PathBuf::from(path)],
            "Lcom/termux/terminal/TerminalSession;",
            4,
        )
        .expect("read the plan")
        .expect("the class is defined");
        assert!(plan.starts_with(
            "{\"schema\":\"rasc.fields-plan/v1\",\"descriptor\":\"Lcom/termux/terminal/TerminalSession;\""
        ));
        assert!(plan.contains("\"counts\":{\"instance\":16,\"static\":3}"));
        assert!(plan.contains("\"instance_ref_mask\":\"0xe5ff\""));
        assert!(plan.contains(
            "{\"field_index\":175,\"name\":\"mArgs\",\"type\":\"[Ljava/lang/String;\",\"access_flags\":18}"
        ));
    }

    #[test]
    fn dex_entries_fall_back_to_any_root_dex_name() {
        let dex = dex::tests::const_string_fixture(1);
        let zip = build_zip(&[("app.dex", &dex, true), ("assets/other.dex", &dex, false)]);
        let path = temp_apk("fallback", &zip);
        let rows =
            find_references(&path, &Query::String("Authorization".to_owned()), 2, false).unwrap();
        assert_eq!(rows, ["app.dex | LFixture0;->m0 | matched=(Authorization)"]);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn duplicate_classes_dex_entries_use_the_first_copy() {
        let first = dex::tests::const_string_fixture(1);
        let second = dex::tests::const_string_fixture(2);
        let zip = build_zip(&[
            ("classes.dex", &first, true),
            ("classes.dex", &second, true),
        ]);
        let path = temp_apk("duplicate-classes", &zip);
        let rows =
            find_references(&path, &Query::String("Authorization".to_owned()), 2, false).unwrap();
        assert_eq!(
            rows,
            ["classes.dex | LFixture0;->m0 | matched=(Authorization)"]
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn duplicate_fallback_dex_entries_use_the_first_copy() {
        let first = dex::tests::const_string_fixture(1);
        let second = dex::tests::const_string_fixture(2);
        let zip = build_zip(&[("app.dex", &first, true), ("app.dex", &second, true)]);
        let path = temp_apk("duplicate-fallback", &zip);
        let rows =
            find_references(&path, &Query::String("Authorization".to_owned()), 2, false).unwrap();
        assert_eq!(rows, ["app.dex | LFixture0;->m0 | matched=(Authorization)"]);
        std::fs::remove_file(&path).unwrap();
    }

    /// The filter a host moves into the engine has to keep showing what it showed.
    #[test]
    fn the_filter_ignores_case_without_changing_the_answer() {
        for (haystack, needle, expected) in [
            ("Authorization", "auth", true),
            ("Authorization", "AUTH", true),
            ("Authorization", "xyz", false),
            ("", "", true),
            ("short", "much longer needle", false),
            // Non-ASCII takes the Unicode fold, which is what a JS `toLowerCase` did.
            ("Grüße", "GRÜSSE", false),
            ("Grüße", "grüße", true),
            ("ключ 🔑", "КЛЮЧ", true),
            ("漢字", "漢", true),
        ] {
            assert_eq!(
                contains_ignore_case(haystack, &needle.to_lowercase()),
                expected,
                "{haystack:?} / {needle:?}"
            );
        }
    }

    /// The byte-level pre-filter answers only what bytes can answer, and says so.
    #[test]
    fn the_raw_filter_defers_anything_it_cannot_answer() {
        // ASCII on both sides is a comparison.
        assert_eq!(raw_contains_ignore_case(b"Authorization header", "auth"), Some(true));
        assert_eq!(raw_contains_ignore_case(b"Authorization header", "zzz"), Some(false));
        assert_eq!(raw_contains_ignore_case(b"anything", ""), Some(true));
        assert_eq!(raw_contains_ignore_case(b"short", "much longer needle"), Some(false));
        // Anything else is the caller's to decode and fold, never a guess.
        assert_eq!(raw_contains_ignore_case("ключ".as_bytes(), "ключ"), None);
        assert_eq!(raw_contains_ignore_case(b"plain ascii", "ключ"), None);
        assert_eq!(raw_contains_ignore_case("漢字".as_bytes(), "漢"), None);
    }

    /// A page is the first matches, in order, and the walk stops once it has them.
    #[test]
    fn a_filtered_page_is_the_first_matches_in_order() {
        let dex = dex::tests::const_string_fixture(4);
        let zip = build_zip(&[("classes.dex", &dex, true)]);
        let path = temp_apk("strings-page", &zip);

        let all = list_strings(&path, 2, false, None, None, None).unwrap();
        let every = |needle: &str| -> Vec<String> {
            let needle = needle.to_lowercase();
            all.iter()
                .filter(|entry| entry.value.to_lowercase().contains(&needle))
                .map(|entry| entry.value.clone())
                .collect()
        };

        for (needle, limit) in [("fixture", 3usize), ("m", 2), ("AUTHORIZATION", 1), ("nothing", 5)] {
            let page = list_strings(&path, 2, false, Some(needle), Some(limit), None).unwrap();
            let expected: Vec<String> = every(needle).into_iter().take(limit).collect();
            assert_eq!(
                page.iter().map(|entry| entry.value.clone()).collect::<Vec<_>>(),
                expected,
                "{needle:?} limit {limit}"
            );
        }

        // An offset with a filter is the next page of the same search, which is the whole
        // reason it exists: a host that scrolls asks for what follows what it has. An offset
        // past the end is an empty page, not an error.
        for (needle, offset, limit) in
            [("fixture", 1usize, 2usize), ("m", 2, 1), ("AUTHORIZATION", 0, 1), ("fixture", 99, 5)]
        {
            let page =
                list_strings(&path, 2, false, Some(needle), Some(limit), Some(offset)).unwrap();
            let expected: Vec<String> = every(needle).into_iter().skip(offset).take(limit).collect();
            assert_eq!(
                page.iter().map(|entry| entry.value.clone()).collect::<Vec<_>>(),
                expected,
                "{needle:?} offset {offset} limit {limit}"
            );
        }

        // A limit with no filter is the head of the table.
        let head = list_strings(&path, 2, false, None, Some(2), None).unwrap();
        assert_eq!(head.len(), 2);
        assert_eq!(head[0].value, all[0].value);
        assert_eq!(head[1].value, all[1].value);

        std::fs::remove_file(&path).unwrap();
    }

    /// A full page stops before the entries below it, and does not even inflate them.
    ///
    /// The second entry's directory record overstates its size, so *inflating* it fails
    /// with a size mismatch. That is what makes "was this entry read?" observable at all:
    /// the limited run passing proves the walk stopped before it, and the unlimited run
    /// failing proves the entry really was there to be read.
    #[test]
    fn a_full_page_never_reads_the_entries_below_it() {
        let dex = dex::tests::const_string_fixture(2);
        let mut zip = build_zip(&[
            ("classes.dex", &dex, true),
            ("classes2.dex", &dex, true),
        ]);
        overstate_second_entry_size(&mut zip);
        let path = temp_apk("strings-stop", &zip);

        let page = list_strings(&path, 1, false, None, Some(1), None).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].index, 0);

        // Without the limit the walk has to inflate that entry and cannot.
        let all = list_strings(&path, 1, false, None, None, None);
        assert!(all.is_err(), "the second entry should not inflate: {all:?}");

        std::fs::remove_file(&path).unwrap();
    }

    /// Adds one byte to the second central-directory entry's uncompressed size.
    fn overstate_second_entry_size(zip: &mut [u8]) {
        let mut seen = 0;
        for offset in 0..zip.len().saturating_sub(4) {
            if zip[offset..offset + 4] == [0x50, 0x4b, 0x01, 0x02] {
                seen += 1;
                if seen == 2 {
                    let size = u32::from_le_bytes(zip[offset + 24..offset + 28].try_into().unwrap()) + 1;
                    zip[offset + 24..offset + 28].copy_from_slice(&size.to_le_bytes());
                    return;
                }
            }
        }
        panic!("the fixture's second central-directory entry was not found");
    }

    /// A DEX that arrives without an archive around it is the entry it would have been.
    #[test]
    fn a_bare_dex_is_one_stored_entry() {
        let dex = dex::tests::const_string_fixture(2);
        let entries = parse_dex_entries(dex.as_slice()).unwrap();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.name, "classes.dex");
        assert_eq!(entry.compression, 0);
        assert_eq!(entry.uncompressed_size, dex.len());
        assert_eq!(
            crate::zip::inflate_entry(dex.as_slice(), entry, usize::MAX).unwrap(),
            dex
        );
    }

    /// The magic is the whole test: a ZIP starts with `PK`, so nothing is both.
    #[test]
    fn a_zip_is_not_read_as_a_bare_dex() {
        let dex = dex::tests::const_string_fixture(1);
        let zip = build_zip(&[("classes.dex", &dex, true)]);
        assert_eq!(parse_dex_entries(zip.as_slice()).unwrap().len(), 1);
        assert!(!is_bare_dex(zip.as_slice()));
        assert!(is_bare_dex(dex.as_slice()));
        // A truncated or unrelated file is not a DEX either, and does not panic.
        assert!(!is_bare_dex(&b"dex\n"[..]));
        assert!(!is_bare_dex(&b"dex\nXX9\0"[..]));
    }

    /// Every command answers a bare DEX exactly as it answers the same DEX in an APK.
    #[test]
    fn bare_and_wrapped_dex_agree_on_every_command() {
        let dex = dex::tests::const_string_fixture(3);
        let zip = build_zip(&[("classes.dex", &dex, true)]);
        let bare = temp_apk("bare", &dex);
        let wrapped = temp_apk("wrapped", &zip);

        assert_eq!(
            list_classes(&bare, 2, false).unwrap(),
            list_classes(&wrapped, 2, false).unwrap()
        );
        assert_eq!(
            find_references(&bare, &Query::String("Authorization".to_owned()), 2, false).unwrap(),
            find_references(
                &wrapped,
                &Query::String("Authorization".to_owned()),
                2,
                false
            )
            .unwrap()
        );
        assert_eq!(
            decompile_class(&bare, "LFixture1;", 2, false).unwrap(),
            decompile_class(&wrapped, "LFixture1;", 2, false).unwrap()
        );

        std::fs::remove_file(&bare).unwrap();
        std::fs::remove_file(&wrapped).unwrap();
    }

    #[test]
    fn duplicate_manifest_entries_use_the_last_copy() {
        let zip = build_zip(&[
            ("AndroidManifest.xml", b"first".as_slice(), false),
            ("AndroidManifest.xml", b"second".as_slice(), false),
        ]);
        let path = temp_apk("duplicate-manifest", &zip);
        assert_eq!(read_entry(&path, "AndroidManifest.xml").unwrap(), b"second");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn decompiles_a_class_from_a_synthetic_apk() {
        let dex = dex::tests::const_string_fixture(1);
        let zip = build_zip(&[("classes.dex", &dex, true)]);
        let path = temp_apk("decompile", &zip);
        let (entry, source) = decompile_class(&path, "LFixture0;", 2, false)
            .unwrap()
            .expect("fixture class is found");
        assert_eq!(entry, "classes.dex");
        assert!(source.contains("Fixture0"), "unexpected source: {source}");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn named_dex_entries_win_over_the_fallback() {
        let named = dex::tests::const_string_fixture(1);
        let other = dex::tests::const_string_fixture(2);
        let zip = build_zip(&[("app.dex", &other, true), ("classes.dex", &named, true)]);
        let path = temp_apk("prefer-named", &zip);
        let rows =
            find_references(&path, &Query::String("Authorization".to_owned()), 2, false).unwrap();
        assert_eq!(
            rows,
            ["classes.dex | LFixture0;->m0 | matched=(Authorization)"]
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn dex_entries_are_selected_by_name() {
        let zip = build_zip(&[
            ("classes.dex", b"first", false),
            ("classes2.dex", b"second", false),
            ("assets/classes3.dex", b"nested", false),
            ("AndroidManifest.xml", b"<manifest/>", false),
        ]);
        let names: Vec<String> = parse_dex_entries(&zip)
            .unwrap()
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(
            names,
            ["classes.dex", "classes2.dex"],
            "root DEX entries only"
        );
    }

    #[test]
    fn find_references_searches_every_logical_dex_in_a_041_container() {
        let container = dex::tests::dex041_container(&[2, 1]);
        let zip = build_zip(&[("classes.dex", &container, true)]);
        let path = temp_apk("dex041", &zip);
        let rows =
            find_references(&path, &Query::String("Authorization".to_owned()), 2, false).unwrap();
        assert_eq!(
            rows,
            [
                "classes.dex!classes1.dex | LFixture0;->m0 | matched=(Authorization)",
                "classes.dex!classes1.dex | LFixture1;->m1 | matched=(Authorization)",
                "classes.dex!classes2.dex | LFixture0;->m0 | matched=(Authorization)",
            ]
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn decompiles_a_class_from_every_logical_dex_of_a_041_container() {
        // A DEX 041 container overlays each member's header at offset 0, so the
        // Adler-32 a member stores no longer covers the bytes the decompiler
        // sees. findrefs never validates it (our scanner does not); the
        // decompiler does, so this test guards the second member too.
        // Member one defines LFixture0;, member two LFixture0; and LFixture1;.
        let container = dex::tests::dex041_container(&[1, 2]);
        let zip = build_zip(&[("classes.dex", &container, true)]);
        let path = temp_apk("dex041-getclass", &zip);
        for (descriptor, expected_entry) in [
            ("LFixture0;", "classes.dex!classes1.dex"),
            ("LFixture1;", "classes.dex!classes2.dex"),
        ] {
            let hit = decompile_class(&path, descriptor, 2, false).unwrap();
            let (entry, source) = hit.unwrap_or_else(|| panic!("{descriptor} did not decompile"));
            assert!(entry.starts_with(expected_entry), "found in {entry}");
            assert!(source.contains("Fixture"), "unexpected source: {source}");
        }
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn an_entry_that_is_not_a_dex_is_a_clean_error() {
        // A packer can leave junk in a classes*.dex entry, and a corrupted magic
        // looks the same. rasc reports that loudly instead of guessing: the
        // reference's findrefs errors on such archives too (its class lookup
        // skips them silently), and silently analysing "the rest" would hide the
        // fact that part of the archive was not analysed at all. What this test
        // pins is that the failure is a message with an exit code, not a panic and
        // not an empty result.
        let good = dex::tests::const_string_fixture(1);
        let zip = build_zip(&[
            ("classes.dex", b"JUNKJUNKJUNK".as_slice(), false),
            ("classes2.dex", &good, true),
        ]);
        let path = temp_apk("junk-dex", &zip);
        let error = find_references(&path, &Query::String("Authorization".to_owned()), 2, false)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("invalid DEX header"),
            "unexpected error: {error}"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn find_class_finds_a_class_whose_type_id_is_a_later_duplicate() {
        // A DEX may repeat a descriptor across several type ids. `list_classes`
        // resolves each class_def's own type id, so `find_class` has to do the
        // same: looking up "the" type id for the descriptor makes the two
        // commands contradict each other on such a file.
        let mut dex = dex::tests::const_string_fixture(2);
        let header_size = crate::bytes::read_u32(&dex, 0x24).unwrap() as usize;
        let strings = crate::bytes::read_u32(&dex, 0x38).unwrap() as usize;
        // type_ids[0] belongs to no class_def; pointing it at "LFixture0;"
        // (string index 1) makes the descriptor's first match a different id than
        // the one the class's class_def carries.
        crate::zip::tests::write_u32(&mut dex, header_size + strings * 4, 1);
        let zip = build_zip(&[("classes.dex", &dex, true)]);
        let path = temp_apk("duplicate-type-id", &zip);
        let listed = list_classes(&path, 2, false).unwrap();
        assert!(
            listed.iter().any(|entry| entry.descriptor == "LFixture0;"),
            "fixture stopped listing the class: {listed:?}"
        );
        let hit = map_class_hit(&path, "LFixture0;", 2, false, |data| Ok(data.to_vec())).unwrap();
        assert!(
            hit.is_some(),
            "listed by classes but missing from the lookup"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn class_index_reads_041_containers_through_the_full_path() {
        // The prefix reader refuses 041 containers (their members overlay headers),
        // so this pins that the fallback still lists every logical member.
        let container = dex::tests::dex041_container(&[2, 1]);
        let zip = build_zip(&[("classes.dex", &container, true)]);
        let path = temp_apk("dex041-classes", &zip);
        let listed = list_classes(&path, 2, false).unwrap();
        // Member one defines LFixture0; and LFixture1;, member two repeats LFixture0;,
        // and the index dedups by descriptor, so the two members' names survive as
        // the first member's (the prefixed name proves the full path ran).
        let names: Vec<&str> = listed.iter().map(|entry| &*entry.dex_name).collect();
        assert_eq!(
            names,
            ["classes.dex!classes1.dex", "classes.dex!classes1.dex"],
            "{listed:?}"
        );
        let descriptors: Vec<&str> = listed
            .iter()
            .map(|entry| entry.descriptor.as_str())
            .collect();
        assert_eq!(descriptors, ["LFixture0;", "LFixture1;"], "{listed:?}");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn single_member_041_container_reports_the_plain_dex_name() {
        // The reference implementation only renames container members when there
        // is more than one, so a single-member container keeps the entry name in
        // every row (the dex column is part of a row's identity).
        let container = dex::tests::dex041_container(&[1]);
        let zip = build_zip(&[("classes.dex", &container, true)]);
        let path = temp_apk("dex041-single", &zip);
        let rows =
            find_references(&path, &Query::String("Authorization".to_owned()), 2, false).unwrap();
        assert_eq!(
            rows,
            ["classes.dex | LFixture0;->m0 | matched=(Authorization)"]
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn find_references_runs_end_to_end_on_a_synthetic_apk() {
        let first = dex::tests::const_string_fixture(2);
        let second = dex::tests::const_string_fixture(1);
        let zip = build_zip(&[
            ("classes.dex", &first, true),
            ("classes2.dex", &second, false),
            ("AndroidManifest.xml", b"<manifest/>", false),
        ]);
        let path = temp_apk("synthetic", &zip);
        let query = Query::String("Authorization".to_owned());
        let rows = find_references(&path, &query, 2, false).unwrap();
        assert_eq!(
            rows,
            [
                "classes.dex | LFixture0;->m0 | matched=(Authorization)",
                "classes.dex | LFixture1;->m1 | matched=(Authorization)",
                "classes2.dex | LFixture0;->m0 | matched=(Authorization)",
            ]
        );
        assert_eq!(
            find_references(&path, &query, 2, false).unwrap(),
            rows,
            "deterministic"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn worker_count_must_be_positive() {
        let zip = build_zip(&[("classes.dex", b"payload", false)]);
        let path = temp_apk("workers", &zip);
        let query = Query::String("x".to_owned());
        assert!(find_references(&path, &query, 0, false).is_err());
        std::fs::remove_file(&path).unwrap();
    }

    /// The inline key must not change the order: every pair has to compare the same way
    /// with the key as the plain `(descriptor, dex_name)` tuple does - including
    /// descriptors that are prefixes of each other, ones that share the first eight
    /// bytes, and ones with multi-byte characters.
    #[test]
    fn the_inline_sort_key_orders_like_the_descriptor() {
        let samples = [
            "L;",
            "La;",
            "La;b",
            "Lab;",
            "Lcom/a;",
            "Lcom/ab;",
            "Lcom/a/b;",
            "Lcom/b;",
            "Lcom/bytedance/aaa;",
            "Lcom/bytedance/aab;",
            "Lcom/bytedance/zzz;",
            "Lé;",
            "L中;",
            "L\u{1F600};",
            "Lcom/example/Main$Nested;",
            "Lcom/example/Main$Nested$1;",
        ];
        let entries: Vec<ClassEntry> = samples
            .iter()
            .enumerate()
            .map(|(index, descriptor)| {
                ClassEntry::new((*descriptor).to_owned(), format!("classes{index}.dex").into())
            })
            .collect();
        for left in &entries {
            for right in &entries {
                let plain = left
                    .descriptor
                    .cmp(&right.descriptor)
                    .then_with(|| left.dex_name.cmp(&right.dex_name));
                assert_eq!(
                    left.cmp(right),
                    plain,
                    "{:?} vs {:?}",
                    left.descriptor,
                    right.descriptor
                );
            }
        }
        // A sorted copy of the samples must match sorting by the plain key.
        let mut sorted = entries.clone();
        sorted.sort_unstable();
        let mut plain: Vec<ClassEntry> = entries;
        plain.sort_by(|left, right| {
            left.descriptor
                .cmp(&right.descriptor)
                .then_with(|| left.dex_name.cmp(&right.dex_name))
        });
        assert_eq!(
            sorted.iter().map(|e| &e.descriptor).collect::<Vec<_>>(),
            plain.iter().map(|e| &e.descriptor).collect::<Vec<_>>()
        );
    }

    /// In-process timing for the container paths: the counterpart of the scanner test in
    /// `dex`, for changes the end-to-end harness cannot resolve (its ~3.8 ms of `fork/exec`
    /// and start-up per stage dominate a `manifest`-sized stage).
    ///
    /// Needs a real archive; skips when the environment does not name one:
    ///
    ///   RASC_BENCH_APK=/path/app.apk cargo test --release -- --ignored --nocapture inproc_paths
    ///
    /// Compare two builds **back to back in one session** - the same binary drifts 5-10%
    /// between sessions, which is more than most changes here are worth.
    #[test]
    #[ignore = "timing harness: run with --release --ignored --nocapture and RASC_BENCH_APK"]
    fn inproc_paths() {
        let Ok(apk) = std::env::var("RASC_BENCH_APK") else {
            println!("[inproc] RASC_BENCH_APK not set, skipping");
            return;
        };
        let path = std::path::Path::new(&apk);
        const CALLS: u32 = 5;

        // Warm each path once (page cache, allocator, the thread pool), then time the
        // repeats. Each call includes what a command would pay once: opening the archive,
        // walking its central directory and building a one-thread pool.
        let _ = read_entry(path, "AndroidManifest.xml").unwrap();
        let started = Instant::now();
        for _ in 0..CALLS {
            let _ = read_entry(path, "AndroidManifest.xml").unwrap();
        }
        println!(
            "[inproc] read_entry(manifest): {:.1} us/call",
            started.elapsed().as_secs_f64() * 1e6 / f64::from(CALLS)
        );

        // The decode half of the manifest command, which is where the vendored AXML
        // parser's per-attribute allocations live: `read_entry` above is only the bytes.
        let manifest_bytes = read_entry(path, "AndroidManifest.xml").unwrap();
        let _ = crate::manifest::decode(&manifest_bytes).unwrap();
        let started = Instant::now();
        for _ in 0..CALLS {
            let _ = crate::manifest::decode(&manifest_bytes).unwrap();
        }
        println!(
            "[inproc] manifest::decode: {:.1} us/call ({} KiB of AXML)",
            started.elapsed().as_secs_f64() * 1e6 / f64::from(CALLS),
            manifest_bytes.len() / 1024
        );

        let classes = list_classes(path, 1, false).unwrap();
        let started = Instant::now();
        for _ in 0..CALLS {
            let _ = list_classes(path, 1, false).unwrap();
        }
        println!(
            "[inproc] list_classes(1 thread): {:.1} us/call ({} classes)",
            started.elapsed().as_secs_f64() * 1e6 / f64::from(CALLS),
            classes.len()
        );

        if let Some(first) = classes.first() {
            let descriptor = first.descriptor.clone();
            let _ = decompile_class(path, &descriptor, 1, false).unwrap();
            let started = Instant::now();
            for _ in 0..CALLS {
                let _ = decompile_class(path, &descriptor, 1, false).unwrap();
            }
            println!(
                "[inproc] decompile_class({descriptor}): {:.1} us/call",
                started.elapsed().as_secs_f64() * 1e6 / f64::from(CALLS)
            );
        }
    }

    #[test]
    fn class_entry_exposes_name_components() {
        let entry = ClassEntry::new("Lcom/example/Main$Nested;".to_owned(), "classes.dex".into());
        assert_eq!(entry.java_name(), "com/example/Main$Nested");
        assert_eq!(entry.package(), "com/example");
        assert_eq!(entry.simple_name(), "Main$Nested");
    }
}
