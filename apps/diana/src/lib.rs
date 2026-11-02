//! Diana application library: input ownership and request scope.
//!
//! The library borrows from an owned input snapshot; production code stays safe
//! Rust with no external process or runtime dependency.

pub mod analysis;
pub mod cli;
