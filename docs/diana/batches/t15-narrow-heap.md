# Boolean and narrow heap accesses

Goal: extend the shared primitive field and array producers to boolean, byte,
short and char. Contracts: TASKS T15; DESIGN §4; ALGORITHMS §2/§4.1–§4.9.
The frozen specification remains draft-7 and existing F15/M13 evidence retains
its original acceptance strength.

P2 retains the original descriptor and opcode family; P3 consumes its word
width. P4 retains field declaration, constantness and binding obligations. P5
seeds boolean Known01 and signed/unsigned narrow producer ranges and seals
store conversions. P8 independently checks the original store before admitting
its truncation or proven boolean conversion; P9 renders sealed views only.
The existing component-fact closure and ordinary method entry are reused.

Invariant: an original byte/short/char store truncates in the original order,
but that permission never repairs a Phi or an unrelated call conversion.
Boolean stores require a boolean definition or non-empty Known01 evidence;
an arbitrary integer is not silently rewritten to a Java boolean. Unused
reads, null/init/volatile effects, original identities and classified failures
remain explicit. New loops and allocations use the method ledger and
cancellation checkpoints.

Existing public tests cover numeric heap widths and declaration failures.
This batch extends those tables and independent Java contexts with fixed
narrow boundaries and boolean operations.

## Implementation and core evidence

P2 field Ops now retain DeclarationType rather than only a numeric scalar
family. `primitive_field_type` reads the exact original descriptor; encoded
field and array families remain independently replayable. The P1 descriptor/
opcode check from the preceding slice remains authoritative. `ArrayComponents`
retains the same finite descriptor closure, now consumed for all primitive loads.

P5 `primitive_stores::view` owns original store truncation and boolean proofs.
`normal::solve` seeds narrow ranges and boolean bits from the descriptor;
`solve_fields` and `solve_arrays` consume the sealed store views. Numeric-family
constraints retain Integral32 without confusing the boolean source type with an
arbitrary integer. P8 `values::verify_primitive_store` independently checks the
original-store conversion after field/array opcode and identity replay. P9
uses the existing narrow and Known01 expression forms unchanged.

The existing `numeric-fields` public table now includes all four new primitive
field and array families in both representation policies. Native volatile flag
contradictions and missing field facts still reject Java. The existing malformed
heap table also exercises direct aput-byte truncation and a boolean store after
`and-int/lit8 ..., 1`; an unproved integer-to-boolean store remains Debug with
JavaAnalysisUnavailable. This does not claim that every such DEX input is legal.

`NarrowDriver.java` extends the self-made independent Java contexts. Separate
`store-reference/fixtures/Invalid.java` and StoreDriver provide the independent
semantics of the two positive Smali store methods. These are core decompiler
fixtures, not tests of the assembler, catalog projector or runner.

Both policies' emitted Case methods compile and match five retained field,
seven retained numeric-array and four narrow-bundle JVM observations. The two
positive Smali methods also compile in their original owner context and match
six independent store observations. Signed/unsigned boundaries, overflow
truncation, boolean values, null/bounds and helper failure prefixes remain
observable. Sources, original classfile projections, public outputs and separate
reference/actual observations are under `.cache/e2e/t15-narrow-heap/`.

## Commands and result

The fixture build reuses locked Temurin 21.0.8+9, D8 8.3.37 and smali 2.5.2.
Java source targets Java8. The same original classfile projection updates the
compact catalog; no JDK scan or output-derived expectation is introduced.

```sh
JDK="$PWD/.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home"
BASE=.cache/e2e/t15-narrow-heap
"$JDK/bin/javac" --release 8 -d "$BASE/reference" crates/diana-decomp/tests/fixtures/numeric-fields/*.java
"$JDK/bin/java" -cp .cache/e2e/tools/downloads/r8-8.3.37.jar com.android.tools.r8.D8 --release --min-api 21 --output "$BASE/dex" "$BASE"/reference/fixtures/*.class
"$JDK/bin/java" -cp '.cache/e2e/tools/downloads/*' org.jf.smali.Main assemble crates/diana-decomp/tests/fixtures/numeric-fields/Invalid.smali -o crates/diana-decomp/tests/fixtures/numeric-fields/invalid.dex
"$JDK/bin/javac" --release 8 -d "$BASE/store-reference" crates/diana-decomp/tests/fixtures/numeric-fields/store-reference/fixtures/Invalid.java crates/diana-decomp/tests/fixtures/numeric-fields/store-reference/StoreDriver.java
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64 all Cargo gates and 124 core tests pass with zero skips. Diff
and sensitive-input checks pass. Full `run-tqm725bm` executes the unchanged
86 cases/177 invocations/181 seams and exits 1 with 51 cases/117 seams passing.
No passing seam or original member state/failure classification changes against
`run-rd3fhza5`. Existing required Java failures remain failures. ART and other
host platforms are not run.

This primitive-access slice is verified. Whole-T15 remains pending for reference
fields, nominal type operations and their complete independent evidence. The
next slice extends those operation products without adding a method entry.
