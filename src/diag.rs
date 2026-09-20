//! Where `--debug` diagnostics go.
//!
//! Diagnostics belong on stderr - that is the CLI's contract, and the tests rely on stdout
//! carrying the payload and nothing else.

use std::fmt::Arguments;

/// Writes a diagnostic line, on stderr.
pub(crate) fn diagnose(args: Arguments<'_>) {
    eprintln!("{args}");
}
