//! H02 public input-module contract: immutable snapshot, entry metadata, root
//! selection, path policy, budgets and classified damage.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use diana::analysis::apk;
use diana::analysis::archive::{EntryView, crc32};
use diana::analysis::input::{
    ArchiveProblem, BytesSource, EntryIndex, InputError, InputKind, InputLimit, InputLimits,
    InputOptions, open_input,
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "diana-h02-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
    fn path(&self) -> &Path {
        &self.path
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct ZipEntry<'a> {
    name: &'a str,
    /// Uncompressed payload; it determines the CRC and declared size.
    data: &'a [u8],
    /// Bytes actually stored in the entry; defaults to `data` for stored records.
    compressed: Option<&'a [u8]>,
    method: u16,
    flags: u16,
    crc: Option<u32>,
}

fn stored_entry<'a>(name: &'a str, data: &'a [u8]) -> ZipEntry<'a> {
    ZipEntry {
        name,
        data,
        compressed: None,
        method: 0,
        flags: 0,
        crc: None,
    }
}

fn zip(entries: &[ZipEntry<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for entry in entries {
        let offset = u32::try_from(out.len()).unwrap();
        let name = entry.name.as_bytes();
        let crc = entry.crc.unwrap_or_else(|| crc32(entry.data));
        let stored = entry.compressed.unwrap_or(entry.data);
        let size = u32::try_from(stored.len()).unwrap();
        let uncompressed = u32::try_from(entry.data.len()).unwrap();
        let mut local = Vec::new();
        local.extend_from_slice(b"PK\x03\x04");
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&entry.flags.to_le_bytes());
        local.extend_from_slice(&entry.method.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&uncompressed.to_le_bytes());
        local.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name);
        local.extend_from_slice(stored);
        out.extend_from_slice(&local);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&entry.flags.to_le_bytes());
        central.extend_from_slice(&entry.method.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&uncompressed.to_le_bytes());
        central.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let directory_offset = u32::try_from(out.len()).unwrap();
    out.extend_from_slice(&central);
    let directory_size = u32::try_from(central.len()).unwrap();
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&directory_size.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn archive_payload(lease: &diana::analysis::input::InputLease, index: EntryIndex) -> &[u8] {
    let entry = &lease.archive().unwrap().entries()[index.0];
    diana::analysis::archive::entry_payload(lease.bytes(), entry).unwrap()
}

fn options(limits: InputLimits) -> InputOptions {
    InputOptions {
        limits,
        allowed_root: None,
    }
}

fn dex(payload: u8) -> Vec<u8> {
    let mut bytes = b"dex\n035\0".to_vec();
    bytes.extend_from_slice(&[payload; 16]);
    bytes
}

#[test]
fn h02_ordinary_file_bare_dex_and_stored_archive_share_one_lease_model() {
    let dir = TempDir::new("lease");
    // Standard CRC-32 vector, independent of our own writer.
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);

    let bare = dex(0x11);
    let bare_path = dir.file("case.dex", &bare);
    let lease = open_input(&bare_path, &options(InputLimits::default())).unwrap();
    assert_eq!(lease.kind(), InputKind::BareDex);
    assert_eq!(lease.bytes(), bare.as_slice());
    assert_eq!(lease.identity().len().0, u64::try_from(bare.len()).unwrap());
    assert_eq!(
        lease.identity().path(),
        fs::canonicalize(&bare_path).unwrap()
    );
    let roots = apk::roots(&lease).unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].label(), "case.dex");
    assert!(roots[0].entry().is_none());
    assert_eq!(roots[0].bytes(), bare.as_slice());

    let opaque_path = dir.file("notes.bin", b"not a container");
    let opaque = open_input(&opaque_path, &options(InputLimits::default())).unwrap();
    assert_eq!(opaque.kind(), InputKind::Opaque);
    assert!(matches!(apk::roots(&opaque), Err(InputError::NotDex)));

    let first = dex(0x21);
    let second = dex(0x22);
    let archive_path = dir.file(
        "sample.apk",
        &zip(&[
            stored_entry("classes.dex", &first),
            stored_entry("assets/readme.txt", b"readme"),
            stored_entry("classes2.dex", &second),
        ]),
    );
    let lease = open_input(&archive_path, &options(InputLimits::default())).unwrap();
    assert_eq!(lease.kind(), InputKind::Archive);
    let directory = lease.archive().unwrap();
    let records = directory.entries();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].name(), "classes.dex");
    assert_eq!(records[2].name(), "classes2.dex");
    assert_eq!(records[1].method(), 0);
    assert_eq!(
        records[1].uncompressed_size().0,
        u64::try_from(b"readme".len()).unwrap()
    );
    assert_eq!(records[1].crc32(), crc32(b"readme"));
    let view = EntryView::from_lease(&lease, EntryIndex(1)).unwrap();
    assert_eq!(view.bytes(), b"readme");
    assert_eq!(view.record().name(), "assets/readme.txt");

    let roots = apk::roots(&lease).unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].label(), "classes.dex");
    assert_eq!(roots[0].bytes(), first.as_slice());
    assert_eq!(roots[1].label(), "classes2.dex");
    assert_eq!(roots[1].bytes(), second.as_slice());
    assert!(apk::is_root_name("classes.dex"));
    assert!(apk::is_root_name("classes17.dex"));
    assert!(!apk::is_root_name("classes0.dex"));
    assert!(!apk::is_root_name("classes1.dex"));
    assert!(!apk::is_root_name("assets/classes3.dex"));
}

