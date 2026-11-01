# FreshArray in shared flow

## Shared producers and proof

This slice migrates primitive new-array and nonempty payload fills through shared
operations, SSA, types and storage. Contracts: [DESIGN](../DESIGN.md) and
[ALGORITHMS](../ALGORITHMS.md).

P2 retains allocation TypeId/component, length register, fill payload range/count
and effect positions. P3 defines each allocated array as an OpResult and retains
ordered uses. P5 seals length conversions and primitive payload-width facts.
Unknown source-call formals remain explicit pending evidence rather than failing
before the later FreshArray obligation can classify an escaping array.

P6 constructs FreshArray from actual shared definitions. It resolves move aliases
to one new-array definition and its constant length, checks length/count coverage
and requires allocation/fill in the same block with no intervening observable
operation. This conservative no-escape span replaces the old method-wide alias
tracker for the admitted profile. Raw allocation, constant and payload facts are
checked before exact element certificates are created. Invalid boolean payloads
and unrepresentable NaN payloads retain ArrayFillShape.

P8 independently checks allocation/result/length uses and exact payload bits
against the owned literal sequence. It preserves the separate allocation and fill
operations. P9 emits new-array at its original allocation position, then ordered
stores at the fill position. The proof establishes nonnull and sufficient length;
no runtime bounds helper or class-initialization helper is introduced. Source
views and array returns use the existing storage/type consumers.

## Existing independent evidence

No test, fixture, Driver, comparator or combination generator was added. The
cached self-made FreshArray input and Driver were reused. Both representations
match nine existing observations: eight primitive components and an alias case.
The Driver compares raw floating bits, signed zero, character units and integer
boundary values. Boolean/NaN source-representation restrictions and malformed
result-kind classification retain their existing outcomes.

Related fixed `run-xt94tggu` passes all D027/D073/D074 invocations: 3 cases,
8 invocations and 8 seams. D074's parameter, short, unknown-length and escaped
inputs retain ArrayFillShape. During migration, escaped input briefly reported
BindingEvidenceMissing before the FreshArray proof; deferring unsupported call
formal evidence restored the original classification without changing expected
values. Array-return certificate coverage was corrected before this passing run.

## Gates and remaining task

Format, strict Clippy, release and cargo-nextest passed: 122 tests, zero skips.
The staged patch passed the same Rust gates in a detached worktree without the
frozen static-field changes, reusing the certified D065 fixture. Diff and
sensitive-input checks passed before submission.

Not run: full required E2E after this slice. The latest full report remains
`run-pi4safz6`; final reacceptance retains the complete fixed-set obligation.
Allocation/constructor initialization remains in the current task. The old
FreshArray path stays until task 6 removes migrated parallel trackers.

Artifacts are under `.cache/e2e/shared-fresh-array/`.
