//! The `--debug` clock.
//!
//! `std::time::Instant::now()` panics on wasm32-unknown-unknown: that target's PAL is the
//! "unsupported" one, because a sandbox with no OS has no clock to read. Under a JS host
//! the host answers the clock instead, so the same timing code runs on every target.
//!
//! Only `now()` and `elapsed()` are needed; everything else in the crate keeps using
//! `std::time::Duration`.

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
pub(crate) use std::time::Instant;

// What a JS host has to provide for the timings.
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    // Host time in milliseconds, from any fixed origin.
    fn rasc_host_now_ms() -> f64;
}

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[derive(Clone, Copy)]
pub(crate) struct Instant(f64);

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
impl Instant {
    pub(crate) fn now() -> Self {
        Instant(unsafe { rasc_host_now_ms() })
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        // `f64::max` ignores a NaN operand, so a host that returns NaN reads as zero
        // rather than panicking inside `from_secs_f64`.
        let elapsed = (unsafe { rasc_host_now_ms() } - self.0).max(0.0);
        std::time::Duration::from_secs_f64(elapsed / 1000.0)
    }
}
