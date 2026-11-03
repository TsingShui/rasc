//! H02 public input-module contract: immutable snapshot, entry metadata, root
//! selection, path policy, budgets and classified damage.

use std::fs;
#[cfg(unix)]
use std::path::Path;

use diana::analysis::apk;
use diana::analysis::archive::{EntryView, crc32};
use diana::analysis::input::{
    ArchiveProblem, BytesSource, EntryIndex, InputError, InputKind, InputLimit, InputLimits,
    InputOptions, open_input,
};
use diana::analysis::request::RequestScope;

mod support;

use support::{TempDir, ZipEntry, dex, stored_entry, zip};

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
fn h02_a_request_scope_owns_the_lease_and_releases_it_without_touching_the_input() {
    let dir = TempDir::new("scope");
    let payload = dex(0x35);
    let archive = zip(&[
        stored_entry("classes.dex", &payload),
        stored_entry("classes2.dex", &dex(0x36)),
    ]);
    let path = dir.file("scope.apk", &archive);
    {
        let scope = RequestScope::open(&path, &options(InputLimits::default())).unwrap();
        assert_eq!(scope.input().kind(), InputKind::Archive);
        let roots = scope.roots().unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].bytes(), payload.as_slice());
        assert_eq!(roots[0].entry().unwrap().name(), "classes.dex");
    }
    // After the request scope is dropped the input is untouched and a later
    // request observes the file's current content instead of a cached snapshot.
    assert_eq!(fs::read(&path).unwrap(), archive);
    fs::write(&path, zip(&[stored_entry("classes.dex", &dex(0x37))])).unwrap();
    let scope = RequestScope::open(&path, &options(InputLimits::default())).unwrap();
    assert_eq!(scope.roots().unwrap()[0].bytes(), dex(0x37).as_slice());
    drop(scope);
    fs::remove_file(&path).unwrap();
    assert!(!path.exists());
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

mod h03 {
    use std::sync::atomic::AtomicBool;

    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::query::{
        Query, QueryError, QueryKind, QueryLimit, QueryLimits, QueryPlan, Row,
    };
    use diana::analysis::request::RequestScope;

    use super::support::{TempDir, dex, stored_entry, zip};

    const FIELD_DEX: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/annotations/field.dex");
    const POLY_DEX: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/dynamic/poly.dex");
    const TEXT_DEX: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/annotations/text.dex");

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    fn plan_for(kind: QueryKind, jobs: usize) -> QueryPlan {
        QueryPlan {
            kind,
            filter: None,
            class: None,
            class_filter: diana::analysis::dex::ClassFilter::default(),
            limits: QueryLimits::default(),
            jobs,
        }
    }

    fn collect(scope: &RequestScope, plan: QueryPlan) -> Vec<Row> {
        let query = Query::new(scope, plan).unwrap();
        let cancel = AtomicBool::new(false);
        let mut rows = Vec::new();
        query
            .run(&cancel, &mut |row| {
                rows.push(row);
                Ok(())
            })
            .unwrap();
        rows
    }

    fn text(units: &[u16]) -> String {
        String::from_utf16(units).unwrap()
    }

    #[test]
    fn h03_defined_members_and_strings_keep_their_original_identity() {
        let dir = TempDir::new("h03-members");
        let path = dir.file("field.dex", FIELD_DEX);
        let scope = RequestScope::open(&path, &options()).unwrap();

        let classes = collect(&scope, plan_for(QueryKind::Classes, 1));
        let mut descriptors: Vec<String> = classes
            .iter()
            .map(|row| match row {
                Row::Class(row) => text(&row.descriptor),
                _ => panic!("classes returned a foreign row"),
            })
            .collect();
        descriptors.sort();
        assert_eq!(descriptors, ["Lfixtures/Field;", "Lfixtures/Mark;"]);

        let methods = collect(&scope, plan_for(QueryKind::Methods, 1));
        let defined: Vec<(u32, String, String)> = methods
            .iter()
            .map(|row| match row {
                Row::Method(row) => (row.class_def.0, text(&row.name), text(&row.prototype)),
                _ => panic!("methods returned a foreign row"),
            })
            .collect();
        // Only class_data definitions are listed; external method_ids references
        // such as Object.<init> never appear as definitions.
        assert_eq!(defined.len(), 4, "{defined:?}");
        assert!(defined.contains(&(0, "read".to_owned(), "()I".to_owned())));
        assert!(defined.contains(&(0, "<init>".to_owned().to_owned(), "()V".to_owned())));
        assert!(defined.contains(&(1, "nums".to_owned(), "()[I".to_owned())));
        assert!(defined.contains(&(1, "value".to_owned(), "()Ljava/lang/String;".to_owned())));

        let fields = collect(&scope, plan_for(QueryKind::Fields, 1));
        assert_eq!(fields.len(), 1);
        match &fields[0] {
            Row::Field(row) => {
                assert_eq!(row.class_def.0, 0);
                assert_eq!(text(&row.name), "n");
                assert_eq!(text(&row.field_type), "I");
                assert_eq!(row.access_flags & 1, 1);
                assert!(matches!(
                    row.storage,
                    diana_decomp::input::FieldStorage::Instance
                ));
            }
            _ => panic!("fields returned a foreign row"),
        }

        let strings = collect(&scope, plan_for(QueryKind::Strings, 1));
        let values: Vec<String> = strings
            .iter()
            .map(|row| match row {
                Row::String(row) => text(&row.units),
                _ => panic!("strings returned a foreign row"),
            })
            .collect();
        assert!(values.contains(&"read".to_owned()));
        assert!(values.contains(&"n".to_owned()));
        // Every row carries the original string_ids index exactly once.
        let indices: Vec<u32> = strings
            .iter()
            .map(|row| match row {
                Row::String(row) => row.string.0,
                _ => panic!("strings returned a foreign row"),
            })
            .collect();
        assert_eq!(
            indices,
            (0..u32::try_from(indices.len()).unwrap()).collect::<Vec<_>>()
        );

        let text_path = dir.file("text.dex", TEXT_DEX);
        let text_scope = RequestScope::open(&text_path, &options()).unwrap();
        let text_rows = collect(&text_scope, plan_for(QueryKind::Strings, 1));
        assert!(text_rows.iter().any(|row| match row {
            Row::String(row) => row.units == [0, 55296, 65, 56320, 92, 117, 48, 48, 48, 97],
            _ => false,
        }));
    }