#[test]
fn h02_the_lease_snapshot_is_isolated_from_later_source_changes() {
    let dir = TempDir::new("snapshot");
    let original = dex(0x31);
    let path = dir.file("case.dex", &original);
    let lease = open_input(&path, &options(InputLimits::default())).unwrap();
    fs::write(&path, b"").unwrap();
    assert_eq!(lease.bytes(), original.as_slice());
    assert_eq!(
        lease.identity().len().0,
        u64::try_from(original.len()).unwrap()
    );
    drop(lease);
    // Dropping the lease releases only the owned copy; the user's file is intact
    // and a later request observes its current content instead of a cache.
    assert!(path.is_file());
    fs::write(&path, dex(0x32)).unwrap();
    let reopened = open_input(&path, &options(InputLimits::default())).unwrap();
    assert_eq!(reopened.bytes(), dex(0x32).as_slice());
}

#[cfg(unix)]
#[test]
fn h02_path_policy_rejects_escapes_and_non_regular_inputs() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new("policy");
    let root = dir.path().join("root");
    fs::create_dir_all(&root).unwrap();
    let inside = root.join("inside.dex");
    fs::write(&inside, dex(0x41)).unwrap();
    let outside = dir.file("outside.dex", &dex(0x42));
    let escape = root.join("escape.dex");
    symlink(&outside, &escape).unwrap();

    let limited = || InputOptions {
        limits: InputLimits::default(),
        allowed_root: Some(root.clone()),
    };
    assert!(open_input(&inside, &limited()).is_ok());
    assert!(matches!(
        open_input(&escape, &limited()),
        Err(InputError::OutsideRoot(_))
    ));
    // Without a configured root the ordinary resolution policy applies.
    assert!(open_input(&escape, &options(InputLimits::default())).is_ok());

    assert!(matches!(
        open_input(&root, &options(InputLimits::default())),
        Err(InputError::NotRegularFile(_))
    ));
    let device = Path::new("/dev/null");
    if device.exists() {
        assert!(matches!(
            open_input(device, &options(InputLimits::default())),
            Err(InputError::NotRegularFile(_))
        ));
    }
    let fifo = dir.path().join("pipe.dex");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo is required to construct the FIFO counterexample");
    assert!(status.success());
    // A FIFO must be rejected from metadata, so this call must not block.
    let (sender, receiver) = std::sync::mpsc::channel();
    let probe = fifo.clone();
    std::thread::spawn(move || {
        let result = open_input(&probe, &options(InputLimits::default()));
        let _ = sender.send(matches!(result, Err(InputError::NotRegularFile(_))));
    });
    assert!(
        receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("opening a FIFO blocked instead of rejecting its file type")
    );
}

