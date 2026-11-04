# 03 — Exact-slot SSA

Code: `crates/diana-decomp/src/p3/`; construction: Engine's private method task
calls `build.rs::build_ssa`. The approved contract is
[PIPELINE_DESIGN §5.3](../PIPELINE_DESIGN.md#53-p3异常感知的-pruned-ssa-与按需-idf).

P3 consumes an owner-published P2 `FlowMethod` with coverage and local fact states.
It uses only established entry slots, ordered reads/writes, commit rules and
transfers, plus the affected scope of facts P2 could not establish. It follows
the [P1–P9 contract](../PIPELINE_DESIGN.md#92-p1p9-统一消费局部结果).
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
- `OutsideFrame`: the original operand interval is outside the method register window.
- `Unavailable`: a local result/possible write has no established value source,
  with the original operation identity retained.
- `Phi`: a merge of states for that same exact slot.

Inputs are bound before normal outputs commit. A normal write defines its exact
slot and invalidates different tracked slots whose word intervals overlap.
An unavailable possible result instead establishes Unavailable for its destination
and overlapping tracked slots at that normal commit. It creates no ValueId and
does not affect unrelated slots or the pre-operation exceptional frame.
Overlap lookup considers only nearby starts because widths are One/Two. An
exception commits neither the normal result nor those invalidations. A handler's
move-exception result has its own `Exception` value origin.

Construction Pending is separate from Undefined. P3 MUST NOT invent zero/null,
extract a scalar value from a wide word or combine independent scalar values into
a wide ValueId. Values and states have separate method-local identities.

`Availability` retains possible readable, undefined, invalidated and unavailable sources.
Phi availability is the least union grounded in incoming states; cycles do not
create a readable value without an initial or operation source. A mixed merge
remains unproved. `readable_value` returns no ValueId for such a use. These facts
do not assert that an abstract bad path actually executes or classify the entire
method as malformed without path-feasibility evidence.

## Local-source contract

P3 MUST distinguish source identity from value interpretation. An unreadable
reference name, literal content or type interpretation does not erase a source
whose slot, definition and commit are established. Conversely, an unavailable
source MUST NOT be replaced with a fabricated ValueId or initial value.

When P2 reports an uncertain write/commit, P3 MUST preserve source uncertainty
for intersecting tracked slots and dependent bindings, separately from Undefined,
Invalidated and construction Pending. Unaffected slots continue normally. With
known control, an established later write can end that register uncertainty; it
does not repair unknown incoming control or clear heap/lock/exception uncertainty.

P3 MUST NOT omit an unresolved transfer and treat the remaining graph as the full
CFG. Dominance and Phi bindings require either exclusion of unknown extra incoming
paths or explicit propagation of their source uncertainty. A local analysis entry
MUST NOT impersonate the method's initial parameter entry. Normal and exceptional
commit states remain distinct; an unknown rule is not a promise to preserve either.

Published definitions, uses and incoming records MUST carry established sources
or explicit local unavailability with origin/reason. `operation_uses` distinguishes
known zero inputs from an unavailable grouping linked to its P2 operation/cause.
Output bindings refer to states; `output_value` returns a ValueId only when that
state establishes one. It does not keep a second, potentially contradictory value
field. P4 can continue unrelated interpretation without reconstructing registers
or requesting a P3 retry.

Current SSA construction still requires P2's known control graph. P1 stops on
unreliable control/protection associations; no edge is discarded to make LT/IDF
appear applicable. General unknown-control processing is not an implemented
capability merely because data-source unavailability is represented.

## Construction

1. `dominance.rs` computes Lengauer–Tarjan dominators over normal and exceptional
   private transfers under the established-control publication floor above.
   Unresolved incoming influence cannot be omitted to satisfy that floor. DFS,
   union/find compression and tree walks use explicit stacks. Independent predecessor-meet verification is compiled only in tests,
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
from the four base availability categories (readable, undefined, invalidated and
unavailable). Every retained Phi must be in the use
closure. These are test obligations, not production acceptance gates or a formal
whole-P3 proof. Availability propagation and Phi construction themselves remain
normal algorithm work; only their independent verification is test-only.

## Outcomes and inspection

Only the owning constructor with Engine's credential can publish `SsaMethod`.
`SsaStop` retains the complete P2 input, classified error and an unvalidated
`SsaPartial`. A partial snapshot cannot be promoted to `SsaFacts` or `SsaMethod`.
Actual allocation/capacity failures remain ResourceExhausted; construction
invariant failures remain InternalInvariant. Neither kind alone requires a whole
method/class stop. Owner-published `SsaMethod` accounts for the bounded local
source states above while retaining independent bindings; failure to
maintain minimum guarantees or retain results still produces a stage stop.
Correspondence failures exist only in test builds. Current stop behavior and
remaining publication limits are recorded in the engine README.

Use the single `dbg_shell` example with `p3` (`details:true` for records) or
`p3-all`. P2 and byte-roundtrip commands observe products retained by the same
Engine chain; they do not bypass P3 or start a second analysis route. The testing
policy remains the root AGENTS.md scoped exception. The existing core test also
covers SSA joins, loops, entry backedges, irreducible flow, exception-old-state
binding, overlap invalidation and unproved sources; no separate suite or committed
corpus fixture is introduced.