    #[test]
    fn h03_filters_match_every_record_without_changing_original_identity() {
        let dir = TempDir::new("h03-filter");
        let path = dir.file("field.dex", FIELD_DEX);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let unfiltered = collect(&scope, plan_for(QueryKind::Methods, 1));
        let reference = unfiltered
            .iter()
            .find_map(|row| match row {
                Row::Method(row) if text(&row.name) == "read" => Some((row.class_def, row.method)),
                _ => None,
            })
            .unwrap();
        let mut plan = plan_for(QueryKind::Methods, 1);
        plan.filter = Some("rea".encode_utf16().collect());
        let rows = collect(&scope, plan);
        assert_eq!(rows.len(), 1);
        match &rows[0] {
            Row::Method(row) => {
                assert_eq!(text(&row.name), "read");
                // The filtered row keeps the same original identity as the
                // unfiltered enumeration of the same input.
                assert_eq!((row.class_def, row.method), reference);
            }
            _ => panic!("filter returned a foreign row"),
        }

        let mut plan = plan_for(QueryKind::Fields, 1);
        plan.class = Some(diana::analysis::dex::class_descriptor("fixtures.Mark"));
        assert!(collect(&scope, plan).is_empty());

        let mut plan = plan_for(QueryKind::Fields, 1);
        plan.class = Some(diana::analysis::dex::class_descriptor("fixtures.Field"));
        assert_eq!(collect(&scope, plan).len(), 1);

        let mut plan = plan_for(QueryKind::Classes, 1);
        plan.filter = Some("Mark".encode_utf16().collect());
        let rows = collect(&scope, plan);
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], Row::Class(row) if row.class_def.0 == 1));

        // A supplementary-plane literal must match as UTF-16 units, not bytes.
        let text_path = dir.file("text.dex", TEXT_DEX);
        let text_scope = RequestScope::open(&text_path, &options()).unwrap();
        let mut plan = plan_for(QueryKind::Strings, 1);
        plan.filter = Some(vec![0xd800, 0x0041, 0xdc00]);
        let rows = collect(&text_scope, plan);
        assert_eq!(rows.len(), 1);
        assert!(
            matches!(&rows[0], Row::String(row) if row.units == [0, 55296, 65, 56320, 92, 117, 48, 48, 48, 97])
        );
    }

    #[test]
    fn h03_entries_list_physical_records_and_reject_a_bare_dex() {
        let dir = TempDir::new("h03-entries");
        let archive = zip(&[
            stored_entry("classes.dex", &dex(0x11)),
            stored_entry("assets/a.txt", b"a"),
            stored_entry("classes.dex", &dex(0x12)),
        ]);
        let path = dir.file("dup.apk", &archive);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let rows = collect(&scope, plan_for(QueryKind::Entries, 1));
        let listed: Vec<(usize, String, u16)> = rows
            .iter()
            .map(|row| match row {
                Row::Entry(row) => (row.entry.0, row.name.clone(), row.method),
                _ => panic!("entries returned a foreign row"),
            })
            .collect();
        assert_eq!(
            listed,
            [
                (0, "classes.dex".to_owned(), 0),
                (1, "assets/a.txt".to_owned(), 0),
                (2, "classes.dex".to_owned(), 0),
            ]
        );

        let bare = dir.file("bare.dex", FIELD_DEX);
        let bare_scope = RequestScope::open(&bare, &options()).unwrap();
        let query = Query::new(&bare_scope, plan_for(QueryKind::Entries, 1)).unwrap();
        let cancel = AtomicBool::new(false);
        assert!(matches!(
            query.run(&cancel, &mut |_row| Ok(())),
            Err(QueryError::NotArchive)
        ));
    }

    #[test]
    fn h03_multi_image_rows_are_complete_regardless_of_completion_order() {
        let dir = TempDir::new("h03-images");
        let archive = zip(&[
            stored_entry("classes.dex", FIELD_DEX),
            stored_entry("classes2.dex", POLY_DEX),
        ]);
        let path = dir.file("two.apk", &archive);
        let scope = RequestScope::open(&path, &options()).unwrap();

        let serial = collect(&scope, plan_for(QueryKind::Methods, 1));
        let parallel = collect(&scope, plan_for(QueryKind::Methods, 4));
        let identity = |rows: &[Row]| {
            let mut keys: Vec<(usize, u32, u32)> = rows
                .iter()
                .map(|row| match row {
                    Row::Method(row) => (row.image.0, row.class_def.0, row.method.0),
                    _ => panic!("methods returned a foreign row"),
                })
                .collect();
            keys.sort();
            keys
        };
        assert_eq!(identity(&serial), identity(&parallel));
        assert_eq!(serial.len(), 4 + 3);
        assert_eq!(identity(&serial).len(), 7);

        let query = Query::new(&scope, plan_for(QueryKind::Methods, 2)).unwrap();
        assert_eq!(query.images().len(), 2);
        assert_eq!(query.images()[0].label, "classes.dex");
        assert_eq!(query.images()[1].label, "classes2.dex");
        assert_eq!(query.images()[0].index.0, 0);
        assert_eq!(query.images()[1].index.0, 1);
    }

    #[test]
    fn h03_explicit_row_bound_and_cancellation_stop_without_a_successful_prefix() {
        let dir = TempDir::new("h03-bounds");
        let path = dir.file("field.dex", FIELD_DEX);
        let scope = RequestScope::open(&path, &options()).unwrap();

        let mut plan = plan_for(QueryKind::Strings, 1);
        plan.limits = QueryLimits {
            max_rows: Some(1),
            max_batch_rows: Some(1),
        };
        let query = Query::new(&scope, plan).unwrap();
        let cancel = AtomicBool::new(false);
        let mut delivered = 0u64;
        let result = query.run(&cancel, &mut |_row| {
            delivered += 1;
            Ok(())
        });
        assert!(matches!(
            result,
            Err(QueryError::Budget {
                limit: QueryLimit::Rows,
                maximum: 1,
                ..
            })
        ));
        assert!(delivered <= 2, "a bounded query published {delivered} rows");

        let plan = plan_for(QueryKind::Strings, 1);
        let query = Query::new(&scope, plan).unwrap();
        let cancel = AtomicBool::new(true);
        assert!(matches!(
            query.run(&cancel, &mut |_row| Ok(())),
            Err(QueryError::Cancelled)
        ));

        let mut plan = plan_for(QueryKind::Strings, 1);
        plan.limits = QueryLimits {
            max_rows: None,
            max_batch_rows: Some(1),
        };
        let rows = collect(&scope, plan);
        assert!(rows.len() > 1);
        let mut distinct = rows.clone();
        distinct.sort_by_key(|row| match row {
            Row::String(row) => row.string.0,
            _ => u32::MAX,
        });
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            rows.len(),
            "a batched query duplicated a row"
        );
    }
}

