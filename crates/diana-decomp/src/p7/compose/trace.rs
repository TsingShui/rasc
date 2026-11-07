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

/// Why one original check-cast keeps its own statement: the first refusal that
/// stopped it from printing at a read. The tag names that condition, so a sweep
/// can group the remaining checks by cause instead of reading each one.
pub fn cast_refused(key: crate::MethodKey, statement: crate::p7::StatementId, why: &'static str) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[cast] class={} method={} statement={} why={why}",
        key.class.class_def.0, key.method.0, statement.0
    );
}

/// One final layout decision per P6-owned condition. Test indices are zero-based;
/// the reason is the first blocking admission check, not every possible cause.
pub fn condition_refused(
    key: crate::MethodKey,
    control: crate::p7::ControlId,
    test: Option<usize>,
    statement: Option<crate::p7::StatementId>,
    why: &'static str,
) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[condition] dex={} class={} method={} control={} layout=explicit test={test:?} statement={:?} why={why}",
        key.class.dex.0,
        key.class.class_def.0,
        key.method.0,
        control.0,
        statement.map(|id| id.0),
    );
}

pub fn condition_selected(
    key: crate::MethodKey,
    control: crate::p7::ControlId,
    values: usize,
    stores: usize,
) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[condition] dex={} class={} method={} control={} layout=short_circuit values={values} stores={stores}",
        key.class.dex.0, key.class.class_def.0, key.method.0, control.0,
    );
}

/// Only installed store compositions print this event; refused proposals do not.
pub fn condition_store(
    key: crate::MethodKey,
    control: crate::p7::ControlId,
    operation: crate::p2::OpId,
    statement: crate::p7::StatementId,
    operand: crate::p7::ExprId,
    storage: crate::p7::StorageId,
) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[condition-store] dex={} class={} method={} control={} operation={} statement={} operand={} storage={}",
        key.class.dex.0,
        key.class.class_def.0,
        key.method.0,
        control.0,
        operation.0,
        statement.0,
        operand.0,
        storage.0,
    );
}

/// This else contains the original final-other edge's copies and no body.
pub fn condition_copy_else(
    key: crate::MethodKey,
    control: crate::p7::ControlId,
    transfer: crate::p7::TransferId,
    copy: crate::p7::CopyPlanId,
    steps: usize,
) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[condition-copy-else] dex={} class={} method={} control={} transfer={} copy={} steps={steps}",
        key.class.dex.0, key.class.class_def.0, key.method.0, control.0, transfer.0, copy.0,
    );
}

pub fn expression_selected(
    key: crate::MethodKey,
    kind: &'static str,
    operation: crate::p2::OpId,
    statement: crate::p7::StatementId,
    operand: crate::p7::ExprId,
) {
    if !ENABLED {
        return;
    }
    eprintln!(
        "[expression] dex={} class={} method={} kind={kind} operation={} statement={} operand={}",
        key.class.dex.0, key.class.class_def.0, key.method.0, operation.0, statement.0, operand.0,
    );
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
