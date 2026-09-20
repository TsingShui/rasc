//! `crate::timing::Instant` shim (rasc fork; see PATCHES.md).
//!
//! Upstream times its stages with `crate::timing::Instant::now()`, and keeps this module so
//! a target can substitute a clock. Every use is instrumentation: the value is passed to
//! `mark(&mut stages, name, t)` and never read for behaviour.
pub(crate) use std::time::Instant;
