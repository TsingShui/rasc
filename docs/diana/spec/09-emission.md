# 09 — Emission and presentation

Code: `crates/diana-decomp/src/p9_emit/`; public report entry:
`report.rs::render`. CLI presentation is in `apps/diana/src/cli/`.

P9 renders products and original facts already supplied by earlier phases. It
does not perform a second analysis, choose new types or repair missing semantics.
The report retains original identities and source spans. Text and JSON represent
the same result rather than selecting different recovery paths.

Debug output is an analysis-oriented snapshot of original DEX facts, not Java or
a promise of reassembly. Unknown or undecoded ranges remain explicit. P9 MUST
respect output quotas and cancellation; errors MUST NOT be disguised as a
complete result.

Existing debug and presentation rules remain in
[ALGORITHMS §7–8](../ALGORITHMS.md#7-original-dex-debug-output) and
[DESIGN §6](../DESIGN.md#6-cli-contract).
