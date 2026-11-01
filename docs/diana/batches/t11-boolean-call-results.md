# T11 boolean call results for F13

Goal: discharge D006's exact ()Z static calls without treating a boolean as int
storage or hoisting the second short-circuit effect. Preserve original call/result
atoms, complete binding evidence, typed copies and failure classifications. Existing
D006 JVM Drivers supply the fixed core acceptance; no new tooling tests are needed.

P4 now seals Boolean separately from Int for an exact original Z result. Existing
complete candidate/access/prototype/throws checks are unchanged. P3 preserves the
original single-word move-result atom, seeds Boolean kind and the 0/1 range, and
keeps boolean storage distinct through typed SSA/copies. Independent original
instruction replay still checks result opcode, register and adjacency. Integer-only
straight-line call chains decline this result rather than misdeclare it as int;
the existing general checked storage producer handles it. No casts or effect
hoisting are introduced.

```sh
python3 tools/e2e.py run --case D006
python3 tools/e2e.py run --representation dispatch_only --case D001 --case D002 --case D003 --case D004 --case D005 --case D006 --case D007 --case D008
python3 tools/e2e.py run
```

On macOS arm64, `run-h96dvwou` passes both D006 invocations with independent JVM
Driver values/events. `run-yqo602hj` passes all eight selected fixed cases and 20
invocations in DispatchOnly. The full default-adapter `run-nscdi4n9` retains 86
mandatory cases, 177 invocations and 181 seams: 43 whole cases and 98 seams pass.
Only D006's two seams are newly passing relative to run-9kw1nuw6; no pass is lost.
The full command exits 1 for remaining genuine failures.

Supplemental authorized sources under `.cache/e2e/t11-boolean-call-results/` reuse
D006's original and/or expressions and vary opaque helper boolean values/failures.
Locked javac/D8 inputs and classfile declaration facts stay separate from actual
sources. Both adapters' actual methods compile with boolean locals and match
independently authored expectations plus the reference JVM for all four truth
combinations, failure in the first call, and failure versus short-circuit skip in
the second call. observations.json retains seven configurations / 14 method
observations per mode. These are supplemental core observations, not extra fixed
passes or runner/comparator tests.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build and all 118 cargo-nextest core tests pass with zero skips. No new tests or
fixed oracle changes were needed for this producer. TASKS.md and the required
denominator remain unchanged. This artifact discharges the D006 producer gap in
T13; shared T11/class acceptance remains tracked separately.
