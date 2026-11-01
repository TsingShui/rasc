# Filled array producers

The operator-approved float oracle correction and current verification are in
[the derived-array batch](t15-derived-array-types.md#review-decision-and-correction).
The producer admits int and proven reference filled forms. The supplemental
float form retains its original instructions as Unsupported. The earlier float
JVM comparison below is a historical execution observation, not semantic acceptance.

Goal: decode filled-new-array and /range, bind their original optional
move-result-object, and recover checked array initialization through the shared
ordinary-method chain. Contracts: TASKS T15; DESIGN §4; ALGORITHMS §2/§4.
The fixed specification remains draft-7; existing D026/D027/D073/D074 evidence
is retained.

P2 records original array type, argument words and optional result site. P3
binds the same definition/use chain with all argument reads before result commit.
P5 seals element views. P6 owns a temporary for an unused result because Java
array creation is not a standalone statement expression. P8 replays original
arguments/result binding and consumes that storage; P9 renders sealed contents.
No complete-method register tracker or recovery route is added.

Invariant: argument order and word interpretation are unchanged, and an unused
result cannot delete allocation. Filled arrays gain no primitive FreshArray
certificate by assertion. Element lists, type checks, temporary storage and
rendering use existing resource/cancellation controls. Fixed self-made Smali and
an independent Java context extend the existing public heap tests.

## Implementation and evidence

P2 FilledArraySite retains original type, argument register sequence and producer
PC. FilledResult distinguishes a discarded result from a stored result with its
original destination/PC. The shared SSA read iterator consumes both call and
filled-array argument products before normal result commit. No implicit result
is invented as a physical DEX register.

P5 seals one FilledArrayUse with final array kind and checked element views.
P6 allocates storage only for a discarded result; stored results use their
existing SSA local. P8 Allocation contents distinguish length allocation from
checked element initialization. Its replay checks raw 35c/3rc arguments,
adjacent object-result binding, original reference, local ownership and every
input view. P9 prints those sealed contents in order. Primitive FreshArray
proofs remain separate and are not granted to filled arrays by assertion.

The retained prior example returns Debug/JavaAnalysisUnavailable for the new
filledObjects, filledRange and filledUnused inputs. Current public outputs
recover all positive counterexamples. The existing public heap table adds
reference/int, range, result-register reuse, zero-argument and discarded-
result cases, plus the corrected unsupported float counterexample. A wrong move-result kind retains Malformed/InvalidResultKind.
The fixtures are authored Smali with independent Java semantics in
`store-reference/fixtures/Invalid.java`; no expected value comes from engine
output.

Both policies' recovered methods compile and match all three independent store/
type/filled Drivers. FilledDriver adds eight fixed observations of identity,
argument order, int boundaries, raw float signed-zero/NaN bits, six-word range,
result overwrite and distinct empty-array allocations. Actual bytecode retains
newarray in the discarded-result method. Existing store and unrelated-type
observations remain equal.

## Commands and result

The shared fixture retains Temurin 21.0.8+9 Java8 targeting and smali 2.5.2.
The compact catalog is updated from the original Smali declarations. Baseline
and current public outputs, source contexts, compiler logs, bytecode and
independent observations are under `.cache/e2e/t15-filled-arrays/`.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64 G0 and all 124 core tests pass with zero skips. Diff and
sensitive-input checks pass. Full `run-q7bywtu6` executes the unchanged
86 cases/177 invocations/181 seams and exits 1 with 51 cases/117 seams passing.
Its passing seam set and every original member state/failure classification
equal `run-xm8tqx23`; `member-comparison.json` records that review.
ART and other host platforms are not run.

The derived-array batch verifies the corrected filled-array profile and extends
the nominal/multidimensional type domain. Its current seven supported FilledDriver
observations replace the earlier eight-observation comparison; the float member
is covered by its classified raw regression rather than a Java initializer.
