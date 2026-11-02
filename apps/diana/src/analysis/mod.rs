//! Input, archive and root-selection analysis owned by the application.
//!
//! `input` owns the immutable snapshot, `archive` reads ZIP metadata and entry
//! payloads from that snapshot, `apk` selects the root DEX images, `dex` reads
//! original table facts, `query` streams rows in completion order, `xref` scans original reference sites, and `request`
//! holds one request's lease for its full lifetime.

pub mod apk;
pub mod archive;
pub mod at;
mod attr_names;
pub mod axml;
pub mod decompile;
pub mod dex;
pub mod images;
pub mod input;
pub mod limits;
pub mod manifest;
pub mod query;
pub mod request;
mod runner;
pub mod selector;
pub mod xref;
