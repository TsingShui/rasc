# Throws annotation scope

P1 `throws::read` now extracts only `dalvik.annotation.Throws`. Other annotations
are fully decoded for range/reference/value integrity but do not affect exception
facts. P4 `source_throws` consumes the extracted exception list without a generic
annotation-support predicate. P8 enum template admission checks its own unchanged
system-metadata allowlist through a read-only query over validated annotations.

The consumed method annotation sets were checked by P1 method admission. Their
presence does not establish Java annotation or generic-signature eligibility;
those consumers retain their existing checks. Malformed Throws values, unrelated
malformed values, resource failures and cancellation retain classified errors.

The real method that motivated this change passed the former P8 call-binding
entry rejection and now reports a specific missing method binding instead. This
is progress through analysis, not a claim of Java recovery for that method.

`tests/fixtures/throws-scope/ThrowsScope.smali` defines equal constant-return bodies
without metadata, with system Signature metadata and with a runtime annotation.
The fixture was assembled with smali 2.5.2 at API 26. The existing public annotation
suite checks equal Java availability and retention of original annotations in
`method_annotations_do_not_contaminate_declared_throws`.

Run the workspace format, Clippy, release-build and cargo-nextest gates. The
ALGORITHMS document is unchanged.
