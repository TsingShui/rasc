//! Input, archive and root-selection analysis owned by the application.
//!
//! `input` owns the immutable snapshot, `archive` reads ZIP metadata and entry
//! payloads from that snapshot, and `apk` selects the root DEX images.

pub mod apk;
pub mod archive;
pub mod input;