mod h03_xref {
    use std::sync::atomic::AtomicBool;

    use diana::analysis::dex::ClassFilter;
    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::query::{Query, QueryKind, QueryLimits, QueryPlan, Row};
    use diana::analysis::request::RequestScope;
    use diana::analysis::xref::{Matched, Target, Xref, XrefPlan, XrefRow};

    use super::support::TempDir;

    const HIERARCHY_DEX: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/hierarchy/hierarchy.dex");
    const FIELD_DEX: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/annotations/field.dex");

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    fn descriptor(text: &str) -> Vec<u16> {
        diana::analysis::dex::class_descriptor(text)
    }

    fn text(units: &[u16]) -> String {
        String::from_utf16(units).unwrap()
    }

    fn xref(scope: &RequestScope, plan: XrefPlan) -> Vec<XrefRow> {
        let xref = Xref::new(scope, plan).unwrap();
        let cancel = AtomicBool::new(false);
        let mut rows = Vec::new();
        xref.run(&cancel, &mut |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
        rows
    }

    fn target(kind: &str) -> Target {
        match kind {
            "type" => Target::Type(descriptor("fixtures.Repo")),
            "method" => Target::Method {
                owner: descriptor("fixtures.Repo"),
                name: "save".encode_utf16().collect(),
                prototype: "()V".encode_utf16().collect(),
            },
            other => panic!("unknown target kind {other}"),
        }
    }

    #[test]
    fn h03_xref_reports_every_code_site_with_its_original_instruction() {
        let dir = TempDir::new("h03-xref");
        let path = dir.file("hierarchy.dex", HIERARCHY_DEX);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let rows = xref(
            &scope,
            XrefPlan {
                target: target("type"),
                class: None,
                limits: QueryLimits::default(),
                jobs: 1,
            },
        );
        let sites: Vec<(String, u32, String)> = rows
            .iter()
            .map(|row| {
                assert_eq!(text(&row.method_name), "sites");
                assert_eq!(text(&row.method_prototype), "(Ljava/lang/Object;)V");
                assert_eq!(row.matched, Matched::Type(descriptor("fixtures.Repo")));
                (row.use_.to_owned(), row.code_unit, text(&row.method_name))
            })
            .collect();
        let mut uses: Vec<String> = sites.iter().map(|site| site.0.clone()).collect();
        uses.sort();
        assert_eq!(uses, ["check-cast", "const-class", "instance-of"]);
        let mut offsets: Vec<u32> = sites.iter().map(|site| site.1).collect();
        offsets.sort_unstable();
        offsets.dedup();
        assert_eq!(offsets.len(), sites.len(), "two sites share one offset");
        assert!(rows.iter().all(|row| row.class_def.0 == 4));

        let rows = xref(
            &scope,
            XrefPlan {
                target: target("method"),
                class: None,
                limits: QueryLimits::default(),
                jobs: 1,
            },
        );
        // Two members call the interface method twice each: every site is a row.
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|row| row.use_ == "invoke-interface"));
        let mut offsets: Vec<(u32, u32)> = rows
            .iter()
            .map(|row| (row.method.0, row.code_unit))
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        assert_eq!(offsets.len(), rows.len());
        assert_eq!(
            rows[0].matched,
            Matched::Method {
                owner: descriptor("fixtures.Repo"),
                name: "save".encode_utf16().collect(),
                prototype: "()V".encode_utf16().collect(),
            }
        );

        let field_path = dir.file("field.dex", FIELD_DEX);
        let field_scope = RequestScope::open(&field_path, &options()).unwrap();
        let rows = xref(
            &field_scope,
            XrefPlan {
                target: Target::Field {
                    owner: descriptor("fixtures.Field"),
                    name: "n".encode_utf16().collect(),
                    field_type: "I".encode_utf16().collect(),
                },
                class: None,
                limits: QueryLimits::default(),
                jobs: 1,
            },
        );
        assert_eq!(rows.len(), 1, "one field read site");
        assert!(rows[0].use_.starts_with("iget"), "{}", rows[0].use_);
        assert_eq!(text(&rows[0].method_name), "read");
    }

    #[test]
    fn h03_xref_class_filter_excludes_other_definitions_and_keeps_empty_results() {
        let dir = TempDir::new("h03-xref-filter");
        let path = dir.file("hierarchy.dex", HIERARCHY_DEX);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let plan = |class: Option<Vec<u16>>| XrefPlan {
            target: Target::Type(descriptor("fixtures.Repo")),
            class,
            limits: QueryLimits::default(),
            jobs: 2,
        };
        assert_eq!(
            xref(&scope, plan(Some(descriptor("fixtures.Use")))).len(),
            3
        );
        assert!(xref(&scope, plan(Some(descriptor("fixtures.MemoryRepo")))).is_empty());
        assert!(
            xref(
                &scope,
                XrefPlan {
                    target: Target::Type(descriptor("fixtures.Absent")),
                    class: None,
                    limits: QueryLimits::default(),
                    jobs: 1,
                }
            )
            .is_empty()
        );
    }

    #[test]
    fn h03_classes_hierarchy_filters_report_direct_edges_only() {
        let dir = TempDir::new("h03-hierarchy");
        let path = dir.file("hierarchy.dex", HIERARCHY_DEX);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let classes = |relation: ClassFilter| {
            let query = Query::new(
                &scope,
                QueryPlan {
                    kind: QueryKind::Classes,
                    filter: None,
                    class: None,
                    class_filter: relation,
                    limits: QueryLimits::default(),
                    jobs: 1,
                },
            )
            .unwrap();
            let cancel = AtomicBool::new(false);
            let mut rows = Vec::new();
            query
                .run(&cancel, &mut |row| {
                    rows.push(row);
                    Ok(())
                })
                .unwrap();
            let mut descriptors: Vec<String> = rows
                .iter()
                .map(|row| match row {
                    Row::Class(row) => text(&row.descriptor),
                    _ => panic!("classes returned a foreign row"),
                })
                .collect();
            descriptors.sort();
            descriptors
        };

        assert_eq!(
            classes(ClassFilter {
                extends: Some(descriptor("fixtures.MemoryRepo")),
                ..ClassFilter::default()
            }),
            ["Lfixtures/DiskRepo;"]
        );
        assert_eq!(
            classes(ClassFilter {
                implements: Some(descriptor("fixtures.Store")),
                ..ClassFilter::default()
            }),
            ["Lfixtures/MemoryRepo;"]
        );
        assert_eq!(
            classes(ClassFilter {
                subinterface: Some(descriptor("fixtures.Repo")),
                ..ClassFilter::default()
            }),
            ["Lfixtures/Store;"]
        );
        // Only direct edges: nobody lists Repo directly except the sub-interface,
        // so no class implements it transitively.
        assert!(
            classes(ClassFilter {
                implements: Some(descriptor("fixtures.Repo")),
                ..ClassFilter::default()
            })
            .is_empty()
        );
        assert_eq!(
            classes(ClassFilter {
                kind: Some(diana::analysis::dex::ClassKind::Interface),
                ..ClassFilter::default()
            }),
            ["Lfixtures/Repo;", "Lfixtures/Store;"]
        );
        assert_eq!(
            classes(ClassFilter {
                kind: Some(diana::analysis::dex::ClassKind::Class),
                implements: Some(descriptor("fixtures.Store")),
                ..ClassFilter::default()
            }),
            ["Lfixtures/MemoryRepo;"]
        );
    }
}