#[test]
fn h02_budgets_are_applied_before_the_snapshot_or_directory_is_trusted() {
    let dir = TempDir::new("budget");
    let payload = dex(0x51);
    let path = dir.file("case.dex", &payload);
    let len = u64::try_from(payload.len()).unwrap();
    assert!(matches!(
        open_input(
            &path,
            &options(InputLimits {
                max_input_bytes: Some(len - 1),
                ..InputLimits::default()
            })
        ),
        Err(InputError::BudgetExceeded {
            limit: InputLimit::InputBytes,
            requested,
            maximum,
        }) if requested == len && maximum == len - 1
    ));
    assert!(
        open_input(
            &path,
            &options(InputLimits {
                max_input_bytes: Some(len),
                ..InputLimits::default()
            })
        )
        .is_ok()
    );

    let archive = zip(&[
        stored_entry("classes.dex", &dex(0x52)),
        stored_entry("classes2.dex", &dex(0x53)),
    ]);
    let archive_path = dir.file("two.apk", &archive);
    assert!(matches!(
        open_input(
            &archive_path,
            &options(InputLimits {
                max_entries: Some(1),
                ..InputLimits::default()
            })
        ),
        Err(InputError::MalformedArchive(
            ArchiveProblem::EntryCountExceeded {
                requested: 2,
                maximum: 1
            }
        ))
    ));
    let total = u64::try_from(dex(0x52).len() + dex(0x53).len()).unwrap();
    assert!(matches!(
        open_input(
            &archive_path,
            &options(InputLimits {
                max_total_entry_bytes: Some(total - 1),
                ..InputLimits::default()
            })
        ),
        Err(InputError::MalformedArchive(
            ArchiveProblem::TotalEntryBytesExceeded { maximum, .. }
        )) if maximum == total - 1
    ));
    assert!(
        open_input(
            &archive_path,
            &options(InputLimits {
                max_entries: Some(2),
                max_total_entry_bytes: Some(total),
                ..InputLimits::default()
            })
        )
        .is_ok()
    );
}

#[test]
fn h02_entry_damage_and_unsupported_methods_keep_their_classified_reason() {
    let dir = TempDir::new("entries");
    let payload = dex(0x61);
    let archive = zip(&[stored_entry("classes.dex", &payload)]);
    let path = dir.file("damaged.apk", &archive);
    let lease = open_input(&path, &options(InputLimits::default())).unwrap();
    let directory = lease.archive().unwrap();
    let entry = directory.entries()[0].clone();
    let good = archive_payload(&lease, EntryIndex(0));
    assert_eq!(good, payload.as_slice());

    let mut damaged = archive.clone();
    let name_at = archive
        .windows(11)
        .position(|window| window == b"classes.dex")
        .unwrap();
    damaged[name_at + 11] ^= 0xff;
    let damaged_path = dir.file("broken.apk", &damaged);
    let broken = open_input(&damaged_path, &options(InputLimits::default())).unwrap();
    assert!(matches!(
        EntryView::from_lease(&broken, EntryIndex(0)),
        Err(ArchiveProblem::DamagedEntry)
    ));

    let unsupported = zip(&[ZipEntry {
        method: 12,
        ..stored_entry("classes.dex", &payload)
    }]);
    let unsupported_path = dir.file("unsupported.apk", &unsupported);
    let unsupported = open_input(&unsupported_path, &options(InputLimits::default())).unwrap();
    assert!(matches!(
        EntryView::from_lease(&unsupported, EntryIndex(0)),
        Err(ArchiveProblem::UnsupportedCompression(12))
    ));
    assert_eq!(entry.method(), 0);
    assert_eq!(
        entry.uncompressed_size().0,
        u64::try_from(payload.len()).unwrap()
    );

    let encrypted = zip(&[ZipEntry {
        flags: 1,
        ..stored_entry("classes.dex", &payload)
    }]);
    let encrypted_path = dir.file("encrypted.apk", &encrypted);
    let encrypted = open_input(&encrypted_path, &options(InputLimits::default())).unwrap();
    assert!(matches!(
        EntryView::from_lease(&encrypted, EntryIndex(0)),
        Err(ArchiveProblem::EncryptedEntry)
    ));
    assert!(encrypted.archive().unwrap().entries()[0].is_encrypted());
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut compressor = libdeflater::Compressor::new(libdeflater::CompressionLvl::default());
    let mut output = vec![0u8; compressor.deflate_compress_bound(data.len())];
    let written = compressor.deflate_compress(data, &mut output).unwrap();
    output.truncate(written);
    output
}

