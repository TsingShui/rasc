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

## Final evidence

The reviewed production revision is clean `421b4fd`. Fmt, strict workspace/
all-target/all-feature Clippy, locked release, diff and sensitive-input checks
pass. Cargo-nextest runs all 122 core tests with zero skips. The actual native
logs are under `.cache/e2e/architecture-reacceptance/`; no new tests or test
frameworks were introduced.

Full `run-q2ewkrrr` records all 86 required cases, 177 invocations and 181 seams.
It exits 1: 51 cases and 117 seams pass. Its passing seam set equals
`run-4a8u0jw8`; required Java/JavaUnit failures are not counted as passes. Every
previous non-Java member retains its failure kind and diagnostic code. The only
member-state change is the operator-approved withdrawal of the unaccepted D044
static-field draft; clean `419790c` already reports that member as Debug with the
same input/catalog/control. The native check and archive are recorded in
[unified-method-exit](unified-method-exit.md).

Dispatch-only `run-emtdt2tk` at clean `1f1ac8a` passes D001–D008/D061/D083:
10 cases and 24 invocations/seams. The subsequent production correction changes
only source-group rejection classification; the affected existing cases are
separately checked above. The final shared-fields Driver compiles current
emitted methods and matches all 88 independent Java observations in dispatch
mode. The same 88 observations pass the final natural/dispatcher-default code
in the preceding boundary batch. Existing original Drivers, helpers and fixed
expectations are reused without extension.

All 22 existing primitive JVM contexts also match original Java and their worked
expectations on `421b4fd`. The three original wide-read/NaN negative observations
retain their statuses and exact causes/coordinates. These supplemental matches
are not added fixed-case passes. Existing empty-fill width/reference/undefined
alias classifications likewise retain their original causes and coordinates.
The JVM uses locked Temurin 21.0.8+9 with Java8 source targeting on macOS arm64;
ART and other host-platform execution remain not-run.

## Historical acceptance review

| Prior scope | Architecture reacceptance decision |
|---|---|
| T01–T05 native input, resource, raw/metadata and owned output producers | Public native regressions and fixed engine seams remain observed. Request cancellation, checked ranges, original data and local failures retain their contracts. Application/whole-product obligations remain distinct. |
| T06–T10 numeric and ordinary CFG producers | Shared P2/P3/P5/P6/P8 replace the scalar path. Fixed D001–D017, copy/width/resource guards, exact UTF16/target-limit behavior and the primitive supplemental evidence retain their producer scope. |
| T11–T12 call and initialization subsets | Original identities, typed arguments/receivers, checked-throws evidence, Uninit/deferred storage and first-statement proofs now use shared values. The existing method-context evidence is retained; D020/D021/D041/D042 Mixed classes do not become accepted JavaUnit. |
| T13–T14 normal regions/switches | Reaccepted for the executed normal-control producer profile. P7 reads P6 before Java lowering; natural structure and dispatcher share the independently checked block/edge/copy representation. Unmatched graphs still dispatch, including the previously recorded D005 boundary. |
| T15 implemented cast/array/int-instance-field/fill subsets | Shared SSA/effect/storage proofs and independent failure-prefix observations are retained. The frozen static-field draft was explicitly withdrawn and archived, not promoted into acceptance. Whole-T15 acceptance is not established. |
| T16–T23 and T25 remaining obligations | Existing negative/raw/declaration evidence remains evidence only for those exact obligations. Protected exception/monitor behavior, broader initialization/generated/nested source and the complete rollup are not accepted by this correction. The full report retains their actual failures. |

## Decision

The architecture correction is complete for the migrated producer scopes: one
ordinary-method phase chain, phase-owned initialization/fill proofs, no parallel
whole-method recovery trackers, P7 before one P8 method verifier, and one
classified read-only raw exit. The original architecture baseline's temporary
routes are removed; surviving metadata re-exports do not execute recovery.

This decision does not assert EngineCoreRC/ProductRC. Fixed expectations,
denominators, acceptance strength and TASKS are unchanged. The goal stops at
architecture correction; remaining feature work requires a separate decision.
