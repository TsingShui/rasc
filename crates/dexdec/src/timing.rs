//! `crate::timing::Instant` shim.
//!
//! Stage timing goes through `crate::timing::Instant::now()`, which keeps this module so
//! a target can substitute a clock. Every use is instrumentation: the value is passed to
//! `mark(&mut stages, name, t)` and never read for behaviour.
pub(crate) use std::time::Instant;
