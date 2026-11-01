# Original reference type checks

Goal: extend original check-cast to supported nominal/array reference types and
add instanceof through the shared operation chain. Contracts: TASKS T15;
DESIGN §4; ALGORITHMS §2/§4/§9.4. The frozen specification remains draft-7;
D024/D025 and existing cast failure-prefix evidence remain required.

P1 checks reference-target format constraints. P2 retains original type identity
and common definition/use/effect facts. P3 creates the same ordinary definitions.
P5 seals reference input views and cast/boolean result types. P6/P7 preserve
storage, effect order and copies. P8 validates the original target, source
scope/name and operand/result identities; P9 prints sealed type-check statements.
No alternative method analysis or register tracker is introduced.

Invariant: an explicit original cast remains at its original site, even when
unused. Instanceof produces the original boolean result and preserves resolution
ordering. A no-throw Object input view permits Java to express the original
reference check without inventing another narrowing runtime check. Unsupported
source names or missing target evidence remain classified raw results.
New traversals and owned text use existing resource/cancellation controls.

Existing public heap tables and independent source contexts provide the extension
points.

## Implementation and evidence

P1 rejects primitive check-cast/instanceof targets as InvalidTypeOperand. P2
CastSite retains DeclarationType and InstanceOf retains original TypeId, target
declaration and source/destination registers. Both use the common read/result
and may-throw products. P5 seeds the exact cast type or boolean result and stores
one input view per operation. Cast-produced arrays seed the existing finite
component closure before array loads are typed.

P8 `cast_ops::validate/lower/verify` checks the original reference/operands,
result definition, target type, source view and checked source spelling.
TypeNames resolves nominal instanceof targets even though the result itself is
boolean. Both control representations consume the same TypeCheck statement.
P9 prints its sealed CheckKind and name; it performs no target selection.

The public heap table adds nominal/array casts and instanceof, including missing
target declarations for both used and unused results. The existing malformed
heap table adds primitive inputs/targets and typed String-to-unrelated-Box checks.
The independent Java reference uses only a no-throw Object widening before those
original checks. A new cast is never introduced to repair an ordinary field use.
Existing P6 replay coverage is retained with the statement variant rename.

Both representation policies' emitted Case methods and positive Smali methods
compile in the original helper context and match all seven independent Drivers.
TypeDriver adds thirty fixed observations of identity, boolean result, FP bits,
null/bounds, unused casts and helper/cast failure prefixes. UnrelatedDriver adds
four String/Box observations, including null and ClassCastException. Actual
bytecode retains the unused checkcast and both instanceof target families.
The original source projections and independent expected observations remain
separate from engine outputs.

## Commands and result

The shared self-made fixtures retain the Temurin 21.0.8+9 Java8 source target,
D8 8.3.37 and smali 2.5.2 recipes from the preceding heap batches. The compact
catalog is refreshed from original classfiles and the original Smali declaration
headers only; no JDK scan or fingerprint gate is introduced.

```sh
JDK="$PWD/.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home"
BASE=.cache/e2e/t15-nominal-type-checks
"$JDK/bin/javac" --release 8 -d "$BASE/reference" crates/diana-decomp/tests/fixtures/numeric-fields/*.java
"$JDK/bin/java" -cp .cache/e2e/tools/downloads/r8-8.3.37.jar com.android.tools.r8.D8 --release --min-api 21 --output "$BASE/dex" "$BASE"/reference/fixtures/*.class
"$JDK/bin/java" -cp '.cache/e2e/tools/downloads/*' org.jf.smali.Main assemble crates/diana-decomp/tests/fixtures/numeric-fields/Invalid.smali -o crates/diana-decomp/tests/fixtures/numeric-fields/invalid.dex
"$JDK/bin/javac" --release 8 -cp "$BASE/reference" -d "$BASE/store-reference" crates/diana-decomp/tests/fixtures/numeric-fields/store-reference/fixtures/Invalid.java crates/diana-decomp/tests/fixtures/numeric-fields/store-reference/*.java
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64 all Cargo gates and 124 core tests pass with zero skips. Diff
and sensitive-input checks pass. Full `run-owgamz2j` executes the unchanged
86 cases/177 invocations/181 seams and exits 1 with 51 cases/117 seams passing.
Its passing seam set and every original member state/failure classification
equal `run-vgp_r580`. Compiler logs, projections, observations, bytecode and
`member-comparison.json` are under `.cache/e2e/t15-nominal-type-checks/`.
ART and other host platforms are not run.

The nominal type-check slice is verified. The T15 contract rollup is recorded in
[t15-acceptance](t15-acceptance.md).
