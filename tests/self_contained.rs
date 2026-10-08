//! The shipped binary must not embed a Python or JVM runtime.
//!
//! Optional JAR/AAR support may invoke a system-installed d8, but rasc itself stays a
//! native Rust binary and APK/DEX analysis remains self-contained. The dependency
//! list must therefore stay free of Python/JVM bindings and build scripts.

use std::fs;
use std::path::Path;

/// Crates that would pull a Python or JVM runtime into the binary.
const FORBIDDEN_DEPENDENCIES: [&str; 6] =
    ["pyo3", "cpython", "j4rs", "jni", "java-locator", "jni-sys"];

#[test]
fn dependencies_carry_no_python_or_jvm_binding() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read Cargo.toml");
    for dependency in FORBIDDEN_DEPENDENCIES {
        assert!(
            !manifest.contains(dependency),
            "Cargo.toml mentions {dependency}; the release binary must not embed a Python or JVM runtime"
        );
    }
    assert!(
        !manifest.contains("build ="),
        "Cargo.toml declares a build script; production code must stay Rust-only"
    );
}
