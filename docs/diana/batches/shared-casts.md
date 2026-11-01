# Casts in shared flow

## Slice

This slice migrates existing explicit Object/String check-casts into the shared
operation arena, typed uses and storage. Contracts: [DESIGN](../DESIGN.md) and
[ALGORITHMS](../ALGORITHMS.md).

P2 retains each original type reference in a cast arena and decodes check-cast
as an operation that reads and defines the same DEX register. P3 creates a new
OpResult instead of changing the old value's identity. P5 fixes the result type
and seals source-use conversions. Reference values use identity or a proven
Object view; original zero uses typed null. Known non-reference inputs retain
a classified Malformed failure. P6 allocates through shared storage.

P8 checks the original target descriptor and namespace evidence, then independently
checks opcode, register, TypeId, result, source definition and use conversion.
P9 writes only the verified target cast and source view. Unused casts remain
materialized operations in original order. Casts participate in common CFG,
SSA, storage, effect and representation consumers, not a method-shape builder.

## Core evidence and checks

No test, fixture, Driver, comparator or validation framework was added. Existing
prepared D025 and D071 inputs and their independent expected values were reused.
The observed D025 method uses shared storage and retains its unused String cast.

- `run-mdzby_en`: D025/D071, 2 cases/10 invocations/10 seams passed.
- Dispatch-only `run-8p7gh6lb`: D025, 1 case/3 invocations/3 seams passed.
- Format, strict workspace/all-target/all-feature Clippy, release build and
  cargo-nextest passed: 122 tests, zero skips.
- The staged patch passed those Rust gates in a detached worktree without the
  frozen static-field changes, reusing the certified D065 fixture.
- Diff and sensitive-input checks passed before submission.

Not run: full required E2E after this slice; final reacceptance retains that
obligation. Allocation/constructor and fill proofs remain in the current task.
The legacy cast tracker remains until task 6 removes migrated parallel routes.

Artifacts are under `.cache/e2e/shared-casts/`.
