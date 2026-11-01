# Instance fields in shared flow

## Slice and implementation

This slice migrates existing int instance reads/writes into the shared operation
arena. Contracts: [DESIGN](../DESIGN.md) and [ALGORITHMS](../ALGORITHMS.md).

- P1 retains the original field-owner TypeId. P2 records FieldId, registers, PC,
  heap effects and throwing position. P3 supplies ordered uses and OpResult.
- P4 collects original owner constraints and declaration evidence. Missing
  binding or constant-read evidence remains explicit through type analysis.
  `field_facts.rs` extracts the existing access/flags/native-declaration/name/
  constantness checks; the committed legacy instance adapter delegates to it.
- P5 checks the nominal receiver and int-word value, retaining BooleanWord
  conversion where necessary. P6 allocates storage through the existing arena.
- P8 resolves field spelling and independently checks raw opcode, registers,
  reference, receiver type, result/value, usage and flags. P9 emits materialized
  loads/stores, including unused reads, at their original operation positions.

The shared receiver profile requires the original owner TypeId exactly. Receiver
calls, constructors, casts and fills still use temporary routes described in
[the baseline](architecture-baseline.md). The heap route remains during migration.
The frozen static-field decoder, renderer and API changes remain uncommitted;
only the reviewed common metadata extraction and original instance adapter are
included in this slice. The task for calls and fields remains open.

## Independent core evidence

`crates/diana-decomp/tests/fixtures/shared-fields/` contains authored Case, Dep,
Failure and Driver sources, declaration facts and a 1844-byte DEX. Box.java is
reused from `../shared-nominal/`. The recipe uses prepared Temurin 21.0.8+9
`javac --release 8`, then R8 8.3.37 D8 with `--debug --min-api 23 --lib <JDK>`
on Case.class, Box.class, Dep.class and Failure.class. Driver runs on the JVM.
No expected values are derived from Diana output.

The public Engine regression covers both representation policies, field/call/
array composition and native volatile metadata overriding missing or contradicted
catalog facts. It reuses the existing declaration-fixture schema.

For each policy, the original reference program and emitted methods match all
78 Driver observations. The inputs cover null receivers/arrays, empty arrays,
negative/out-of-range indices, zero/multiple loop iterations and call failure.
Dep mutates the receiver before returning or throwing, exposing reordered reads
and writes. Examples include an array bounds failure after two completed writes
and a call failure after one completed write. Captured array/field state and call
trace match the reference. The emitted unused method retains getfield before
arraylength, confirmed with javap. These are supplemental method-context JVM
observations, not additional fixed passes or JavaUnit acceptance.

The initial library used a nested Failure class and was rejected by the existing
plain-declaration call profile. Its input/source remain in the batch cache.
The frozen fixture uses a separate top-level Failure class with the same intended
throw/effect behavior; nested-declaration support was not changed.

## Executed gates

- `cargo fmt --all -- --check`, strict workspace/all-target/all-feature Clippy,
  release workspace build and cargo-nextest: passed; 122 core tests, zero skips.
- Full fixed E2E: `run-m7z0ts4o`, overall failed; 51 cases/117 seams passed out
  of 86 cases/177 invocations/181 seams. No lost or gained pass relative to
  `run-zafjrdz3`; unresolved failures retain their actual status.
- Dispatch-only related E2E: `run-f99abre0`, 9 cases/21 invocations/21 seams
  passed (D006, D018, D019, D024, D026, D028, D071, D077, D080).
- The staged production/test patch was applied to detached a332f93 without the
  frozen static-field changes. Format, Clippy, release and all 122 core tests
  passed there. The existing certified D065 fixture was reused.
- Diff and sensitive-input checks passed before submission.

Logs, emitted methods, JVM observations, bytecode inspection and the isolation
patch are retained under `.cache/e2e/shared-instance-fields/`.
