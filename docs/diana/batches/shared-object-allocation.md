# Object allocation in shared flow

## Shared initialization product

This slice migrates existing new-instance/constructor pairs through shared
operations and word definitions. Contracts: [DESIGN](../DESIGN.md) and
[ALGORITHMS](../ALGORITHMS.md).

P2 distinguishes constructor targets from ordinary direct methods and records
new-instance and initialization as separate effect-bearing operations. P3 keeps
the original reference-word identities, move definitions and constructor argument
order. Initialization changes the reference's usable state, not its raw identity.
P4 retains original constructor constraints; source binding is discharged later.

P5 constructs sealed initialization plans and UninitAlias products. The admitted
allocation and constructor stay in one block, with only checked alias moves or
NOPs between them. Effects, escape, mismatched owner, missing/repeated init and
uninitialized returns remain unavailable. This is a per-allocation proof over
shared definitions, not another method register tracker. P6 independently checks
raw allocation/init sites and alias move formats, then seals deferred storage.

`Initializer::{Default, Parameter, Deferred}` separates ordinary DA initialization
from unreadable uninitialized aliases. Deferred Java locals have no null stand-in.
P8 retains deferred allocation/alias events, verifies their ownership, resolves the
constructor and owns argument views. It exposes all aliases only after normal
constructor completion. P9 emits the proved new expression and subsequent alias
assignments; the checked non-observable span permits the original adjacent-pair
fusion. Allocation/invoke identities and original receiver remain in owned calls.

Constructor entry/this-super prologues remain the next migration obligation.

## Reused core evidence

No test, fixture, Driver or combination generator was added. Existing self-made
adjacent-allocation inputs, catalogs and Driver were reused. Both policies'
recovered alias and failure methods compile and match the existing normal/throwing
reference observations, including constructor publication and ordered callback
state. Emitted uninitialized-alias locals are unassigned until the new expression
completes; Java compilation checks their subsequent use.

Existing effect, escape, wrong-owner, uninitialized-return, repeated-init,
bad-argument and branch boundaries retain InitializationUnavailable. The existing
post-init register-overwrite input retains its available Java. No expected values
were refreshed from engine output.

Related fixed `run-azovs90n` retains its passes for D019 and D043; D020/D041/D042
retain their earlier whole-class failures. D043 now uses shared deferred storage
and constructor arguments evaluated before allocation. These remain method-context
evidence, not JavaUnit acceptance.

## Gates

Format, strict Clippy, release and cargo-nextest passed: 122 tests, zero skips.
The staged patch passed those Rust gates in a detached worktree without the frozen
static-field changes, reusing the certified D065 fixture. Diff and sensitive-input
checks passed before submission.

Not run: full required E2E after this slice. Final reacceptance retains that
obligation. The old allocation/constructor paths remain until the migration and
parallel-route deletion tasks establish parity. Artifacts are under
`.cache/e2e/shared-object-allocation/`.
