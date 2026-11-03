pub(crate) mod branch;
pub(crate) mod constructor_chain;
pub(crate) mod control;
pub(crate) mod diagnostics;
pub(crate) mod member_emission;
pub(crate) mod request;

/// Opens a compute-stage span when the `perf` feature is enabled. The engine
/// never installs a global reporter; the application owns reporting/flushing.
#[cfg(feature = "perf")]
pub(crate) fn phase_span(name: &'static str) -> Option<fastrace::Span> {
    Some(fastrace::Span::enter_with_local_parent(name))
}

/// No-op span when performance instrumentation is compiled out.
#[cfg(not(feature = "perf"))]
pub(crate) fn phase_span(_name: &'static str) -> Option<()> {
    None
}
