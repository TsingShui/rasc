# T05 public-library adapter

## Implemented

Contracts: TASKS T05, DESIGN §2.2–§3.4 and ALGORITHMS §7.
`tools/e2e.py run` builds and calls the public Engine development example, then
submits actual owned sources, diagnostics, UTF8 slices and spans to the existing
comparator. Per-run artifact directories preserve previous observations. The
required case/invocation/seam set comes from the unchanged draft-7 specification.

The native partial signature catalog consumes exact UTF16 signatures from the
certified catalog artifacts. It establishes signature presence, not dispatch,
hierarchy, throws or overload completeness. Reachable invokes without local
headers or supplied signature evidence produce `BindingEvidenceMissing`.
Header-only local lookup does not read another method's body.

Production corrections exposed by the fixed core cases:

- Debug snapshots retain registry mnemonics instead of uppercase substitutions.
  Two existing opcode expectations now use the original spelling.
- Descriptor-defined JVM parameter-word and MUTF8 limits apply to executable and
  no-code methods. They do not estimate generated body size.
- A target-limit failure retains a complete Debug snapshot instead of forcing
  Unavailable.

The development example writes large results directly to an artifact when asked.
This removes the development subprocess log limit from the D086 result path;
it does not add an engine limit or claim Java recovery.

## Evidence

Run `python3 tools/e2e.py run`.
The actual report is `.cache/e2e/engine/run-uyk7qvdu/report.json`; its revision
records `296875d` plus a dirty source tree for this batch.
The derived full set contains 86 cases, 177 invocations and 181 seams.
Four whole cases and eight seams passed:

- D059, D060, D061 and D084 passed their complete invocation sets.
- D075 static-over, instance-over, abstract-at and abstract-over passed.
  Abstract-at includes the existing compiler-backed declaration check.

The full run exited 1. D075 static-at and instance-at still require executable
Java. D086 unlimited-default now reaches the comparator and fails because its
actual result is Debug rather than required Java. Unsupported request controls,
other seams and application-side obligations retain explicit non-passes.
The narrower raw run is `.cache/e2e/engine/run-403pfseb/report.json`.

Checks passed: workspace fmt, all-target/all-feature Clippy with warnings denied,
locked release build, 110 existing cargo-nextest core tests with zero skips,
`git diff --check` and the sensitive-input audit. No new test functions were
added. The fixed expectations, manifest, input certificates and TASKS.md are
unchanged.

## Remaining acceptance

T04/T05 remain unaccepted. Executable Java recovery and controlled-request
observations still need their production consumers and fixed evidence.
D046 also fails original field-initializer comparison: the native snapshot links
`fields[].initial.encoded_static_values_ordinal` to `static_values.entries`,
whereas the independent observation contains inline `tag`/`value` facts. The
native entries retain the observed boolean, integer and null values. Resolve the
owning producer representation before accepting that obligation; do not change
independent expected values or hide the difference through normalization.

Next: complete the public observation of typed input/control failures, then
implement the first executable Java producer along the D009 capability chain.
