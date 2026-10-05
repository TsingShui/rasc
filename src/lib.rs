//! Native APK/DEX analysis shared by the one-shot CLI and headless MCP adapter.
//!
//! The crate is arranged around three top-level modules:
//! - [`cli`] owns argument parsing and command-line dispatch;
//! - [`mcp`] adapts the analysis session to MCP stdio;
//! - [`analysis`] contains archive, DEX, manifest, decompiler, and session logic.
//!
//! This is an implementation seam for the shipped binary, not yet a versioned
//! public Rust interface.

mod analysis;
mod cli;
mod diag;
mod mcp;

// Preserve the existing library seam while keeping its implementation grouped
// with the rest of the analysis engine.
pub use analysis::session;

/// Run the command-line application.
pub fn run() {
    cli::run();
}
