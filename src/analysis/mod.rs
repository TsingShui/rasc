//! Native APK/DEX analysis implementation.
//!
//! The one-shot CLI and persistent MCP adapter share this module. Its external
//! seam is [`session::AnalysisSession`]; archive, DEX, manifest, and decompiler
//! details remain implementation-local.

pub(crate) mod apk;
pub(crate) mod archive;
pub(crate) mod bytes;
pub(crate) mod dex;
pub(crate) mod emitter;
pub(crate) mod manifest;
pub(crate) mod query;
pub mod session;
