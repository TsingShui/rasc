//! `crate::timing::Instant` shim (rasc fork; see PATCHES.md).
//!
//! Upstream times its stages with `crate::timing::Instant::now()`, and keeps this module so
//! a target can substitute a clock. Every use is instrumentation: the value is passed to
//! `mark(&mut stages, name, t)` and never read for behaviour. Neither target rasc builds for needs a
//! substitute today - native and WASI both have a real clock - which is why the stub this
//! once carried for `wasm32-unknown-unknown` went with that target.
pub(crate) use std::time::Instant;
