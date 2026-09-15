//! Where `--debug` diagnostics go.
//!
//! Diagnostics belong on stderr - that is the CLI's contract, and the tests rely on stdout
//! carrying the payload and nothing else. Under WASI there is no separate stream to lose:
//! the host hands the instance a real stderr, so `eprintln!` is the whole implementation
//! and this module is the one place that says so.

use std::fmt::Arguments;

/// Writes a diagnostic line, on stderr.
pub(crate) fn diagnose(args: Arguments<'_>) {
    eprintln!("{args}");
}
