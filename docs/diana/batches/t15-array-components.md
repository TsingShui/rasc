# Array component evidence

Goal: extend the ordinary array chain to float and wide array accesses without
sealing an int load before descriptor evidence is available. Contracts: TASKS
T15; DESIGN §4; ALGORITHMS §2/§4.3–§4.7. Fixed specification: draft-7;
existing D026/D027/D073/D074 evidence and original public field/array composition
regressions remain required.

P2 produces encoded array access kinds and exact wide definition/use widths.
P3 retains the same identities and word ownership. P5 closes finite descriptor
component facts along real move/Phi edges before numeric families, and uses
those facts for load producers and store requirements. This is a phase-owned
type-fact closure, not a register or alias tracker. P6/P7 retain ordered storage
and copies. P8 replays the original access opcode, component and value view;
P9 prints only the existing sealed array statements.

Invariant: the actual array descriptor decides float/int and double/long
interpretation, while unknown/conflicting array evidence cannot authorize a
cast. Reads and writes, including unused reads, retain null, bounds, store and
volatile/call failure prefixes. Wide loads/stores consume the same checked
SSA word products as numeric fields. Every new traversal and allocation uses
the existing method ledger and cancellation checkpoints.

Existing coverage admitted int/Object arrays only. This batch extends that public
coverage with fixed float/wide counterexamples and reuses independent Java
contexts rather than adding a new comparison framework.

## Implementation

`p2_flow::normal` decodes aget-wide/aput-wide and records the two-word result or
store input. ArrayElement keeps the encoded opcode family; it checks the full
component declaration rather than equating byte/short/char with an int array.

`p5_types::array_components::collect` seeds finite component masks from original
parameter and new-array descriptors and unions them only along SSA move/Phi
edges. Each growth adds a bit from a fixed descriptor domain; no final family or
source type feeds back into this closure. The current implementation rescans
operations/edges until no bit grows; its worst-case cost depends on descriptor
facts and propagation edges and is charged per visit. It does not claim linear
time. Unknown or conflicting component sets cannot seal an array load.

`normal::solve` uses that evidence to seed load types before numeric constraints.
`numeric::constrain` checks numeric store requirements from the same descriptors;
`normal::solve_arrays` seals the final index/value views against final array
storage. P8 `array_ops::verify` independently checks original registers/opcode,
component kind, definition identity and the existing numeric-view replay. P9's
array statement renderer is unchanged.

The existing engine `tests/fixtures/numeric-fields/Case.java` adds array accesses,
unused reads, literal stores and a float-array branch Phi. `ArrayDriver.java`
contains fixed value/bit/failure observations. `Invalid.smali` adds overwritten
wide-load, partial wide-store and wrong component counterexamples. These extend
the existing public heap tests with table rows; no duplicate resource harness is
introduced. The numeric-field source snapshot remains under its preceding
`.cache/e2e/t15-numeric-fields/source-snapshot/` evidence directory.

## Execution evidence

Rebuild the shared fixture with locked Temurin 21.0.8+9 and D8 8.3.37, and
assemble the malformed methods with locked smali 2.5.2:

```sh
JDK="$PWD/.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home"
BASE=.cache/e2e/t15-array-components
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

On macOS arm64 all Cargo gates and 124 core tests pass with zero skips. Both
representation policies' emitted Case methods compile in the original helper
context and match all five existing field and seven new array JVM observations.
Those observations include raw FP bits, null/bounds/helper failures, old values,
mutation prefixes, unused reads, literal views and both Phi branches. Sources and
independent Driver observations remain separate from engine output; the catalog
is updated from original classfiles with the existing independent reader.

Full `run-rd3fhza5` executes the unchanged draft-7 set of 86 cases,
177 invocations and 181 seams. It exits 1 with 51 cases/117 seams passing.
Its passing seam set and every original member's state/failure classification
equal `run-pz027ipo`. Diff and sensitive-input checks pass. Logs, public outputs,
compiled contexts and `member-comparison.json` are under
`.cache/e2e/t15-array-components/`. ART and other host platforms are not run.

The float/wide-array slice is verified. Whole-T15 remains pending for the
remaining primitive/reference operation and evidence obligations. The next slice
adds boolean/narrow accesses with their original store-conversion semantics.