#[test]
fn h02_deflated_entries_inflate_on_demand_under_the_configured_bound() {
    let dir = TempDir::new("deflate");
    // A repetitive payload makes the deflate stream genuinely smaller.
    let mut payload = dex(0x81);
    payload.extend(std::iter::repeat_n(0x5a_u8, 512));
    let compressed = deflate(&payload);
    assert!(compressed.len() < payload.len());
    let archive = zip(&[ZipEntry {
        method: 8,
        compressed: Some(&compressed),
        ..stored_entry("classes.dex", &payload)
    }]);
    let path = dir.file("deflated.apk", &archive);
    let lease = open_input(&path, &options(InputLimits::default())).unwrap();
    let entry = &lease.archive().unwrap().entries()[0];
    assert_eq!(entry.method(), 8);
    assert_eq!(
        entry.compressed_size().0,
        u64::try_from(compressed.len()).unwrap()
    );
    assert_eq!(
        entry.uncompressed_size().0,
        u64::try_from(payload.len()).unwrap()
    );
    let inflated =
        diana::analysis::archive::read_entry(lease.bytes(), entry, InputLimits::default()).unwrap();
    assert_eq!(inflated.bytes(), payload.as_slice());
    // A deflated root DEX is delivered through the same root selection.
    let roots = apk::roots(&lease).unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].bytes(), payload.as_slice());

    let declared = u64::try_from(payload.len()).unwrap();
    assert!(matches!(
        diana::analysis::archive::read_entry(
            lease.bytes(),
            entry,
            InputLimits {
                max_inflated_bytes: Some(declared - 1),
                ..InputLimits::default()
            }
        ),
        Err(ArchiveProblem::InflatedBytesExceeded { maximum, .. }) if maximum == declared - 1
    ));
    assert!(
        diana::analysis::archive::read_entry(
            lease.bytes(),
            entry,
            InputLimits {
                max_inflated_bytes: Some(declared),
                ..InputLimits::default()
            }
        )
        .is_ok()
    );

    // A damaged deflate stream is classified, not guessed into a payload.
    let mut damaged = archive.clone();
    let data_at = damaged
        .windows(compressed.len())
        .position(|window| window == compressed.as_slice())
        .unwrap();
    damaged[data_at + compressed.len() / 2] ^= 0xff;
    let damaged_path = dir.file("damaged-deflate.apk", &damaged);
    let broken = open_input(&damaged_path, &options(InputLimits::default())).unwrap();
    let broken_entry = &broken.archive().unwrap().entries()[0];
    assert!(matches!(
        diana::analysis::archive::read_entry(broken.bytes(), broken_entry, InputLimits::default()),
        Err(ArchiveProblem::DecompressionFailed) | Err(ArchiveProblem::DamagedEntry)
    ));
}

#[test]
fn h02_truncated_or_replaced_archives_never_publish_a_partial_directory() {
    let dir = TempDir::new("truncated");
    let archive = zip(&[stored_entry("classes.dex", &dex(0x71))]);
    for cut in [4usize, archive.len() / 2, archive.len() - 1] {
        let path = dir.file("cut.apk", &archive[..cut]);
        assert!(
            matches!(
                open_input(&path, &options(InputLimits::default())),
                Err(InputError::MalformedArchive(_))
            ),
            "truncation at {cut} published a directory"
        );
    }
    // An empty file carries no archive signature, so it is leased as opaque input
    // rather than guessed to be a damaged archive.
    let empty = dir.file("empty.apk", b"");
    let lease = open_input(&empty, &options(InputLimits::default())).unwrap();
    assert_eq!(lease.kind(), InputKind::Opaque);
    assert!(matches!(apk::roots(&lease), Err(InputError::NotDex)));
}
