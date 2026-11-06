# 02 — Flow and effects

Code: `crates/diana-decomp/src/p2/`; construction: the Engine method task calls
`build.rs::build_flow` with its private scheduling credential.
The approved product boundary is [PIPELINE_DESIGN §5.2](../PIPELINE_DESIGN.md#52-p2完整方法执行模型-flowmethod).

P2 consumes an owner-published `DecodedMethod`. Under the
[P1–P9 contract](../PIPELINE_DESIGN.md#92-p1p9-统一消费局部结果), completeness means
coverage and local states are accounted for, not that every fact was recovered.
P2 produces ordered operations, terminals, normal and exceptional transfers,
exits, original-item coverage and explicit unavailable execution facts.
P2 MUST NOT decode instructions again, resolve declarations through a program
index, construct SSA or choose Java expressions. Numeric quotas are not used.
Platform capacity and allocation failures retain their classifications.

## Execution obligations

An operation reads all inputs from the pre-operation register state. Its
normal result commits only after successful execution. An exceptional transfer
leaves at its operation or throw terminal, not at a shared block-end state.
Register results are not committed on that transfer. Heap activity performed
by an opaque call or class initialization is not rolled back.

| Original behavior | P2 obligation |
|---|---|
| Numeric constants and copies | Retain original bits and exact One/Two slots; capture a wide source before either overlapping destination word is written. |
| Arithmetic and conversion | Retain numeric domain, operand order, wrapping integer behavior, masked shifts, conversion direction and floating comparison bias. Integer division/remainder can throw; floating division/remainder do not throw for division by zero. |
| Field/array access | Retain object, index and value roles, access direction, element interpretation and resolution/throw obligations. `aput` begins at opcode `0x4b`, not `0x4c`. |
| String/class/handle/type constants | Retain the original reference and resolution site. String resolution can initialize String; class-literal resolution does not imply initialization of the referenced class. |
| Allocation, filled array and fill | Retain allocation or existing-array identity, input order and borrowed payload association. `ExecutionFacts::array_payload` delivers the associated checked element width/count and original bits. P2 MUST NOT expand bulk fill into stores. |
| Invoke | Consume P1's checked receiver/parameter layout; retain dispatch kind and polymorphic prototype. A used result commits at its original move-result site. A discarded result does not remove the call. |
| Monitor | Retain the lock operand and enter/exit direction. Enter acquires on success; exit releases on success. An exceptional exit's lock outcome is not established by its advanced exception-selection PC. |
| Terminal | Retain ordered condition/selector/return/throw reads and the original target/exit roles. |

The [DEX bytecode specification](https://source.android.com/docs/core/runtime/dalvik-bytecode)
is the instruction-semantic basis. In particular, it states overlapping
move-wide reads-before-writes, producer/result adjacency, monitor-exit's advanced
selection PC, array access opcodes and mathematical operation details.
The symbolic numeric domains denote these specified operations: integer add,
subtract and multiply wrap; shift counts are masked to 5/6 bits; division truncates
toward zero and preserves the minimum-value/-1 case; floating arithmetic uses
IEEE rounding and the specified truncating remainder. Conversion nodes distinguish
sign extension, truncation, rounding, saturation and NaN-to-zero by their source
and destination domains. Equality of nodes checks that P2 retained the specified
operation, not that Rust evaluated every possible operand.
The pinned Android 15 ART interpreter is supplementary implementation evidence:
[interpreter_common.h](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/runtime/interpreter/interpreter_common.h)
contains `ResolveString`, integer division/remainder and monitor helpers;
[interpreter_switch_impl-inl.h](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/runtime/interpreter/interpreter_switch_impl-inl.h)
contains field/array access and check-cast handlers. Neither source is a production
dependency or an external declaration source. The checks do not establish ART
admission or account for debugger/VM instrumentation.

## Local execution-fact contract

P2 MUST keep instruction identity and established behavior, reads, writes,
commit rules, effects and transfers independently. One failed interpretation
MUST NOT erase the other established facts or prevent unrelated operations from
being processed. Known facts and an unavailable item together form a published
execution description; an inconsistent construction snapshot does not.

Unknown reads/writes MUST NOT become empty spans. P2 MUST identify the possibly
affected register range and completion paths when precise facts are unavailable.
An unknown target/handler/commit MUST NOT become fallthrough, no handler, no write
or a method exit. Uncertain reachability MUST NOT be labeled unreachable merely
because an unresolved edge was omitted. If no narrower influence is established,
P2 MUST expose the wider uncertainty rather than invent precise bounds.

P3 receives these facts and impact through the execution interface, not raw
opcode access. Heap, lock and initialization uncertainty remains separate from
register state; absence of a recovered effect is not absence of the effect.

`UnresolvedInvoke` retains dispatch kind, original target/prototype identities and
call/heap/initialization/throw obligations when argument grouping is unavailable.
`reads` returns an error, not an empty argument list; `input_words` retains the
ungrouped original invocation words. `InvokeResult` independently distinguishes
Void, a known width and Unavailable. `UnresolvedResult` preserves an invalid
move-result as its own destination site, never a fabricated producer binding.

`writes` retains encoded destination intervals, including possible writes whose
result is unavailable. Consumers MUST check result establishment and register-window
validity before treating them as known definitions. Out-of-frame operands retain
their original intervals and classified issues; they are not valid method variables.

## Test-only translation validation

The previous core test was removed by operator decision; no current test case
invokes these existing checks. `check.rs`, `oracle.rs`, `replay.rs` and their scratch/predicate helpers compile
only under `cfg(test)`. Test builds check constructed products; ordinary debug
and release requests do not run these passes and have no verification switch.
Input parsing, required instruction associations and construction failures remain
part of normal analysis. The test checker:

1. Check arena identities and the complete, non-overlapping block partition.
2. Walk each block's operations in original PC order, including each merged
   producer/result pair and its terminal.
3. Establish a bijection between original instruction sites and their operation,
   merged-result or terminal owners. Payload and unreachable facts remain distinct.
4. Match normal edges by source site and branch/case/default role.
5. Match exceptional edges by original throw site and handler ordinal, including
   selection PC, source operation, catch type and target. Match uncaught/return
   exits by their original site and kind.
6. Reject duplicate or unowned edges/exits and verify entry reachability using
   an indexed adjacency walk.

`oracle.rs` checks complete effect obligations and terminal semantics. It does
not call the builder's opcode translation helpers. `product.rs` owns immutable
consumer records; `transfer.rs` owns explicit normal/exception completion rules.
`evidence.rs` separates original inspection from normal analysis queries.
`replay.rs` constructs two local symbolic descriptions:

- The DEX side derives behavior, captured inputs, expected word writes and
  completion rules from original instructions and P1's checked call layout.
- The P2 side interprets existing operation kind, ordered slots, references,
  `NormalCompletion` and `ExceptionCompletion`. It does not recover a polymorphic
  prototype or a completion rule from original opcode bytes.

The two completion constructors are separate. The DEX constructor is not used
to manufacture the P2 completion. DEX completion derives register patches and
possible heap activity from original instruction obligations; P2 completion
interprets the published destination and completion fields. Shared symbols identify
arbitrary old exact slots, literal bits and opaque event results; they are not SSA
ValueIds.
Every local comparison starts with an arbitrary frame/heap and does not propagate
values, aliases or feasible paths into another operation.

For `move-wide` variants, `wide_copy.rs` replaces generic word-patch equality
with an exact captured-input predicate. The remaining relation fields still
compare independently constructed descriptions. The predicate's Verus contract
establishes acceptance conditions and equality with the old-frame DEX copy rule
for arbitrary word values, including overlap. The proof entry includes the same
executable source used by test replay; ordinary Cargo builds exclude this checker.
The decoded/symbolic-record adapter and whole-method composition remain unproven.
See the [proof contract and reproduction](../../../crates/diana-decomp/proofs/README.md).

Test acceptance: each exercised fully established operation has matching behavior,
ordered arguments, normal register patch and exceptional selection/frame obligations.
Unavailable facts retain their source, reason and affected scope rather than a
fabricated interpretation. Every established transfer has one original owner. Traversals have finite measures and
scratch allocation is fallible. Scratch indexing uses standard in-place unstable
sorting; no request cancellation callback participates in the algorithm.
The transfer index costs O(E log E + X log X) to order, followed by indexed
matching and an O(B + E) reachability walk; it does not scan all edges per block.

## Evidence limits and architecture integration

Symbolic arithmetic nodes retain exact operation/domain distinctions; the checker
does not numerically evaluate all floating-point inputs. Opaque heap/call events
preserve identities and argument order, not callee implementations or concrete
heap outcomes. A successful comparison is relative to these semantic definitions,
not a proof merely because two Rust values compare equal.

`NormalCompletion` delivers reads-before-commit order, normal commit kind/site,
heap outcome and lock acquisition/release. `ExceptionCompletion` delivers the
throw/selection sites, pre-operation frame, heap outcome and lock outcome.
A throwing terminal has its own completion. P3 consumes these facts without
interpreting an opcode. `EventOutcome` preserves possible heap activity, not an
observed write or heap rollback; `ReleaseOutcome` retains exceptional monitor-exit
uncertainty. A verifier MUST NOT manufacture missing downstream facts.

Engine single-method and class requests use the same method task: reader → P1 →
P2. The task alone issues the credential required by the private P2 constructor.
The debug shell observes these outcomes and does not call a stage builder.
`FlowMethod` privately owns P1; its ordinary interface supplies execution facts
and original declarations. `MethodEvidence` supplies restricted borrowed debug/check
facts, with no product extraction, mutable access or construction entry.
`ExecutionFacts` is a separate immutable view over complete P2 execution facts.
Its `catch_type`/`catch_types` queries preserve the method reader's descriptor
results and use issues, keyed by the original logical-DEX-local TypeId used on
exception edges. These queries expose no index or declaration-resolution ability.
Its operation-bound array-payload query borrows P1's checked data without supplying
a raw code view to consumers. Non-fill operations have no array payload; a broken
established association remains an explicit construction error.
P3 privately retains FlowMethod and consumes this view; it cannot extract a
predecessor product or obtain raw evidence through the ordinary execution view.

`FlowMethod` can contain the local data/binding states above while retaining P1
issues. `FlowStop` retains P1, a classified construction error and a partial
snapshot; it MUST NOT be relabeled as `FlowMethod`. Product records remain private,
and consumers do not repair missing facts. A returned method-task failure no longer
terminates siblings by error kind: class result storage is reserved before tasks
start, and each result occupies its original member position. Shared selection or
collector allocation failures still return a class-request error before task work.
Control/protection publication limits are described in the input chapter; P2 does
not manufacture an unknown-edge graph from a stopped P1 snapshot.

Inspect actual products through the single `dbg_shell` example. `p2-all` reports
constructed/failed methods and encountered opcode variants, not verification results. Missing variants
are missing input evidence, not a pass. Follow the root AGENTS.md scoped test
policy; input safety remains in production and correspondence verification stays in tests.
