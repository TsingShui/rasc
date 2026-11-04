# 03 — Exact-slot SSA

Code: `crates/diana-decomp/src/p3/`; construction: Engine's private method task
calls `build.rs::build_ssa`. The approved contract is
[PIPELINE_DESIGN §5.3](../PIPELINE_DESIGN.md#53-p3异常感知的-pruned-ssa-与按需-idf).

P3 consumes a complete P2 `FlowMethod`. It uses only entry slots, ordered
operation/terminal reads and writes, commit rules and original transfers.
P3 MUST NOT decode instructions, resolve declarations, infer handlers or perform
P4 type analysis. `SsaMethod` privately owns P2 and exposes `SsaFacts`, borrowed
`ExecutionFacts` and separate original `MethodEvidence`. Neither borrowed view
extracts a predecessor product or grants analysis construction authority.

## State positions and original mapping

`graph.rs` creates private positions for initial entry, block entry, operation
execution, normal commit and terminals. Initial entry has its own transfer;
a backedge into the original first block does not reuse initial parameter values.
A normal output commits after its operation's reads. An exceptional transfer
leaves the execution position, before normal definitions and their invalidations.
A used invoke result's commit record retains P2's `AfterResult` rule and original
commit PC. No new executable operation or heap effect is introduced.

Every transfer retains its original edge, return, normal-commit or continuation
role. Equal source/target pairs keep distinct transfer identities. `OperationSite`
and `TerminalSite` provide original ownership, state positions and binding
spans. Phi incoming records refer to `TransferId`; consumers can obtain the
original transfer mapping without invoking a P3 analysis helper.

## Fixed slot states

Tracked slots are the exact `(start, One|Two)` keys present in P2 reads, writes
and entry parameters. They are sorted and fixed before dominance or IDF work.
A slot's definition is one of:

- `Value`: an exact-slot parameter, this, operation result or exception object.
- `Undefined`: no exact-slot initial parameter or definition.
- `Invalidated`: a recorded overlapping normal write destroyed the intact slot.
- `Phi`: a merge of states for that same exact slot.

Inputs are bound before normal outputs commit. A normal write defines its exact
slot and invalidates different tracked slots whose word intervals overlap.
Overlap lookup considers only nearby starts because widths are One/Two. An
exception commits neither the normal result nor those invalidations. A handler's
move-exception result has its own `Exception` value origin.

Construction Pending is separate from Undefined. P3 MUST NOT invent zero/null,
extract a scalar value from a wide word or combine independent scalar values into
a wide ValueId. Values and states have separate method-local identities.

`Availability` retains possible readable, undefined and invalidated sources.
Phi availability is the least union grounded in incoming states; cycles do not
create a readable value without an initial or operation source. A mixed merge
remains unproved. `readable_value` returns no ValueId for such a use. These facts
do not assert that an abstract bad path actually executes or classify the entire
method as malformed without path-feasibility evidence.

## Construction

1. `dominance.rs` computes Lengauer–Tarjan dominators over normal and exceptional
   private transfers. DFS, union/find compression and tree walks use explicit
   stacks. Independent predecessor-meet verification is compiled only in tests,
   separate from LT's semi-dominator scratch.
2. `rename.rs` computes backward live demand per affected slot. Normal writes and
   overlap invalidations both kill preceding state versions. Entry-only states
   do not require IDF expansion.
3. Demand-driven IDF walks dominator subtrees from definition positions in depth
   order. Live-in demand prunes placement; no complete dominance-frontier table
   or register-by-edge matrix is constructed.
4. Dominator-tree renaming uses exact-slot state stacks. It binds every ordered
   use and fills each incoming transfer separately.
5. Trivial state Phis are aliased internally, and only the actual-use Phi closure
   is retained. Identities are compacted before publication, not changed by P4.
6. Finite source-category propagation establishes availability.

Scratch allocation is fallible. Per-slot demand and IDF traversal can revisit
graph positions for different slots; this is not a claim
that the whole method pass is linear. There are no numeric resource quotas.

## Test-only correspondence checks

`check.rs` and the independent dominator verifier compile only under `cfg(test)`.
Neither runs on ordinary Engine requests; there is no runtime verification mode.
`check.rs` verifies position ownership and the exact normal/exception transfer
multiset against P2, including commits, first entry and returns. It checks the
fixed slot set, entry/normal/exception definitions, complete overlap
invalidations, original use/output spans and Phi incoming bijections.

Binding checks walk backwards through the private graph until a matching Phi or
original state definition is reached. Every visited path must agree with the
published exact-slot source. A sparse claim cache detects conflicting sources
without manufacturing missing bindings. This checker does not reuse the
renamer's version stacks. Availability is checked independently by reachability
from the three base source categories. Every retained Phi must be in the use
closure. These are test obligations, not production acceptance gates or a formal
whole-P3 proof. Availability propagation and Phi construction themselves remain
normal algorithm work; only their independent verification is test-only.

## Outcomes and inspection

Only the owning constructor with Engine's credential can publish `SsaMethod`.
`SsaStop` retains the complete P2 input, classified error and an unvalidated
`SsaPartial`. A partial snapshot cannot be promoted to `SsaFacts` or `SsaMethod`.
Actual allocation/capacity failures remain ResourceExhausted; construction
invariant failures remain InternalInvariant. Correspondence failures exist only
in test builds. Class termination retains delivered
members and an explicit termination kind.

Use the single `dbg_shell` example with `p3` (`details:true` for records) or
`p3-all`. P2 and byte-roundtrip commands observe products retained by the same
Engine chain; they do not bypass P3 or start a second analysis route. The testing
policy remains the root AGENTS.md scoped exception. The existing core test also
covers SSA joins, loops, entry backedges, irreducible flow, exception-old-state
binding, overlap invalidation and unproved sources; no separate suite or committed
corpus fixture is introduced.
