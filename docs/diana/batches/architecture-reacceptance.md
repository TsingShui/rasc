# Architecture reacceptance

This batch rechecks the historical producer scopes against the shared pipeline
rather than treating earlier commits or isolated method passes as architecture
acceptance. Contracts: DESIGN/ALGORITHMS and the unchanged TASKS acceptance rules.

The final review found six non-Java members whose Unsupported kind was unchanged
but whose source-group rejection code had changed. `engine::normal_member` now
retains JavaAnalysisUnavailable for that source-language rejection while still
running shared analysis before the P8 eligibility check. The existing
D048/D060/D084 contexts retain their original member states and diagnostic codes
in `run-0r3x30er`. D048 still fails required whole-class Java; the run exits 1.

Fmt, strict Clippy, locked release and all 122 cargo-nextest tests pass with zero
skips. No tests, expected values or cases were added. The clean-revision full
reacceptance and its historical-scope decision follow after this correction.
