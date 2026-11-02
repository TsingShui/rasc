//! CLI inheritance queries use self-made DEX data, not a real APK corpus.
mod support;

use std::fs;
use std::process::{Command, Output};

fn u32_at(data: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize
}

fn put(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn fixture() -> Vec<u8> {
    let mut data = support::const_string_dex(4);
    let defs = u32_at(&data, 0x64);
    // Fixture1 directly extends Fixture0; Fixture2 only extends Fixture1.
    put(&mut data, defs + 32 + 8, 1);
    put(&mut data, defs + 64 + 8, 2);
    while !data.len().is_multiple_of(4) {
        data.push(0);
    }
    let interfaces = data.len() as u32;
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // duplicate must not duplicate output
    put(&mut data, defs + 96 + 12, interfaces);
    // These queries must not visit code/class_data.
    for index in 0..4 {
        put(&mut data, defs + index * 32 + 24, u32::MAX);
    }
    let len = data.len() as u32;
    put(&mut data, 0x20, len);
    data
}

fn run(path: &std::path::Path, kind: &str, target: &str, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args(["findrefs", "--threads", "2"])
        .arg(path)
        .args([kind, target])
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn direct_relations_have_upstream_output_and_accept_both_name_forms() {
    let root = tempfile::tempdir().unwrap();
    let dex = fixture();
    let path = root.path().join("relations.apk");
    fs::write(
        &path,
        support::deflated_zip(&[("classes.dex", &dex), ("classes2.dex", &dex)]),
    )
    .unwrap();
    for (kind, descriptor, verb) in [
        ("extends", "LFixture1;", "extends"),
        ("impl", "LFixture3;", "implements"),
    ] {
        let expected = format!(
            "classes.dex | {descriptor} {verb} LFixture0;\nclasses2.dex | {descriptor} {verb} LFixture0;\n"
        );
        for target in ["Fixture0", "LFixture0;"] {
            let output = run(&path, kind, target, &[]);
            assert!(output.status.success(), "{:?}", output);
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
            assert!(output.stderr.is_empty());
        }
        let output_file = root.path().join("results.txt");
        let output = run(
            &path,
            kind,
            "Fixture0",
            &["-o", output_file.to_str().unwrap()],
        );
        assert!(output.status.success());
        assert_eq!(fs::read(output_file).unwrap(), output.stdout);
        assert!(run(&path, kind, "Missing", &[]).stdout.is_empty());
    }
    let bare = root.path().join("relations.dex");
    fs::write(&bare, dex).unwrap();
    assert!(run(&bare, "extends", "Fixture0", &[]).status.success());
}

#[test]
fn malformed_relations_return_errors_without_panics() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("bad.dex");
    for (kind, field, value) in [("extends", 8, 999), ("impl", 12, u32::MAX)] {
        let mut dex = fixture();
        let defs = u32_at(&dex, 0x64);
        put(&mut dex, defs + field, value);
        fs::write(&path, dex).unwrap();
        let output = run(&path, kind, "Fixture0", &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
    let mut dex = fixture();
    let defs = u32_at(&dex, 0x64);
    let list = u32_at(&dex, defs + 96 + 12);
    put(&mut dex, list, u32::MAX);
    fs::write(&path, dex).unwrap();
    assert_eq!(run(&path, "impl", "Fixture0", &[]).status.code(), Some(1));
    assert!(!run(&path, "extends", "", &[]).status.success());
}

#[test]
fn duplicate_type_descriptors_and_external_parents_are_supported() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("external.dex");
    let mut dex = fixture();
    let defs = u32_at(&dex, 0x64);
    let types = u32_at(&dex, 0x44);
    put(&mut dex, types, 1); // type 0 and type 1 both name Fixture0
    put(&mut dex, defs + 32 + 8, 0);
    // Remove Fixture0's definition: a parent only needs a type_id, not a class_def.
    put(&mut dex, defs, 4);
    fs::write(&path, dex).unwrap();
    let output = run(&path, "extends", "Fixture0", &[]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("LFixture1; extends LFixture0;"));
}

#[test]
fn help_and_embedded_skill_advertise_inheritance_queries() {
    let help = Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args(["findrefs", "--help"])
        .output()
        .unwrap();
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("extends"));
    assert!(help.contains("impl"));
    let skill = Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args(["skill", "--print"])
        .output()
        .unwrap();
    let skill = String::from_utf8(skill.stdout).unwrap();
    assert!(skill.contains("rasc findrefs app.apk extends android.app.Activity"));
    assert!(skill.contains("rasc findrefs app.apk impl android.os.IInterface"));
}
