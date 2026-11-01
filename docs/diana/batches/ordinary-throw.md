# Ordinary throw terminals

Goal: recover an unprotected explicit throw through the shared P2–P9 chain.
Contracts: TASKS T09/T11, DESIGN §4, ALGORITHMS §2.1/§4/§6.1/§9.5.
Preparation reuses the accepted macOS arm64 draft-7 tools and fixed D009/D022
regressions. Independent evidence starts with javac/D8 self-made null, unchecked
and checked rethrow methods; the public Driver observes exception identity and
normal branch results. Protected methods retain their checked SSA/raw boundary.

P3 already binds terminal operands and proves availability. P5 owns the
reference/null terminal use, P6 owns its storage, P7 keeps an abrupt exit without
a normal successor, and P8 checks Throwable hierarchy and checked-throws coverage
before admitting Java. P9 renders only that verified use. No added cast or
exception object changes the original throw.

## Implementation and verification

The ordinary scheduler now carries throw through P5 typed terminal uses, P6
storage and P7 abrupt exits. P8 `validate_bindings` checks original exception
types with `SourceThrows::covers_exception`; the dispatcher verifier independently
checks operand identity, null evidence and absence of normal successors. Both
P9 representations emit that checked terminal. Protected methods still stop at
the existing SSA boundary. All new vectors and hierarchy scans use the current
work/allocation ledger and cancellation points.

Self-made `tests/fixtures/ordinary-throw/ThrowCase.java` and `Driver.java` use the
engine's existing license. Locked Temurin 21.0.8+9 compiles them with
`javac --release 8`; R8 8.3.37 D8 with min-api 21 produces `classes.dex`.
`catalog.json` reuses the certified JDK declarations from D009 and independent
classfile projections for the new source. The public Engine regression checks
null/nominal throws, source coverage and missing Throwable evidence. The existing
pure-loop interpreter explicitly rejects throw rather than modeling exceptions.

Both representation policies compile the actual recovered methods in their
original known class context. All six Driver observations match original JVM
execution, including exception identity, null behavior and the normal branch.
These are method-context observations, not JavaUnit acceptance. The original
projection, compiler logs, references, public results and actual observations
are under `.cache/e2e/ordinary-throw/`.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run --case D009 --case D022 --case D034 --case D055 --case D060 --case D061 --case D064
python3 tools/audit/desensitize_check.py --range HEAD
git diff --check
```

On macOS arm64, Rust gates pass with 131 tests and zero skips. Fixed
`run-sibc__4j` passes all seven selected cases and ten invocations/seams.
The full required rollup is deferred to integration of the remaining reviewed
capabilities; ART and other host platforms remain not-run.

Decision: ordinary null and declaration-proven checked/unchecked throw terminals
are verified through the shared chain. Missing reference evidence remains raw.
