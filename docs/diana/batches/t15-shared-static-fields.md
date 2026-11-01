# Shared static int fields

Goal: recover static int reads and writes through the ordinary-method chain.
Contracts: DESIGN §3.4/§4, ALGORITHMS §2/§4/§9.4, TASKS T15.
Fixed specification: draft-7; D044 and D080 exercise static-field bindings and
constant rejection. Existing static-field JVM sources and Driver under
`.cache/e2e/t15-static-fields/` provide independent initialization, volatile,
unused-read and failure-prefix evidence.

P2 produces original static field operations, definitions/uses and conservative
heap/class-init effects. P4 consumes catalog/native declaration and constantness
facts. P5 checks int value conversions; P6/P7 retain ordered storage/control.
P8 checks the original opcode, reference, operands and source qualifier before
P9 prints the sealed operations. No method-specific analysis entry is added.
Invariant: every access remains at its original site, including unused reads;
constant-inlined Java and missing or contradictory declaration facts are rejected.

Every new traversal and owned string uses the existing method ledger and
cancellation checkpoints. The existing field-composition test covers instance
volatile evidence; the new public static-field test covers the distinct
class-initialization binding obligation in both representation policies. It
reuses the existing static sources rather than introducing a new interpreter.

## Implementation and fixtures

`p2_flow::normal` adds static operations to decoding, reads, destinations and
conservative class-init/heap effects. `p4_calls::normal` and `field_facts` retain
storage, original owner/name/reference and checked flags. Constant rejection
precedes qualifier admission so D080 retains its original classified reason.
`p5_types::normal::solve_fields` checks the static store's sole input.
`p8_java::field_ops` lowers and replays original opcode/register/reference,
qualifier, flags, result and value conversions. `calls::operations` consumes
these statements; `p9_emit::field_ops` only renders their sealed receiver form.

`tests/fixtures/static-fields/` under the engine package contains the self-made
Java sources, Driver, DEX and reduced declaration catalog. Sources reuse the
existing local T15 evidence and the repository's fixture licensing. The catalog
retains fixture declarations and the existing Object/Throwable/Exception/
RuntimeException facts; it is not generated from engine output.
The DEX was rebuilt with locked Temurin 21.0.8+9 and D8 8.3.37:

```sh
JDK="$PWD/.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home"
BASE=.cache/e2e/t15-shared-static-fields
mkdir -p "$BASE/reference" "$BASE/dex"
"$JDK/bin/javac" --release 8 -d "$BASE/reference" crates/diana-decomp/tests/fixtures/static-fields/*.java
"$JDK/bin/java" -cp .cache/e2e/tools/downloads/r8-8.3.37.jar com.android.tools.r8.D8 --release --min-api 21 --output "$BASE/dex" "$BASE"/reference/fixtures/*.class
```

The public test exercises all five existing Reader methods and rejects missing
facts, contradicted native volatile flags, constant/unknown final reads and a
shadowed source qualifier. The full E2E suite retains the independent external
constant rejection cases. Existing resource/cancellation tests are reused.

## Evidence

On macOS arm64, fmt, strict workspace/all-target/all-feature Clippy, locked
workspace release, diff and sensitive-input checks pass. Cargo-nextest runs all
123 core tests with zero skips. Logs and public outputs live under
`.cache/e2e/t15-shared-static-fields/`.

All five emitted Reader methods compile in the original independent helper
context with Java8 targeting. The existing ten JVM observations match the
original Java and the previously recorded independent observations. Each invokes
twice, including initializer failure followed by erroneous-class access. The
unused reads retain getstatic; write retains putstatic before the later read.
Native volatile flags remain required. `observations.json`, compiler logs and
`actual-bytecode.txt` retain the evidence.

Full `run-ix2eoiq7` executes the unchanged draft-7 required set: 86 cases,
177 invocations and 181 seams. It exits 1 with 51 cases/117 seams passing.
No passing seam is gained or lost against `run-q2ewkrrr`. Original-member review
finds exactly one change: D044 method 2 (`get`) improves from Debug to Java.
Every other member retains its state and classified failure. D080's two
invocations pass after correcting the observed constant-classification regression.

D044 still requires whole-class Java, including static initializer recovery and
class assembly. Those obligations remain failing; the member improvement does
not pass the case or establish whole-T15 acceptance. ART and other host platforms
are not run. Next action: select the next T15 producer gap against its fixed
independent evidence before starting another batch.
