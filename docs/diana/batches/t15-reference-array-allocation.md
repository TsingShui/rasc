# Reference array allocation

Goal: recover Object[] new-array through the existing allocation operation and
array storage chain. The T15 contract review exposed the primitive-only decoder
and missing Object[] result admission. Contracts: TASKS T15; DESIGN §4;
ALGORITHMS §2/§2.4/§4. Specification: unchanged draft-7.

P2 produces a distinct primitive/Object allocation kind, original TypeId, length
use and result definition. P3/P5 retain common identities, array descriptor facts
and checked length views. Primitive FreshArray proofs remain primitive-only.
P8 checks original type/registers, Java qualifier binding and allocation kind;
P9 prints the sealed allocation. No method-specific recovery is added.

Invariant: Object[] allocation preserves negative-length and allocation/effect
ordering; initialized reference slots remain null until original stores execute.
Object[] cannot acquire a primitive fill certificate. Missing source qualifier
facts remain Unsupported. New traversals/text use the method ledger and
cancellation checkpoints. Existing core fixtures and independent Drivers are
extended with fixed allocation counterexamples.

## Implementation and evidence

ArrayCreation now carries ArrayKind::Primitive or Object. The same NewArray Op,
SSA definition, P5 length view and P8 Allocation statement consume that product.
The array component closure seeds Object[] facts from the allocation. Primitive
fill proofs explicitly require the Primitive variant. Object[] return signatures
use the common reference declaration parser instead of failing before decoding.
P8 validates the original descriptor and Java qualifier before admitting the
sealed Object component spelling.

The prior release example is retained locally as `baseline-inspect`. Against
the new fixed self-made input, both `objects` and `unusedObjects` produce Debug/
JavaAnalysisUnavailable before the correction. The current public outputs
recover both plus `zeroObjects` as Java. The existing public heap table adds
these rows and a contradictory qualifier-evidence rejection. Its malformed-input
table adds an Object[] allocation followed by primitive fill payload, which
retains Malformed/InvalidArrayPayloadType rather than gaining a fill certificate.

Both policies' emitted Case methods compile and match all six original Java
Drivers in this batch. AllocationDriver adds fifteen fixed observations of
returned array identity/type/length, null defaults, negative length, bounds and
allocation/helper failure order, including an unused allocation. Old primitive
access, reference, cast and instanceof observations remain equal.

## Commands and result

The shared fixture uses Temurin 21.0.8+9 with Java8 targeting, D8 8.3.37 and
smali 2.5.2. Original classfiles and Smali declarations update the compact
catalog. The existing build recipe is unchanged apart from the added sources.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64 G0 and 124 core tests pass with zero skips; diff and sensitive-
input checks pass. Full `run-xm8tqx23` runs 86 cases/177 invocations/181 seams
and exits 1 with 51 cases/117 seams passing. Its passing seam set and every
original member's state/failure classification equal `run-owgamz2j`.
Baseline/current public outputs, compiler logs, original projections, independent
observations and `member-comparison.json` live under
`.cache/e2e/t15-reference-array-allocation/`. ART and other host platforms are
not run.

Object[] new-array is verified. T15 remains pending while the array-construction
review checks filled-new-array and its original move-result binding through the
same operation chain.
