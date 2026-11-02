//! Input, archive and root-selection analysis owned by the application.
//!
//! `input` owns the immutable snapshot, `archive` reads ZIP metadata and entry
//! payloads from that snapshot, `apk` selects the root DEX images, and `request`
//! holds one request's lease for its full lifetime.

pub mod apk;
pub mod archive;
pub mod input;
pub mod request;
