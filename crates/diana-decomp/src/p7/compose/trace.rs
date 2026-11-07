//! Per-pass diagnostics for development.
//!
//! A pass reports the counters that changed while it ran, so a reader can see
//! which display choice acted on a method and how much it removed. The report is
//! compiled in only with the `trace-passes` feature: an ordinary build computes
//! no snapshot and writes nothing to stderr. It states what a pass changed, not
//! whether the result is correct.

/// Whether the `trace-passes` feature is compiled in.
pub const ENABLED: bool = cfg!(feature = "trace-passes");

/// Counters that the display passes change, taken before and after one pass.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    fallthrough: usize,
    labels: usize,
    transparent: usize,
    natural: usize,
    omitted_literals: usize,
    omitted_returns: usize,
    omitted_checks: usize,
    inline_checks: usize,
    composed: usize,
    allocations: usize,
    array_literals: usize,
    site_annotations: usize,
}

/// Counters of the current plan. Only called when the feature is enabled.
pub fn snapshot(code: &super::super::MethodCode) -> Snapshot {
    let emission = &code.emission;
    let count = |values: &[bool]| values.iter().filter(|value| **value).count();
    Snapshot {
        // A fallthrough never prints the label it reaches.
        fallthrough: emission
            .transfers
            .iter()
            .filter(|transfer| {
                matches!(
                    transfer.spelling,
                    super::super::TransferSpelling::Fallthrough
                )
            })
            .count(),
        labels: emission
            .blocks
            .iter()
            .filter(|block| block.show_label)
            .count()
            + emission
                .controls
                .iter()
                .filter(|control| control.label_required)
                .count(),
        transparent: emission
            .controls
            .iter()
            .filter(|control| control.transparent)
            .count(),
        natural: count(&emission.natural_stores),
        omitted_literals: count(&emission.omitted_literals),
        omitted_returns: count(&emission.omitted_returns),
        omitted_checks: emission.omitted_checks.iter().flatten().count(),
        inline_checks: emission.inline_checks.len(),
        composed: count(&emission.composed_operations),
        allocations: count(&emission.allocation_declarations),
        array_literals: emission.array_literals.iter().flatten().count(),
        site_annotations: count(&emission.site_annotations),
    }
}

/// Prints the counters this pass changed, if any.
pub fn report(key: crate::MethodKey, pass: &'static str, before: &Snapshot, after: &Snapshot) {
    if !ENABLED || before == after {
        return;
    }
    let mut line = String::new();
    let mut note = |name: &str, a: usize, b: usize| {
        if a != b {
            use std::fmt::Write;
            let _ = write!(line, " {name}={a}->{b}");
        }
    };
    note("fallthrough", before.fallthrough, after.fallthrough);
    note("labels", before.labels, after.labels);
    note("transparent", before.transparent, after.transparent);
    note("natural_stores", before.natural, after.natural);
    note(
        "omitted_literals",
        before.omitted_literals,
        after.omitted_literals,
    );
    note(
        "omitted_returns",
        before.omitted_returns,
        after.omitted_returns,
    );
    note(
        "omitted_checks",
        before.omitted_checks,
        after.omitted_checks,
    );
    note("inline_checks", before.inline_checks, after.inline_checks);
    note("composed", before.composed, after.composed);
    note("allocations", before.allocations, after.allocations);
    note(
        "array_literals",
        before.array_literals,
        after.array_literals,
    );
    note(
        "site_annotations",
        before.site_annotations,
        after.site_annotations,
    );
    eprintln!(
        "[pass] class={} method={} {pass}{line}",
        key.class.class_def.0, key.method.0
    );
}