mod h03_manifest {
    use diana::analysis::axml::ManifestLimits;
    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::manifest::{self, ManifestSelectionError};
    use diana::analysis::request::RequestScope;

    use super::support::{TempDir, axml_manifest, dex, stored_entry, zip};

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    #[test]
    fn h03_manifest_uses_the_last_physical_entry_and_keeps_classified_failures() {
        let dir = TempDir::new("h03-manifest");
        let first = axml_manifest("first.pkg");
        let second = axml_manifest("second.pkg");
        let path = dir.file(
            "dup.apk",
            &zip(&[
                stored_entry("AndroidManifest.xml", &first),
                stored_entry("classes.dex", &dex(0x91)),
                stored_entry("AndroidManifest.xml", &second),
            ]),
        );
        let scope = RequestScope::open(&path, &options()).unwrap();
        let manifest = manifest::read(&scope, ManifestLimits::default()).unwrap();
        assert_eq!(manifest.entry().index().0, 2);
        assert!(manifest.xml().contains("package=\"second.pkg\""));

        let missing = dir.file(
            "no-manifest.apk",
            &zip(&[stored_entry("classes.dex", &dex(0x92))]),
        );
        let missing_scope = RequestScope::open(&missing, &options()).unwrap();
        assert!(matches!(
            manifest::read(&missing_scope, ManifestLimits::default()),
            Err(ManifestSelectionError::Missing)
        ));

        let bare = dir.file("bare.dex", &dex(0x93));
        let bare_scope = RequestScope::open(&bare, &options()).unwrap();
        assert!(matches!(
            manifest::read(&bare_scope, ManifestLimits::default()),
            Err(ManifestSelectionError::NotArchive)
        ));

        // Damaged records no longer fail the request: the text stays balanced XML
        // and the gap is reported as a warning.
        let damaged = axml_manifest("broken.pkg");
        let path = dir.file(
            "damaged.apk",
            &zip(&[stored_entry(
                "AndroidManifest.xml",
                &damaged[..damaged.len() - 3],
            )]),
        );
        let damaged_scope = RequestScope::open(&path, &options()).unwrap();
        let partial = manifest::read(&damaged_scope, ManifestLimits::default()).unwrap();
        assert!(partial.is_partial());
        assert!(partial.xml().contains("<manifest"));
        assert!(partial.xml().trim_end().ends_with("</manifest>"));

        let mut compressor = libdeflater::Compressor::new(libdeflater::CompressionLvl::default());
        let mut compressed = vec![0u8; compressor.deflate_compress_bound(second.len())];
        let written = compressor
            .deflate_compress(&second, &mut compressed)
            .unwrap();
        compressed.truncate(written);
        let path = dir.file(
            "deflated.apk",
            &zip(&[super::support::ZipEntry {
                method: 8,
                compressed: Some(&compressed),
                ..stored_entry("AndroidManifest.xml", &second)
            }]),
        );
        let deflated = RequestScope::open(&path, &options()).unwrap();
        let manifest = manifest::read(&deflated, ManifestLimits::default()).unwrap();
        assert!(manifest.xml().contains("package=\"second.pkg\""));
    }
}

mod h04 {
    use diana::analysis::images::{
        ImageSet, resolve_class, resolve_field, resolve_method, resolve_string,
    };
    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::request::RequestScope;
    use diana::analysis::selector;
    use diana::analysis::selector::SelectorError;

    use super::support::{TempDir, stored_entry, zip};

