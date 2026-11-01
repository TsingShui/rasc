# Derived array types

Goal: represent nominal and multidimensional array types, including components
without an independent DEX type ID, through the ordinary heap chain. Contracts:
TASKS T15; DESIGN §4; ALGORITHMS §4.3–§4.7. Specification: unchanged draft-7.
Preparation uses the accepted macOS arm64 profile and existing locked tools.

## Producers and invariants

P1 owns checked descriptor projections tied to original type identity. P2 keeps
those declarations in ordinary operations. P5 unions finite component origins
before selecting families; each array-get transfers every origin, not a
provisional unique component. P6 retains the types in locals and copy temporaries
with disjoint opaque identities. P8 verifies source names, allocation dimensions,
original type-check targets and access views. Call results use the same reference
descriptor parser and seed the component closure. P4 retains array return
signatures and validates overload formals without using return types to select a
target. P9 renders the sealed products.

Removing an array prefix does not invent a DEX type ID or default the component
to Object. Reference-array stores use a proven Object[] widening that retains
runtime store checks. Array allocation and type checks do not initialize the
component class. Every new traversal and owned product uses existing resource
and cancellation controls. The implementation adds no decoding or alias tracker.

## Independent input and baseline

The self-made `numeric-fields/Projection.smali` and
`store-reference/fixtures/{Projection,Detached}.java` extend the existing core
fixture. Temurin 21.0.8+9 targets Java8; smali 2.5.2 assembles `projected.dex`.
The existing classfile/layout projectors record original declarations and code.
A manual JShell query through the locked dexlib2 dependency records the type
pool: Detached[][][] exists without Detached, Detached[] or Detached[][] type IDs;
int[][] exists without int[]. The compact declaration catalog retains original
Smali method signatures, including malformed and unsupported members.

The retained pre-projection example returns Debug for `swap` and `stringsStore`.
Additional baseline requests expose Debug for `callRows` and `callFloat` before
the shared call-return correction; the latter requires float[] origin evidence
before aget family selection. A return-only overload collision stays unsupported.
The public regression table exercises both representation policies, derived
component loads, reference-store views, multidimensional copy cycles, explicit
casts/instanceof, missing component declarations, qualifier shadowing and an
invalid wide read. Original Java supplies the successful behavior expectations;
no expected value comes from recovered output.

## Review decision and correction

The operator approved preserving `filledFloats(FF)[F` and its original instructions
as Unsupported rather than emitting a normal float-array initializer. The public
heap table now checks that classified raw result. The faulty Java initializer
and its JVM observation were removed from the supplemental oracle; the original
Smali counterexample and member identity remain. Required Java expectations in
the frozen specification are unchanged.

Source review: [ART interpreter_common.cc](https://android.googlesource.com/platform/art/+/master/runtime/interpreter/interpreter_common.cc),
`DoFilledNewArray`, rejects non-int primitive components with InternalError and
uses unchecked reference stores. Consequently, P5 admits int filled arrays and
reference arguments with proven component assignability; P8 independently checks
those conditions. The draft that introduced checked Object[] stores for unproved
reference-filled arguments was removed. Ordinary aput-object runtime checks remain
in their separate access proof. The Object-argument/String[] filled counterexample
stays Unsupported; a typed String-argument counterpart supplies the positive case.
Direct pinned Android 15 web retrieval was blocked by the fetcher's fake-IP check;
indexed current source is review evidence, not pinned-runtime certification.
ART execution is not run.

## Execution and regression

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64, Cargo gates pass with 125 tests and zero skips. Diff and
sensitive-input checks pass. Both representation policies compile recovered
methods with `javac --release 8` and match the independent Projection, Filled,
Store and Unrelated Drivers: respectively 24, 7, 6 and 4 observations per policy.
These cover copy-cycle identity, nested reads, null/bounds/store failures,
negative allocation size, unused casts and component-class initialization state.
Original member source spans match all 80 supplemental member observations.

Full `run-2s0a_s5e` runs 86 cases/177 invocations/181 seams and exits 1 with
51 cases/117 seams passing. All twenty T15 D024–D028, D073/D074 and D080 rows pass.
No passing seam or Java member availability is lost against `run-q7bywtu6`.
Review checks 2379 original member observations. D050 `$values`, method ID 0,
remains Debug/Unsupported with complete raw code; its reason refines from
JavaAnalysisUnavailable to BranchJavaUnsupported after array decoding reaches
the unsupported enum source-binding check. All other member states and reasons
are unchanged. The failing whole enum case is not counted as passed.

Evidence is under `.cache/e2e/t15-derived-array-types/`: original projections and
type-pool query, baseline/current reports, compiler logs, bytecode, Driver outputs,
`nextest.log`, `full-e2e.log` and `member-comparison.json`. The preceding static
and shared-field evidence remains applicable to unchanged inputs and helpers.
T15's bounded array producer gap and supplemental oracle correction are verified.
