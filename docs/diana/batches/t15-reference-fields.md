# Reference field accesses

Goal: extend the ordinary field chain to reference descriptors with exact typed
assignment and no added runtime check. Contracts: TASKS T15; DESIGN §3.4/§4;
ALGORITHMS §2/§4/§9.4. Fixed specification: draft-7; existing heap and external
constant evidence remains required.

P1 retains the field type's original TypeId. P2 records builtin, supported array
or nominal reference declarations in the same field operations; P3 retains the
same definitions and uses. P4 binds original declaration/flags/name facts. P5
admits exact reference assignments, proven null assignments and reference-to-
Object widening, and propagates field-produced array descriptor facts through
the existing component closure. P8 checks original references, source names,
input/result identities and assignment views; P9 prints sealed statements only.

Invariant: an Object value does not acquire a narrower field type through a new
cast merely to satisfy javac. Missing assignability evidence keeps a classified
raw result. Final primitive/String constantness remains checked without treating
an Object-typed final as a Java compile-time constant. Original null/init/volatile
and effect order remain observable. Existing method ledgers and cancellation
checkpoints cover each traversal and allocation.

Existing public heap tables and independent Drivers are extended with reference
field counterexamples.

## Implementation and evidence

`p1_input::FieldDeclaration` retains its original field TypeId as an internal
producer fact. `p2_flow::normal::field_type/reference_type` selects supported
reference declarations without inventing names or casts. FieldAccess replaces
the numeric-only declaration product. Field-produced primitive-array descriptors
seed the existing P5 component closure, so a later array load does not guess its
float/int family. The public input and Engine interfaces are unchanged.

P5 `references::store_view` admits only exact reference types, a proven null
assignment or reference-to-Object widening in this profile. P8
`values::verify_reference_store` independently checks source identity and those
views after field opcode/flags/name replay. NullAssignment is a distinct sealed
assignment view; it cannot enter method-return verification or overload binding.
P9 renders it as an untyped null assignment and performs no type analysis.

The existing public heap table adds Object/String/nominal/array fields, static
references, null assignments, Object widening and unused reads. Missing or
contradicted native volatile facts still reject Java. Unknown constantness of
an Object-typed final retains Java because that source type cannot become a
Java compile-time constant. A separate pair of Smali methods shares the same
complete catalog: exact Box-to-Box assignment passes, while Object-to-Box without
proof remains Unsupported/JavaAnalysisUnavailable and generates no added cast.
The catalog includes the original Smali declaration facts, not engine output.

Both policies' emitted Case methods compile in the original helper context and
match all retained Drivers plus ten new reference identity/mutation/failure
observations. These cover null, helper exceptions, field-produced float arrays,
bounds, null assignments, safe Object widening and unused reads. Actual bytecode
retains the unused finalObject getfield. The ordinary Java class helpers remain
independent; whole-class acceptance is not inferred from these method contexts.

## Commands and result

The shared self-made fixture uses the existing Temurin 21.0.8+9 Java8 targeting,
D8 8.3.37 and smali 2.5.2 recipes. Original classfile projections update only the
fixture declarations; no JDK scan or fingerprint gate is added.

```sh
JDK="$PWD/.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home"
BASE=.cache/e2e/t15-reference-fields
"$JDK/bin/javac" --release 8 -d "$BASE/reference" crates/diana-decomp/tests/fixtures/numeric-fields/*.java
"$JDK/bin/java" -cp .cache/e2e/tools/downloads/r8-8.3.37.jar com.android.tools.r8.D8 --release --min-api 21 --output "$BASE/dex" "$BASE"/reference/fixtures/*.class
"$JDK/bin/java" -cp '.cache/e2e/tools/downloads/*' org.jf.smali.Main assemble crates/diana-decomp/tests/fixtures/numeric-fields/Invalid.smali -o crates/diana-decomp/tests/fixtures/numeric-fields/invalid.dex
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64 all Cargo gates and 124 core tests pass with zero skips. Diff
and sensitive-input checks pass. Full `run-vgp_r580` executes the unchanged
86 cases/177 invocations/181 seams and exits 1 with 51 cases/117 seams passing.
Its passing seam set and every original member state/failure classification
equal `run-tqm725bm`. Artifacts, compiler logs, observations, bytecode and
`member-comparison.json` are under `.cache/e2e/t15-reference-fields/`.
ART and other host platforms are not run.

This reference-field slice is verified. Whole-T15 remains pending for nominal
explicit casts, instanceof and the final F15 obligation review. Those operations
will extend the same checked definition/use/effect products.
