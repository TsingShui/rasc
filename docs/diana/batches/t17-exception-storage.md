# Exception types and handler storage

Goal: implement T17 in P6 after the operator-approved P4/P5 exception-type
prerequisite. Contracts: DESIGN §4/§3.5; ALGORITHMS §5/§10. P3's checked
exception edges and pre-write Phi inputs feed P4 declaration evidence, P5 types
and P6 local-only snapshots and handler ingress. P7/P8 consume the placement
obligations; protected Java acceptance belongs to T18.

Preparation reuses accepted B01–B04 on macOS arm64, draft-7, the fixed D029–D035
inputs and the independent `throw-state` fixtures described in
`t16-throw-state.md`. T08's copy replay supplies the existing exact-type parallel
copy verifier. Each handler/Phi owns a separate shadow; every exceptional input
must have a same-site, pre-throw, defined, initialized and exact-type source.

Baseline inspection: `pipeline/branch.rs` stops protected methods after P3;
`p5_types/normal.rs` rejects ExceptionAt. The operator approved completing those
producer obligations before P6 implementation. Missing declaration evidence
remains Unsupported, never an invented exception type.

## Products and independent evidence

P4 `exceptions::collect` retains every handler's original exception operation and
checks public, nongeneric catch declarations and their concrete superclass chain
to Throwable. The check permits Throwable's Serializable interface without
relaxing ordinary method-overload evidence. Missing declarations, cycles and a
non-Throwable parent prevent typing. A shared handler with different incoming
catch types uses Throwable for the exception value only; the ordered catch edges
retain their original types and ranks.

P5 `normal::solve` gives ExceptionAt its evidenced nominal or Throwable type.
P6 `exceptional::{build,verify}` allocates exclusive handler/Phi shadows, records
each exceptional edge's source and before-throw position, and invokes the existing
parallel-copy producer and replay for ingress. The verifier independently checks
handler coverage, exception binding, shadow ownership, every exceptional Phi
input, source materialization before the site, exact types and copy coverage.
An unproved or deferred source stops storage admission as Unsupported; a
published plan with that source fails verification as InternalInvariant.
Snapshots contain only local
copies, so casts, calls, allocation and heap writes cannot appear in them.

P7 records a Protected shape instead of selecting an ordinary natural structure
or dispatcher for a protected graph. P8 `exceptional::obligations` produces
before-throw and catch-ingress placements, rechecks storage and matches original
protection targets/ranks. The existing classified raw exit retains the original
member identities and diagnostic codes while protected source is unavailable.

The fixed D030 DEX and declaration subset come from its already-certified
preparation artifacts. The core test checks the two old literals against the
independent source contract. Existing throw-state members cover same-register
results, typed/catch-all ordering, shared targets, an outer-protected handler
throw and wide old state. Supplemental `exception-storage/Case.smali` adds a
loop with a protected call and two retained same-type handler values. Its Java
reference and Driver compile with the locked Temurin 21.0.8+9 using `--release 8`;
smali 2.5.2 assembles the 035 input. The independent dexlib2 projection records
its instruction sites and try table. Source-JVM observations cover loop counts
0, 1 and 3, each with successful and throwing calls, plus both two-value paths.
They certify the reference derivation, not execution of handwritten DEX on ART.
Fixtures use the engine's existing license.

The supplemental catalog reuses the throw-state declarations, the new reference
classfile and Java 8 ct.sym declarations for Throwable, Exception,
RuntimeException and IllegalArgumentException. The D030 catalog retains its
complete source declarations and required JDK superclass directories.

## Execution and remaining obligations

```sh
java -cp '.cache/e2e/tools/downloads/*' org.jf.smali.Main assemble crates/diana-decomp/tests/fixtures/exception-storage/Case.smali -o crates/diana-decomp/tests/fixtures/exception-storage/classes.dex
javac --release 8 -d .cache/e2e/t17-exception-storage/reference crates/diana-decomp/tests/fixtures/throw-state/fixtures/ThrowDep.java crates/diana-decomp/tests/fixtures/throw-state/fixtures/ThrowError.java crates/diana-decomp/tests/fixtures/exception-storage/fixtures/ShadowState.java crates/diana-decomp/tests/fixtures/exception-storage/Driver.java
java -cp .cache/e2e/t17-exception-storage/reference Driver
java -cp '.cache/e2e/projector/classes:.cache/e2e/tools/downloads/*' ProjectDex crates/diana-decomp/tests/fixtures/exception-storage/classes.dex
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run -p diana-decomp --all-features --locked t17_
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

Java commands use the locked JDK under
`.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home/bin/`.
On macOS arm64, Cargo gates pass with 137 tests and zero skips; the targeted T17
selection runs five tests, including the extended public Engine regression with
both incomplete and complete catch declarations. Eleven plan mutations reject
post-site placement, normal-result substitution even when the Phi input is also
changed, a type-changing copy, aliased shadows, missing snapshots/ingress,
foreign, Undefined or deferred sources, wrong source blocks and lost exception
binding.
Producer and independent replay both reject configured work/allocation limits
and cancellation. Catch typing rejects missing, cyclic, non-Throwable and generic
evidence.

Full `run-dvwip56y` executes the unchanged draft-7 denominator and exits 1:
51 cases and 117 seams pass. Against the recorded constructor-argument baseline
`run-r0ex1_dw`, all seam outcomes and all 2379 original member observations retain
their status, Java availability, classified failure and complete Debug source.
The earlier T16 baseline predates the accepted D050 constructor diagnostic change;
it is not used to attribute that existing change to T17.

Evidence under `.cache/e2e/t17-exception-storage/` includes original DEX and
classfile projections, reference classfiles/observations, `nextest.log`,
`full-e2e.log` and `member-comparison.json`. Diff and sensitive-input checks pass.
T17's plan and certificate checks pass. T18 still needs to consume the placement
obligations in actual try/catch construction and check the emitted catch stack;
B(F16,F17), required protected Java and ART execution remain incomplete.
