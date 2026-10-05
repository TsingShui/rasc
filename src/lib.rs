//! Native APK/DEX analysis shared by the one-shot CLI and headless MCP adapter.
//!
//! This library is an implementation seam for the shipped binary, not yet a
//! versioned public Rust interface.
mod apk;
mod bytes;
mod cli;
#[doc(hidden)]
pub mod cli_entry;
mod dex;
mod diag;
mod emitter;
mod manifest;
mod mcp;
mod query;
pub mod session;
mod skill;
mod zip;
