//! The flag matrix, which nothing checked before this file existed.
//!
//! Clap's conflicts are declarative lists, so a flag can be missing from one - and one was:
//! `strings --xrefs --offset 5` was accepted and the offset silently ignored, producing a census
//! page that pretended to be page zero. A flag that exists and does nothing is worse than a flag
//! that does not exist, so the matrix is asserted here: every combination that must be refused is,
//! and the combinations that must work are not refused.
//!
//! The binary is the oracle rather than the parser struct, because the parser is not public - and
//! because what a host sees is the process, not the struct.

use std::process::Command;

fn run(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args(args)
        // A path that cannot be read: clap validates before the archive is opened, so a parse
        // failure is distinguishable from a later failure by its exit code and its message.
        .arg("/nonexistent/flag-matrix.apk")
        .output()
        .expect("the binary runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn combinations_that_contradict_each_other_are_refused() {
    for args in [
        // A page of a census is not a thing: the census has no order to page.
        vec!["strings", "--json", "--xrefs", "--offset", "5"],
        vec!["strings", "--json", "--count", "--offset", "5"],
        vec!["strings", "--json", "--count", "--filter", "e"],
        vec!["strings", "--json", "--count", "--limit", "3"],
        vec!["strings", "--json", "--xrefs", "--filter", "e"],
        vec!["strings", "--json", "--xrefs", "--limit", "3"],
    ] {
        let (code, stderr) = run(&args);
        assert_eq!(code, 2, "{args:?} should be refused by the parser, got exit {code}");
        assert!(
            stderr.contains("cannot be used with"),
            "{args:?} should say which flags contradict, got: {stderr}",
        );
    }
}

#[test]
fn combinations_that_make_sense_are_not_refused() {
    // The other half of the matrix. Without it a conflict declared too broadly would pass the
    // test above and break a host that does ask for a page.
    for args in [
        vec!["strings", "--json", "--offset", "5", "--filter", "http", "--limit", "2"],
        vec!["strings", "--json", "--offset", "5"],
        vec!["strings", "--json", "--filter", "http", "--limit", "2"],
        vec!["strings", "--json", "--xrefs"],
        vec!["strings", "--json", "--count"],
    ] {
        let (code, stderr) = run(&args);
        assert_ne!(code, 2, "{args:?} is a combination a host may ask for, but: {stderr}");
        assert!(!stderr.contains("cannot be used with"), "{args:?} was refused: {stderr}");
    }
}