    const POLY: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/dynamic/poly.dex");
    const FIELD: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/annotations/field.dex");

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    fn control() -> diana_decomp::RunContext<'static> {
        diana_decomp::RunContext::new(diana_decomp::Limits::default(), &|| false)
    }

    #[test]
    fn h04_canonical_locators_validate_identity_and_reject_a_descriptor_mismatch() {
        let dir = TempDir::new("h04-at");
        let path = dir.file("poly.dex", POLY);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let images = ImageSet::open(&scope).unwrap();
        assert_eq!(images.images().len(), 1);
        assert_eq!(images.images()[0].logical, 0);
        assert_eq!(images.images()[0].header_offset.0, 0);
        assert!(images.images()[0].entry.is_none());

        let mut control = control();
        let selector = selector::class("e0:l0:c0::Lfixtures/Poly;").unwrap();
        let selection = resolve_class(&images, &selector, &mut control).unwrap();
        assert_eq!(selection.class_def.0, 0);
        assert_eq!(
            String::from_utf16(&selection.descriptor).unwrap(),
            "Lfixtures/Poly;"
        );

        // Same index, different descriptor: the locator is stale.
        let wrong = selector::class("e0:l0:c0::Lfixtures/Other;").unwrap();
        assert!(matches!(
            resolve_class(&images, &wrong, &mut control),
            Err(SelectorError::InvalidIndex)
        ));
        // Out-of-range class_def index.
        let out_of_range = selector::class("e0:l0:c9::Lfixtures/Poly;").unwrap();
        assert!(matches!(
            resolve_class(&images, &out_of_range, &mut control),
            Err(SelectorError::InvalidIndex)
        ));
        // Wrong physical entry / logical ordinal.
        assert!(matches!(
            resolve_class(
                &images,
                &selector::class("e9:l0:c0::Lfixtures/Poly;").unwrap(),
                &mut control
            ),
            Err(SelectorError::NotFound)
        ));
        assert!(matches!(
            resolve_class(
                &images,
                &selector::class("e0:l1:c0::Lfixtures/Poly;").unwrap(),
                &mut control
            ),
            Err(SelectorError::NotFound)
        ));
        // A short dotted name resolves to the same definition.
        let short = selector::class("fixtures.Poly").unwrap();
        let resolved = resolve_class(&images, &short, &mut control).unwrap();
        assert_eq!(resolved.class_def.0, 0);
        assert_eq!(
            selector::class("e0:l0:cZ::Lfixtures/Poly;")
                .unwrap_err()
                .code(),
            1
        );
    }

    #[test]
    fn h04_member_locators_check_names_prototypes_and_field_types() {
        let dir = TempDir::new("h04-members");
        let poly = dir.file("poly.dex", POLY);
        let scope = RequestScope::open(&poly, &options()).unwrap();
        let images = ImageSet::open(&scope).unwrap();
        let mut control = control();
        let method = selector::method(
            "e0:l0:c0:m1::Lfixtures/Poly;->call(Ljava/lang/invoke/MethodHandle;I)I",
        )
        .unwrap();
        let selected = resolve_method(&images, &method, &mut control).unwrap();
        assert_eq!(selected.method.unwrap().0, 1);
        assert_eq!(String::from_utf16(&selected.name).unwrap(), "call");

        let wrong_name = selector::method(
            "e0:l0:c0:m1::Lfixtures/Poly;->other(Ljava/lang/invoke/MethodHandle;I)I",
        )
        .unwrap();
        assert!(matches!(
            resolve_method(&images, &wrong_name, &mut control),
            Err(SelectorError::InvalidIndex)
        ));
        let wrong_proto = selector::method("e0:l0:c0:m1::Lfixtures/Poly;->call()I").unwrap();
        assert!(matches!(
            resolve_method(&images, &wrong_proto, &mut control),
            Err(SelectorError::InvalidIndex)
        ));

        let field_path = dir.file("field.dex", FIELD);
        let field_scope = RequestScope::open(&field_path, &options()).unwrap();
        let field_images = ImageSet::open(&field_scope).unwrap();
        let field = selector::field("e0:l0:c0:f0::Lfixtures/Field;->n:I").unwrap();
        let selected = resolve_field(&field_images, &field, &mut control).unwrap();
        assert_eq!(String::from_utf16(&selected.name).unwrap(), "n");
        let wrong_type = selector::field("e0:l0:c0:f0::Lfixtures/Field;->n:J").unwrap();
        assert!(matches!(
            resolve_field(&field_images, &wrong_type, &mut control),
            Err(SelectorError::InvalidIndex)
        ));

        let string = selector::string("e0:l0:s0").unwrap();
        let resolved = resolve_string(&field_images, &string, &mut control).unwrap();
        assert_eq!(resolved.string.0, 0);
        assert!(!resolved.units.is_empty());
        let missing = selector::string("e0:l0:s9999").unwrap();
        assert!(matches!(
            resolve_string(&field_images, &missing, &mut control),
            Err(SelectorError::InvalidIndex)
        ));
    }

    #[test]
    fn h04_duplicate_definitions_are_reported_as_ambiguous() {
        let dir = TempDir::new("h04-ambiguous");
        let path = dir.file(
            "two.apk",
            &zip(&[
                stored_entry("classes.dex", POLY),
                stored_entry("classes2.dex", POLY),
            ]),
        );
        let scope = RequestScope::open(&path, &options()).unwrap();
        let images = ImageSet::open(&scope).unwrap();
        assert_eq!(images.images().len(), 2);
        assert_eq!(images.images()[0].label, "classes.dex");
        assert_eq!(images.images()[1].label, "classes2.dex");
        let mut control = control();
        let short = selector::class("fixtures.Poly").unwrap();
        let error = resolve_class(&images, &short, &mut control).unwrap_err();
        assert!(matches!(error, SelectorError::Ambiguous));
        assert_eq!(error.code(), 4);
        // The canonical at still selects one definition.
        let exact = selector::class("e1:l0:c0::Lfixtures/Poly;").unwrap();
        let selected = resolve_class(&images, &exact, &mut control).unwrap();
        assert_eq!(selected.image.entry.unwrap().0, 1);
    }

    #[test]
    fn h04_locator_syntax_and_index_failures_are_distinguished() {
        for text in ["", "e", "e0", "e0:l", "e0:l0", "e0:l0:"] {
            let error = selector::class(text).unwrap_err();
            assert_eq!(error.code(), 2, "{text:?}");
        }
        assert_eq!(selector::method("Lfixtures/Poly;").unwrap_err().code(), 2);
        assert_eq!(
            selector::method("Lfixtures/Poly;->call")
                .unwrap_err()
                .code(),
            2
        );
        assert_eq!(
            selector::field("Lfixtures/Field;->n").unwrap_err().code(),
            2
        );
        assert_eq!(selector::string("e0:l0:x1").unwrap_err().code(), 2);
    }
}

mod h04_native {
    use std::path::Path;

    use diana::analysis::images::{ImageSet, resolve_class, resolve_method};
    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::request::RequestScope;
    use diana::analysis::selector;

    use super::support::TempDir;

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    fn container() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/diana-decomp/tests/fixtures/container/container.dex");
        std::fs::read(&path).expect("the certified two-member container fixture is readable")
    }

    #[test]
    fn h04_native_logical_members_keep_their_own_identity() {
        let dir = TempDir::new("h04-native");
        let path = dir.file("container.dex", &container());
        let scope = RequestScope::open(&path, &options()).unwrap();
        let images = ImageSet::open(&scope).unwrap();
        assert_eq!(images.images().len(), 2);
        assert_eq!(images.images()[0].logical, 0);
        assert_eq!(images.images()[0].header_offset.0, 0);
        assert_eq!(images.images()[1].logical, 1);
        assert_eq!(images.images()[1].header_offset.0, 0x78);
        // Both members are one physical entry.
        assert!(images.images()[0].entry.is_none());
        assert!(images.images()[1].entry.is_none());

        let mut control = diana_decomp::RunContext::new(diana_decomp::Limits::default(), &|| false);
        for logical in ["l0", "l1"] {
            let text = format!("e0:{logical}:c0::Lfixtures/Case;");
            let selected =
                resolve_class(&images, &selector::class(&text).unwrap(), &mut control).unwrap();
            assert_eq!(selected.class_def.0, 0);
            assert_eq!(selected.image.logical, if logical == "l0" { 0 } else { 1 });
            let method = format!("e0:{logical}:c0:m1::Lfixtures/Case;->f()I");
            let selected =
                resolve_method(&images, &selector::method(&method).unwrap(), &mut control).unwrap();
            assert_eq!(selected.method.unwrap().0, 1);
            assert_eq!(String::from_utf16(&selected.name).unwrap(), "f");
        }
        // A member that the container does not have is not found.
        let missing = selector::class("e0:l2:c0::Lfixtures/Case;").unwrap();
        assert!(matches!(
            resolve_class(&images, &missing, &mut control),
            Err(diana::analysis::selector::SelectorError::NotFound)
        ));
        // The same class index in another logical member is a different object.
        let other = resolve_class(
            &images,
            &selector::class("e0:l1:c0::Lfixtures/Case;").unwrap(),
            &mut control,
        )
        .unwrap();
        assert_eq!(other.image.header_offset.0, 0x78);
    }
}

