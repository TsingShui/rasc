# T13 coverage and acceptance

Goal: finish the required normal-region evidence, including nested/hard graphs,
exact coverage rejection and dependency-ready implementation facts. TASKS T13 and
ALGORITHMS §6 own acceptance. Existing fixed Drivers cover behavior; existing
copy-certificate tests do not inject invalid native region certificates. M11
therefore needs one minimal core regression through a public Engine-produced
storage body and the actual native coverage validator, not runner/comparator tests.

The dominance producer now initializes a charged RPO worklist and schedules only
successors affected by an idom change. Ancestor walks and each scheduled edge
remain charged. A chosen loop also checks its reverse predecessor closure from
the latch, stopping at the header. Each discovered member is header-dominated;
the exact closure equals the planned cyclic leaves. Exit-only blocks remain
separately owned. No recursion, topology changes or default numeric caps are added.

The single new native core regression opens a self-made DEX through public Engine,
uses its admitted storage and checks the actual region validator. The valid plan
passes; removing a leaf, losing the latch EdgeId, switching exit scope or duplicating
a leaf each yields InternalInvariant, never an unsupported-pattern success. The
existing irreducible public core test still verifies dispatcher correspondence
and the absence of DA-default reads. All 118 cargo-nextest tests pass with zero
skips, together with fmt, strict all-target/all-feature Clippy and release build.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D004 --case D005 --case D006 --case D007 --case D008
python3 tools/e2e.py run --representation dispatch_only --case D001 --case D002 --case D003 --case D004 --case D005 --case D006 --case D007 --case D008
```

On macOS arm64, `run-qdpt8l8h` and `run-h60ugcqh` execute the complete selected
eight-case set. D005's nested named exit JVM returns 6 in both modes, using the
legal dispatcher because its shape is unmatched. ALGORITHMS §6.2/§6.5 explicitly
allows ordinary schema mismatch to use that representation; nested source shape
is not the behavioral acceptance gate. Existing diamond, loop, continue, break
and switch observations remain passing in both modes.

Both commands exit 1: D006's two required boolean short-circuit invocations retain
Debug/BindingEvidenceMissing for exact ()Z calls. Those invocations are not counted
as Java passes, and the complete selected denominator is preserved. T13 remains
pending on that actual producer obligation. Next action: add the exact boolean
call-result contract and typed storage through the existing T11 binding chain,
then rerun both adapters. The last complete mandatory report is still
run-9kw1nuw6; no complete-set acceptance is claimed by this batch.

## T13 acceptance

The committed exact Boolean result artifact `406076e` discharges D006 without
weakening source types or moving effects. The natural-loop writer now uses a
labelled while and explicitly names that lexical scope for every break/continue,
as required by ALGORITHMS §6.2. Copies still precede each named transfer.

Final `run-nxu3v2bb` (StructuredThenDispatch) and `run-yrju3pcn` (DispatchOnly)
each pass all eight selected fixed cases / 20 invocations after that renderer
change. This includes the D001 diamond, D002/D003 zero/several-iteration copy
cycles, D004 shared continue latch, D005 nested named exit, D006 actual-branch
boolean effects, D007 switch behavior and D008 exit copies. Natural simple-loop
ASTs and the legal unmatched-graph dispatcher remain distinct owned products.
The native M11 regression proves classified rejection of the four invalid
certificate forms; the existing irreducible and protected-method core cases pass.

Final fmt, strict all-target/all-feature Clippy, locked release and all 118
cargo-nextest tests pass with zero skips. The complete mandatory report
`run-nscdi4n9` immediately before the label-only change records 43 whole cases /
98 seams and no lost pass, retaining 86 cases / 177 invocations / 181 seams.
The full-set command exits 1 for real remaining obligations; it is not a claim
of EngineCoreRC or whole-class acceptance. The label-only change reruns the full
T13-related fixed set in both modes, not the complete mandatory set.

T13 is accepted for the executed native/macOS-arm64 JVM profile: required
behavioral observations, source correspondence, M11 rejection and resource/safety
gates are recorded across the natural-loop, guarded-exit, body-diamond and this
batch. Supplemental effect/failure observations remain supplemental rather than
extra fixed passes. ART/other-platform execution is not recorded as a pass.
Next action: start T14 from the accepted normal-storage/coverage producers.
