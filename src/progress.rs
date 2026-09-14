//! How far a walk over the archive's entries has got.
//!
//! The entry walk is the long part of every command, and it is *per entry*: an APK with
//! ten DEX files is ten steps, and each one is real work whose completion the host can
//! see. A browser host uses this to show a fraction instead of an indeterminate bar, and
//! the rule about progress is that a fraction has to be one: this reports completed
//! entries against a total the walk knew before it started.
//!
//! Only the host build sends it. The native and WASI walks run entries in parallel, where
//! a sequence of callbacks would say which worker finished rather than how far the walk
//! has got, and neither of them has a host to report to anyway.

/// Reports that `done` of `total` entries have been walked.
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
pub(crate) fn report(done: usize, total: usize) {
    // Declared in the crate root beside the other host imports.
    unsafe { crate::rasc_host_progress(done as u32, total as u32) };
}

/// Reports nothing: this build has no host to report to and no ordered walk to report.
#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
pub(crate) fn report(_done: usize, _total: usize) {}