mod h05 {
    use diana::analysis::decompile;
    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::request::RequestScope;
    use diana::analysis::selector::{self, Selector};
    use diana_decomp::{ClassOutput, MethodOutput};

    use super::support::TempDir;

    const D085: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/dynamic/d085.dex");
    const POLY: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/dynamic/poly.dex");
    const D051: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/annotations/d051.dex");

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    fn decompile_class(bytes: &[u8], name: &str, text: &str) -> (TempDir, decompile::Decompiled) {
        let dir = TempDir::new(name);
        let path = dir.file("input.dex", bytes);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let selector = Selector::Class(selector::class(text).unwrap());
        let mut control = diana_decomp::RunContext::new(diana_decomp::Limits::default(), &|| false);
        let decompiled = decompile::run_with_catalog(&scope, &selector, &mut control).unwrap();
        // The report owns everything; the leased input may be dropped.
        drop(scope);
        (dir, decompiled)
    }

    #[test]
    fn h05_java_members_keep_spans_and_mixed_classes_stay_visible() {
        let (_dir, decompiled) = decompile_class(D051, "h05-java", "Lfixtures/Case;");
        assert!(matches!(
            decompiled.report.class(),
            ClassOutput::Mixed { .. } | ClassOutput::JavaUnit { .. }
        ));
        assert_eq!(decompiled.class_at, "e0:l0:c0::Lfixtures/Case;");
        let members = decompiled.report.class().members();
        assert_eq!(members.len(), 1);
        // External annotation declarations are no longer required; the member
        // stays visible whether the source is readable Java or original facts.
        assert!(matches!(
            members[0],
            MethodOutput::Java { .. } | MethodOutput::Debug { .. }
        ));
        let span = decompiled
            .rendered
            .spans
            .iter()
            .find(|span| span.method == members[0].key())
            .unwrap();
        let slice = &decompiled.rendered.source[span.start.0..span.end.0];
        let source = match &members[0] {
            MethodOutput::Java { source, .. } => source.as_str(),
            MethodOutput::Debug { source, .. } => source.source(),
            _ => unreachable!(),
        };
        assert_eq!(slice, source);
        assert!(span.start.0 <= span.end.0);
        assert!(span.end.0 <= decompiled.rendered.source.len());
    }

    #[test]
    fn h05_unsupported_members_keep_their_reason_and_other_members() {
        // A class request keeps the Java constructor and the invoke-custom raw
        // member; a method request returns only the failing member with its reason.
        let (_dir, class) = decompile_class(D085, "h05-mixed", "Lfixtures/Case;");
        let ClassOutput::Mixed { members, .. } = class.report.class() else {
            panic!("a class with an unsupported member must be Mixed");
        };
        assert_eq!(members.len(), 3);
        let dynamic = members
            .iter()
            .find(|member| member.key().method.0 == 2)
            .unwrap();
        assert!(matches!(dynamic, MethodOutput::Debug { .. }));
        let reason = match dynamic {
            MethodOutput::Debug { reason, .. } => reason,
            _ => unreachable!(),
        };
        assert_eq!(
            class.report.diagnostics()[reason.0].code(),
            diana_decomp::DiagnosticCode::InvokeCustomUnsupported
        );
        assert!(
            members
                .iter()
                .any(|member| matches!(member, MethodOutput::Java { .. }))
        );

        let dir = TempDir::new("h05-method");
        let path = dir.file("input.dex", D085);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let selector = Selector::Method(selector::method("Lfixtures/Case;->aDynamic()I").unwrap());
        let mut control = diana_decomp::RunContext::new(diana_decomp::Limits::default(), &|| false);
        let method = decompile::run_with_catalog(&scope, &selector, &mut control).unwrap();
        assert_eq!(method.method.unwrap().0, 2);
        assert_eq!(method.report.class().members().len(), 1);
        assert!(matches!(
            method.report.class().members()[0],
            MethodOutput::Debug { .. }
        ));
        assert!(
            method
                .method_at
                .as_deref()
                .unwrap()
                .starts_with("e0:l0:c0:m2::Lfixtures/Case;->aDynamic()I")
        );
        let span = &method.rendered.spans[0];
        let slice = &method.rendered.source[span.start.0..span.end.0];
        assert!(slice.contains("invoke-custom"));
        drop(scope);
        drop(dir);
    }

    #[test]
    fn h05_method_selector_requires_an_unambiguous_complete_reference() {
        let dir = TempDir::new("h05-selector");
        let poly = dir.file("poly.dex", POLY);
        let scope = RequestScope::open(&poly, &options()).unwrap();
        let mut control = diana_decomp::RunContext::new(diana_decomp::Limits::default(), &|| false);
        // A bare name without the prototype is a syntax error; this is a method
        // reference, not a guess.
        let error = selector::method("Lfixtures/Poly;->call").unwrap_err();
        assert_eq!(error.code(), 2);
        let selector = Selector::Method(
            selector::method("Lfixtures/Poly;->call(Ljava/lang/invoke/MethodHandle;I)I").unwrap(),
        );
        let selected = decompile::run_with_catalog(&scope, &selector, &mut control).unwrap();
        assert_eq!(selected.method.unwrap().0, 1);
        assert_eq!(
            selected.method_at.as_deref().unwrap(),
            "e0:l0:c0:m1::Lfixtures/Poly;->call(Ljava/lang/invoke/MethodHandle;I)I"
        );
        assert!(matches!(
            selected.report.class().members()[0],
            MethodOutput::Debug { .. }
        ));
    }
}

mod h06 {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use diana::analysis::decompile;
    use diana::analysis::input::{InputError, InputLimit, InputLimits, InputOptions};
    use diana::analysis::limits::{RequestLimits, enforce_output};
    use diana::analysis::request::RequestScope;
    use diana::analysis::selector::{self, Selector};
    use diana_decomp::{FailureKind, Limits};

