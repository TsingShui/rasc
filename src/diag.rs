//! Where `--debug` diagnostics go.
//!
//! Diagnostics belong on stderr - that is the CLI's contract, and the tests rely on stdout
//! carrying the payload and nothing else. A host-driven wasm build has no stderr of its own,
//! so the bytes go back to the host instead of being dropped on the floor: the same split as
//! the clock, and the reason `--debug` works through the JS host.
//!
//! The single entry point is [`diagnose`], a thin wrapper over `eprintln!` so every call site
//! keeps the same formatting.

use std::fmt::Arguments;

/// Writes a diagnostic line, on stderr natively and through the host elsewhere.
#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
pub(crate) fn diagnose(args: Arguments<'_>) {
    eprintln!("{args}");
}

// What a JS host provides for diagnostics.
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    // Writes `len` diagnostic bytes at `ptr` to wherever the host keeps stderr.
    fn rasc_host_write_err(ptr: u32, len: u32) -> u32;
}

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
pub(crate) fn diagnose(args: Arguments<'_>) {
    let mut line = String::new();
    if std::fmt::Write::write_fmt(&mut line, args).is_err() {
        return;
    }
    line.push('\n');
    unsafe { rasc_host_write_err(line.as_ptr() as u32, line.len() as u32) };
}
