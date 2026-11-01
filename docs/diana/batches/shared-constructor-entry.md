# Constructor entry in shared flow

## Shared entry and first-statement proof

This slice migrates UninitThis, this/super chaining, pure prologue arguments and
post-chain own-field writes through shared SSA/storage. Contracts:
[DESIGN](../DESIGN.md) and [ALGORITHMS](../ALGORITHMS.md).

P2 distinguishes constructor frames. P3 keeps the original receiver/parameter
word definitions. P5's initialization product now distinguishes heap allocation
from UninitThis. Its chain producer consumes entry Phi/alias definitions, admits
only the existing pure int parameter/literal prologue and requires an initialization
call before entry transfer. An entry backedge, effect, field use, monitor or
unproved receiver before chaining remains unavailable.

P6 independently checks raw chaining, alias formats and argument origins. The
storage product distinguishes ThisAfterChain from ordinary parameters/defaults
and deferred heap references. P8 discharges constructor binding and the existing
this-chain cycle proof, owns a zero-or-one prologue and verifies its original
call/argument identity. Both representations consume this same storage. P9 writes
the verified this/super call before any local declaration, then exposes initialized
this aliases and emits the original body operations. No register tracker or
constructor method template is added.

Own int writes reuse the reviewed native identity/name/flags checker. P5 requires
this provenance; private field and volatile semantics stay explicit. Static/final
or mismatched field evidence retains rejection. The frozen static-field decoder
and renderer remain uncommitted.

## Reused independent core evidence

No test, fixture, Driver or combination generator was added. Existing self-made
super-argument, private this-chain and post-chain-field inputs were reused. For
both policies, each unchanged Driver's normal and throwing observation matches
the reference: six observations per policy across the three existing contexts.
Field values, constructor publication and ordered callbacks match. Java compilation
checks that this/super is first and that explicit parameter origins are legal.
For the private chain, the unsupported terminal constructor body remains original
context, not newly recovered Java or JavaUnit acceptance.

Existing branch/effect/field/monitor/String/write/duplicate-init/wrong-receiver
prologue boundaries retain InitializationUnavailable. Self/cyclic/unproven this
chains retain InitializationUnavailable. Missing or contradicted field facts
retain BindingEvidenceMissing. D042's unsupported int-constructor body remains
Debug as before; available constructor methods now use shared storage.

## Gates and task boundary

Format, strict Clippy, release and cargo-nextest passed: 122 tests, zero skips.
The staged patch passed the same Rust gates in a detached worktree without the
frozen static-field changes, reusing the certified D065 fixture. Diff and
sensitive-input checks passed before submission.

Full required `run-m2kvanok` remains overall failed: 51 cases/117 seams passed
out of 86 cases/177 invocations/181 seams. No lost or gained pass against
`run-pi4safz6`. Related `run-ql2ii9va` retains the D028/D043 passes and earlier
D020/D021/D041/D042 whole-class failures.

The migration slices now cover the existing cast, heap/constructor initialization,
empty-fill and FreshArray producers. Legacy route deletion, broader shared scalar
vocabulary parity and the unified raw exit remain task 6 obligations, not claimed
complete by this task. Artifacts are under `.cache/e2e/shared-constructor-entry/`.