    use super::support::TempDir;

    const D085: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/dynamic/d085.dex");

    fn base() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    #[test]
    fn h06_input_and_output_bounds_are_explicit_and_never_truncate() {
        let dir = TempDir::new("h06-bounds");
        let path = dir.file("input.dex", D085);
        let limits = RequestLimits {
            max_input_bytes: Some(8),
            ..RequestLimits::default()
        };
        let error = RequestScope::open(
            &path,
            &InputOptions {
                limits: limits.input(),
                allowed_root: None,
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            InputError::BudgetExceeded {
                limit: InputLimit::InputBytes,
                ..
            }
        ));

        // The delivered payload is checked on its actual bytes.
        assert!(enforce_output(b"0123456789", Some(10), "max_report_output_bytes").is_ok());
        let error =
            enforce_output(b"01234567890", Some(10), "max_report_output_bytes").unwrap_err();
        assert_eq!(error.maximum, 10);
        assert_eq!(error.actual, 11);
        assert!(enforce_output(b"any", None, "max_report_output_bytes").is_ok());
    }

    #[test]
    fn h06_cancellation_stops_the_request_without_a_partial_report() {
        let dir = TempDir::new("h06-cancel");
        let path = dir.file("input.dex", D085);
        let scope = RequestScope::open(&path, &base()).unwrap();
        let selector = Selector::Class(selector::class("Lfixtures/Case;").unwrap());

        let cancelled = AtomicBool::new(true);
        let stop = || cancelled.load(Ordering::Relaxed);
        let mut control = diana_decomp::RunContext::new(Limits::default(), &stop);
        let error = decompile::run(&scope, &selector, &mut control).unwrap_err();
        assert_eq!(error.code(), 1);

        // Cancelling after some real work also terminates instead of finishing.
        let polls = AtomicU64::new(0);
        let stop = || {
            let seen = polls.fetch_add(1, Ordering::Relaxed) + 1;
            seen > 200
        };
        let mut control = diana_decomp::RunContext::new(Limits::default(), &stop);
        let result = decompile::run(&scope, &selector, &mut control);
        if let Err(error) = result {
            assert_eq!(error.code(), 1);
            assert!(polls.load(Ordering::Relaxed) > 200);
        } else {
            panic!("cancellation after 200 checks still delivered a report");
        }
    }

    #[test]
    fn h06_method_and_report_output_limits_stay_separate() {
        let dir = TempDir::new("h06-output");
        let path = dir.file("input.dex", D085);
        let scope = RequestScope::open(&path, &base()).unwrap();
        let selector = Selector::Class(selector::class("Lfixtures/Case;").unwrap());

        // A member-output limit turns the affected member into a classified
        // failure; the request itself still yields a report for the other members.
        let stop = || false;
        let mut control = diana_decomp::RunContext::new(
            Limits {
                max_method_output_bytes: Some(16),
                ..Limits::default()
            },
            &stop,
        );
        let decompiled = decompile::run(&scope, &selector, &mut control).unwrap();
        assert!(
            decompiled
                .report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.cause.kind() == FailureKind::BudgetExceeded)
        );

        // A report-output limit stops rendering entirely instead of truncating it.
        let mut control = diana_decomp::RunContext::new(
            Limits {
                max_report_output_bytes: Some(16),
                ..Limits::default()
            },
            &stop,
        );
        let error = decompile::run(&scope, &selector, &mut control).unwrap_err();
        assert_eq!(error.code(), 1);
        assert!(error.to_string().contains("max_report_output_bytes"));
    }
}

mod h09 {
    use std::sync::atomic::AtomicBool;

    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::query::{Query, QueryKind, QueryLimits, QueryPlan, Row};
    use diana::analysis::request::RequestScope;
    use diana::analysis::xref::{Matched, Target, Xref, XrefPlan};

    use super::support::{TempDir, stored_entry, zip};

    const HIERARCHY: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/hierarchy/hierarchy.dex");
    const D085: &[u8] =
        include_bytes!("../../../crates/diana-decomp/tests/fixtures/dynamic/d085.dex");

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    fn descriptor(text: &str) -> Vec<u16> {
        diana::analysis::dex::class_descriptor(text)
    }

    #[test]
    fn h09_sites_keep_their_member_offset_and_logical_identity_across_inputs() {
        let dir = TempDir::new("h09-sites");
        let path = dir.file(
            "two.apk",
            &zip(&[
                stored_entry("classes.dex", HIERARCHY),
                stored_entry("classes2.dex", HIERARCHY),
            ]),
        );
        let scope = RequestScope::open(&path, &options()).unwrap();
        let xref = Xref::new(
            &scope,
            XrefPlan {
                target: Target::Method {
                    owner: descriptor("fixtures.Repo"),
                    name: "save".encode_utf16().collect(),
                    prototype: "()V".encode_utf16().collect(),
                },
                class: None,
                limits: QueryLimits::default(),
                jobs: 2,
            },
        )
        .unwrap();
        let cancel = AtomicBool::new(false);
        let mut rows = Vec::new();
        xref.run(&cancel, &mut |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
        // Four sites per image, eight in total; equal indices in the two entries
        // are different objects and are never merged.
        assert_eq!(rows.len(), 8);
        let mut keys: Vec<(usize, u32, u32)> = rows
            .iter()
            .map(|row| (row.image.0, row.method.0, row.code_unit))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), rows.len());
        assert!(
            rows.iter()
                .all(|row| matches!(row.matched, Matched::Method { .. }))
        );
        let mut per_image = [0usize; 2];
        for row in &rows {
            per_image[row.image.0] += 1;
        }
        assert_eq!(per_image, [4, 4]);
    }

    #[test]
    fn h09_string_targets_use_exact_utf16_equality() {
        let dir = TempDir::new("h09-string");
        let path = dir.file("d085.dex", D085);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let literal: Vec<u16> = vec![0, 55296, 42, 47, 92, 117, 48, 48, 48, 97];
        let xref = Xref::new(
            &scope,
            XrefPlan {
                target: Target::String(literal.clone()),
                class: None,
                limits: QueryLimits::default(),
                jobs: 1,
            },
        )
        .unwrap();
        let cancel = AtomicBool::new(false);
        let mut rows = Vec::new();
        xref.run(&cancel, &mut |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].use_, "const-string");
        assert_eq!(rows[0].matched, Matched::String(literal));

        // A shorter prefix is not the same value.
        let prefix = Target::String(vec![0, 55296, 42]);
        let xref = Xref::new(
            &scope,
            XrefPlan {
                target: prefix,
                class: None,
                limits: QueryLimits::default(),
                jobs: 1,
            },
        )
        .unwrap();
        let mut rows = Vec::new();
        xref.run(&cancel, &mut |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn h09_defined_members_keep_abstract_and_constructor_declarations() {
        let dir = TempDir::new("h09-members");
        let path = dir.file("hierarchy.dex", HIERARCHY);
        let scope = RequestScope::open(&path, &options()).unwrap();
        let query = Query::new(
            &scope,
            QueryPlan {
                kind: QueryKind::Methods,
                filter: None,
                class: Some(descriptor("fixtures.Repo")),
                class_filter: Default::default(),
                limits: QueryLimits::default(),
                jobs: 1,
            },
        )
        .unwrap();
        let cancel = AtomicBool::new(false);
        let mut rows = Vec::new();
        query
            .run(&cancel, &mut |row| {
                rows.push(row);
                Ok(())
            })
            .unwrap();
        assert_eq!(rows.len(), 1);
        // An interface method is a real definition, not an external method_id.
        match &rows[0] {
            Row::Method(row) => {
                assert_eq!(String::from_utf16(&row.name).unwrap(), "save");
                assert_eq!(row.access_flags & 0x400, 0x400);
            }
            _ => panic!("methods returned a foreign row"),
        }

        // An unused string-pool entry stays a string row and is never inferred as
        // a code reference.
        let query = Query::new(
            &scope,
            QueryPlan {
                kind: QueryKind::Strings,
                filter: None,
                class: None,
                class_filter: Default::default(),
                limits: QueryLimits::default(),
                jobs: 1,
            },
        )
        .unwrap();
        let mut strings = Vec::new();
        query
            .run(&cancel, &mut |row| {
                if let Row::String(row) = row {
                    strings.push(row);
                }
                Ok(())
            })
            .unwrap();
        assert!(strings.iter().any(|row| {
            String::from_utf16(&row.units).ok().as_deref() == Some("Lfixtures/Repo;")
        }));
    }
}

