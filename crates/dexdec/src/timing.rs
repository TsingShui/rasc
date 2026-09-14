//! `crate::timing::Instant` shim (rasc fork; see PATCHES.md).
//!
//! Upstream times its stages with `crate::timing::Instant::now()`. The
//! `wasm32-unknown-unknown` target has no clock, and `Instant::now()` panics
//! there - which took the whole host-module build (the browser/JS product)
//! down on the first `getclass`, because the host module has no WASI. Every use
//! is instrumentation: the value is passed to `mark(&mut stages, name, t)` and
//! never read for behaviour. On that one target the stub returns a zero
//! duration; everywhere else this is exactly `crate::timing::Instant`.

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
pub(crate) use std::time::Instant;

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Instant;

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
impl Instant {
    pub(crate) fn now() -> Self {
        Self
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}
