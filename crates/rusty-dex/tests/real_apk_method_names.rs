//! Real-data check: every method in a real multi-DEX APK must carry its name.
//!
//! `EncodedMethod::get_method_name` used to regex the rendered
//! `Lcls;->name(args)ret` text and return `""` whenever the shape did not match.
//! On a real 30-DEX APK that silently swallowed 8,201 of 61,198 method names
//! (members of `-$$Lambda$...` classes, names containing `$`, descriptors
//! containing `-`). The name now comes from `method_ids.name_idx` while parsing.
//!
//! Gated with `#[ignore]` rather than "no fixture, return quietly": the latter is a
//! green test with zero coverage, which is exactly how a bug like this survives a
//! test suite. Run it explicitly:
//!
//! ```text
//! RASC_REAL_APK=/path/to/real.apk \
//!     cargo test -p rusty-dex --features apk -- --ignored --nocapture real_apk
//! ```

#![cfg(feature = "apk")]

use std::collections::BTreeMap;

use rusty_dex::dex::file::DexFile;
use rusty_dex::dex::reader::DexReader;

#[test]
#[ignore = "需要真实多 DEX APK；设 RASC_REAL_APK 后用 --ignored 显式运行"]
fn real_apk_every_method_carries_its_name() {
    let Ok(path) = std::env::var("RASC_REAL_APK") else {
        panic!("set RASC_REAL_APK to a real multi-DEX APK");
    };

    let readers = DexReader::build_from_file(&path).expect("open the APK");
    assert!(
        readers.len() > 1,
        "expected a multi-DEX APK, got {} entry/entries",
        readers.len()
    );

    let mut total = 0usize;
    let mut unnamed = 0usize;
    let mut unnamed_examples: Vec<String> = Vec::new();
    let mut names_with_dollar = 0usize;
    let mut per_dex = BTreeMap::new();

    for reader in readers {
        // Declaration level defers members; materialize each class to get them.
        // Bytecode level would decode code items we never look at.
        let mut dex = DexFile::build_metadata(reader).expect("parse DEX metadata");
        for index in 0..dex.classes.items.len() {
            dex.materialize_class(index).expect("decode class members");
        }

        let mut dex_total = 0usize;
        for name in dex.get_classes_names() {
            let Some(class) = dex.get_class_def(name) else {
                continue;
            };
            // CheapTrick's count came from the same accessor: direct then virtual,
            // each in `class_data` order, which is `method_ids` ascending.
            for method in class.get_methods() {
                dex_total += 1;
                let method_name = method.get_method_name();
                if method_name.is_empty() {
                    unnamed += 1;
                    if unnamed_examples.len() < 10 {
                        unnamed_examples.push(format!("{name}->{}", method.proto));
                    }
                } else if method_name.contains('$') {
                    names_with_dollar += 1;
                }
            }
        }
        per_dex.insert(name_of(&dex), dex_total);
        total += dex_total;
    }

    println!("dexes: {}", per_dex.len());
    for (dex, count) in &per_dex {
        println!("  {dex}: {count} methods");
    }
    println!("methods: {total}");
    println!("unnamed: {unnamed}");
    println!("names containing '$': {names_with_dollar}");

    assert_eq!(
        unnamed,
        0,
        "methods without a name: {unnamed_examples:#?}"
    );
    assert!(
        names_with_dollar > 0,
        "no '$'-bearing name found; the regression this test guards is not being exercised"
    );
    assert!(total > 50_000, "unexpectedly sparse method table: {total}");
}

/// The DEX's own name, for the per-entry report. `DexFile` keeps no name, so the
/// checksum stand-in is the class count times the first class: enough to tell the
/// entries apart in a report without re-reading the archive.
fn name_of(dex: &DexFile) -> String {
    let first = dex.get_classes_names().first().map(|n| n.to_string());
    match first {
        Some(name) => format!("{} classes, first {name}", dex.classes.items.len()),
        None => format!("{} classes", dex.classes.items.len()),
    }
}
