# Java decline diagnostics

P2 flow decoding and P8 Java verification now attach the producing phase and the
failed check to their generic Java declines. P9 and application JSON consume those
facts without further analysis. Existing diagnostic codes, failure kinds, member
identities and recovery decisions remain unchanged.

P2 records return/parameter/field/cast projections, array component selection,
invoke result types, unsupported opcodes, reachable payloads and exception-edge
construction. Instruction-local declines include their original code-unit offset;
method-wide checks leave it absent.

P8 distinguishes capture context, protected-region planning, array operations,
fresh fills, casts, call source binding, storage type names and final Java
lowering/verification. These checks currently return method-wide failures, so no
instruction position is invented. Final lowering/verification remains a combined
check until its producer supplies a more specific reason.

The existing public array-component regression in
`crates/diana-decomp/tests/pipeline/ssa.rs` checks that a missing component retains
its original failure category and identifies P8 storage type naming. A small
real-application inspection observed separate P8 call binding, cast binding and
storage type-name declines previously presented as `BranchJavaUnsupported`.

Run `cargo nextest run --workspace --all-features --locked` and the workspace
format, Clippy and release-build gates for this change.