mod h10 {
    use std::path::Path;

    use diana::analysis::axml::ManifestLimits;
    use diana::analysis::extract::{self, ExtractError, Request};
    use diana::analysis::input::{InputLimits, InputOptions};
    use diana::analysis::request::RequestScope;

    use super::support::{TempDir, axml_manifest, dex, stored_entry, zip};

    fn options() -> InputOptions {
        InputOptions {
            limits: InputLimits::default(),
            allowed_root: None,
        }
    }

    fn request(name: &str, index: Option<usize>, raw: bool) -> Request<'_> {
        Request {
            name,
            index,
            raw,
            limits: ManifestLimits::default(),
        }
    }

    #[test]
    fn h10_selection_is_exact_and_duplicates_are_ambiguous() {
        let dir = TempDir::new("h10-select");
        let path = dir.file(
            "dup.apk",
            &zip(&[
                stored_entry("classes.dex", &dex(0x31)),
                stored_entry("dup.txt", b"one"),
                stored_entry("dup.txt", b"two"),
            ]),
        );
        let scope = RequestScope::open(&path, &options()).unwrap();
        let error = extract::prepare(&scope, &request("dup.txt", None, false)).unwrap_err();
        assert!(matches!(error, ExtractError::Ambiguous { .. }));
        assert_eq!(error.code(), 4);
        let extracted = extract::prepare(&scope, &request("dup.txt", Some(2), false)).unwrap();
        assert_eq!(extracted.bytes, b"two");
        assert!(matches!(
            extract::prepare(&scope, &request("dup.txt", Some(0), false)).unwrap_err(),
            ExtractError::IndexMismatch { .. }
        ));
        assert!(matches!(
            extract::prepare(&scope, &request("absent", None, false)).unwrap_err(),
            ExtractError::Missing(_)
        ));
    }

    #[test]
    fn h10_binary_xml_is_decoded_by_default_and_raw_keeps_the_payload() {
        let dir = TempDir::new("h10-xml");
        let manifest = axml_manifest("pkg");
        let path = dir.file(
            "app.apk",
            &zip(&[
                stored_entry("AndroidManifest.xml", &manifest),
                stored_entry("classes.dex", &dex(0x32)),
            ]),
        );
        let scope = RequestScope::open(&path, &options()).unwrap();
        let decoded =
            extract::prepare(&scope, &request("AndroidManifest.xml", None, false)).unwrap();
        assert!(decoded.decoded);
        let text = String::from_utf8(decoded.bytes.clone()).unwrap();
        assert!(text.contains("<manifest"), "{text}");
        assert!(text.contains("package=\"pkg\""));
        let raw = extract::prepare(&scope, &request("AndroidManifest.xml", None, true)).unwrap();
        assert!(!raw.decoded);
        assert_eq!(raw.bytes, manifest);
        // A non-binary-XML entry is always byte for byte.
        let dex_entry = extract::prepare(&scope, &request("classes.dex", None, false)).unwrap();
        assert!(!dex_entry.decoded);
        assert_eq!(dex_entry.bytes, dex(0x32));
    }

    #[test]
    fn h10_publish_is_atomic_and_rejects_unsafe_targets() {
        let dir = TempDir::new("h10-publish");
        let apk = dir.file("app.apk", &zip(&[stored_entry("dup.txt", b"payload")]));
        let scope = RequestScope::open(&apk, &options()).unwrap();
        let extracted = extract::prepare(&scope, &request("dup.txt", None, false)).unwrap();

        let target = dir.path().join("out.bin");
        std::fs::write(&target, b"old").unwrap();
        assert!(matches!(
            extract::publish(&extracted, &target, &apk, false).unwrap_err(),
            ExtractError::TargetExists
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        extract::publish(&extracted, &target, &apk, true).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"payload");

        // A directory, a symbolic link and the input itself are unsafe targets.
        assert!(matches!(
            extract::publish(&extracted, dir.path(), &apk, true).unwrap_err(),
            ExtractError::UnsafeTarget
        ));
        assert!(matches!(
            extract::publish(&extracted, &apk, &apk, true).unwrap_err(),
            ExtractError::UnsafeTarget
        ));
        #[cfg(unix)]
        {
            let link = dir.path().join("link.bin");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(matches!(
                extract::publish(&extracted, &link, &apk, true).unwrap_err(),
                ExtractError::UnsafeTarget
            ));
        }
        // No private temporary file survives a completed or rejected run.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".diana-extract-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert!(Path::new(&target).is_file());
    }
}
