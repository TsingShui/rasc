# P6 control, protection and monitor plans

[PIPELINE_DESIGN §5.6](../PIPELINE_DESIGN.md#56-p6自然区域优先保留显式转移的-java-like-控制计划)
defines the product obligations. This chapter describes their implementation in
`crates/diana-decomp/src/p6/`. Capability observations belong in the
[engine README](../../../crates/diana-decomp/README.md).

## Boundary and failure behavior

Engine's method task calls `build_regions(task, storage)` once. P6 consumes a
mature `StorageMethod`, privately retains it, and publishes `PlannedMethod` or
`RegionStop`. Only the owning constructor has publication authority.

`PlannedMethod::facts()` supplies `RegionFacts`. `storage_facts()`, `type_facts()`
and `execution()` provide the established facts needed by P7. Borrowed
`storage_evidence()` and `analysis_evidence()` support inspection without extracting
an owned predecessor or exposing another analysis entry. P6 MUST NOT use the
retained SSA inspection view to reconstruct missing storage facts.

The P5 execution graph has known operation boundaries, terminals and targets.
Local storage gaps do not invalidate that graph. P6 records affected accesses and
continues planning independent regions. A `Graph` region denotes known explicit
control, not an invented target for unknown control. P6 MUST NOT replace an
unavailable operand with a default, omit an effect, or re-run an earlier solver.

A region mismatch rejects that composition, not the method. Resource failures and
necessary construction-invariant failures produce a classified `RegionStop` with
the complete P5 product and a separate, unpublished `RegionPartial`. The partial
arena is not `RegionFacts`. A method-private stop does not cancel sibling tasks.

## Planning state and published diagnostics

During construction, P6's private issue accumulator includes `PlanningIssue::Graph`
records for candidate rejection and graph selection. P6 consumes those records to
select irreducible graph reasons and explicit lock-separation reasons. P6 MUST
finish these computations before finalizing the published diagnostic set.

Before publishing `PlannedMethod`, P6 MUST remove all `PlanningIssue::Graph`
records from `RegionFacts::issues()`. Each surviving `RegionKind::Graph` retains
its selected reason and entry; its printed `graph<reason>` header states that
reason once. This includes `Unstructured`, `MultipleEntry`, `Irreducible` and
`ProtectionBoundary` when selected, and any other selected `RecoveryReason`.
Rejected-candidate reasons, including `MultipleEntry`, `Overlap` and `LockBoundary`
at entries retained as non-graph regions, are planning state rather than
reader-facing execution gaps. P6 MUST NOT publish them as diagnostics or header
annotations. The finalization changes no region, node, label or selected reason.

P6 MUST retain `Storage`, `Lock` and `Protection` diagnostics in their original
order and with their original identities and content. A stopped construction keeps
its unchanged unpublished `RegionPartial`, including its planning records; it is
not subject to successful-product diagnostic finalization. P7 consumes the mature
diagnostic set without reconstructing candidate history or matching issue entries
to regions.

## Original blocks and normal structure

Each original P2 block has one `BlockPlan`. Its ordered `Node` portions partition
the original operations and terminal without duplication. Portions isolate monitor
operations, terminal evaluation and changes of exception protection. A
`WithinBlock` link is a sequencing relation between those portions; it is not a
new DEX edge, effect, instruction or SSA definition. Original normal edges retain
their `EdgeId` and connect the original block's final portion to the target's
first portion. Reachability includes the original exceptional transfers.

P6 builds private normal incoming/outgoing indexes. Iterative Lengauer–Tarjan
computes normal dominators, with analysis-only roots for the method entry,
handlers and otherwise disconnected components. Reverse LT supplies finite-exit
postdominator candidates. A position with no path to an exit has no such candidate.
P6 MUST retain every original divergent path when checking a candidate; virtual
analysis roots do not become runtime transfers. Iterative SCC walks and
header-dominated backedges establish natural loops. An SCC alone does not
establish a natural loop.

The region arena contains `Block`, `Sequence`, `If`, `Loop`, original `Switch`,
`LabelledBlock`, `Try`, `Synchronized` and explicit `Graph` nodes. Containment-sized
candidates combine previously owned content. External entrances, disjoint child
ownership, protection boundaries and P5 construction-group membership constrain
composition. Crossing candidates remain explicit rather than duplicating their
shared content. Empty branch arms contain no fabricated instruction.

Private parts carry entry, continuation and membership constraints. Parent paths
supply nested loop and scope context when P6 chooses transfers. All graph walks,
ownership composition and context walks use explicit stacks or finite queues.
Candidate frontiers are derived from the input; a mismatch does not restart method
analysis or launch an unbounded template search. Scratch and result allocation are
fallible and checked, without a configured numeric quota.

## Loops, switches and continuations

`LoopForm` is `While(LoopCondition)`, `DoWhile(LoopCondition)` or `Infinite`.
`LoopCondition` identifies the original predicate node, repeating link and exiting
link. Its polarity is preserved by those original edge roles. The body and the
syntax-owned predicate have separate execution ownership.

A header predicate becomes `while` only when its original evaluation position and
an actual exiting edge establish that candidate. An internal header branch does
not suppress a valid tail candidate. P6 tries the tail when the header candidate
has not succeeded. Preceding producer operations remain in the loop body of
`while(true)` when lifting them would move or repeat execution. A tail predicate
remains at the tail. `continue` in a do-while targets that predicate, not the next
body execution. Traditional for, enhanced for and for-each are not region forms.

A conditional loop's normal continuation is the actual destination of its exiting
predicate link. It is not a common postdominator beyond exit-side work. Other loop
exits retain their own targets. A non-terminating region does not acquire a
synthetic exit to make postdomination convenient. `LoopPlan::members()` retains
the backedge core. A loop region can additionally own closed synchronized scopes
and their release/break corridors outside that core. Candidate acceptance still
checks the enlarged region's original entrances and unique content ownership.

Original switch case keys, default, roles and duplicate-target edges remain distinct.
A switch body can contain a local graph with multiple case entrances and
fallthrough. The selector has one execution owner; recovery does not introduce a
state variable, generated dispatcher or repeated selector evaluation.

`Continuation` records whether a completed region finishes its parent, reaches a
confirmed next position, or needs an explicit label transfer. P7 MUST consume this
choice rather than infer fallthrough from source layout. Public follow content has
one owner, not a separate copy in each arm.

## Transfers, labels and P5 obligations

Every original edge has an `EdgePlan`. Each normal `LinkPlan` chooses a concrete
`Goto`, `Next`, arm, loop predicate, `Break`, `Continue` or synchronized-boundary
action. Return and throw retain their original terminal identity; P6 does not
invent normal edges for them.

`LabelId` binds a node entrance, original `BlockId`, entry kind and leaf ownership.
Original block entries, handler entries and within-block continuation labels remain
distinct. An original jump still targets the original block entrance. Continuation
labels only describe the planned sequencing of portions of that same block.
A handler label does not turn an exception arrival into a normal goto.

P6 MUST select the original branch before executing that edge's P5 copies. Normal
link copies refer to the original `NormalEdge` site. `CopyPlacement` retains every
other P5 location, including initial entry, normal commit, before-operation,
before-terminal, before-throw and handler-only receiving positions. Assignment gaps
and unavailable copy components remain P5 facts; a plan ID is not evidence that
all of its assignments are available.

`CopyPlacement::owner` selects original emission, a P5 construction group or a
synchronized outlet. Outlet-owned plans retain their P5 locations and appear in
that outlet's before/after-release references, not also in ordinary link emission.
P7 MUST consume that ownership and phase order without reordering assignments.
`SynchronizedReleased` identifies the particular original release and its link;
the outlet separately records any syntax-owned routing to the final continuation.

`OperationOwner` and `TerminalOwner` define one emission partition. A natural
predicate, catch capture, synchronized operation or P5 construction member MUST
NOT also execute through an ordinary block lowering. P6 retains all original
facts even when natural syntax represents an event. Selected P5 construction
groups stay atomic across implicit control/protection scopes. Their deferred
publication, operation/copy emission and snapshot prelude remain P5 decisions.

## Protection and catch ownership

Every original throwing operation and throw terminal has a `ThrowSiteId`, its
original completion, ordered handler chain, original edges and before-throw snapshot
references. Throw and selection PCs remain the P2 facts. Handler entries retain the
original exception capture and P5 receives after that capture. Missing catch type
content is not a reason to remove its original identity or priority.

A natural `TryPlan` records a clause range in an original chain and disjoint body
and handler content. Catch ownership follows the ordered clause range, not target
address equality. A shared target does not merge different catch roles. Normal or
sibling-catch entrances into a natural catch body reject that wrapping. Shared
handler tails require a common continuation or explicit representation.

Proved enclosing synchronization supplies a bounded protection context containing
the actual held graph and outlet anchors. Cleanup handlers remain ordinary
protected content: P6 does not remove their clauses, captures, operations or P5
receives in favor of an implicit cleanup template. Ordered prefix and suffix
clauses remain unchanged. Candidate composition prefers the full catch frontier
when equal memberships would otherwise let a suffix obscure it.

Follow candidates start after guarded nodes. Within a synchronized context,
handlers that complete through abrupt outlets do not constrain a shared normal
follow; guarded nodes' own normal successors still constrain it. Exception-closed
body/handler membership and entrance checks retain nested cleanup protection and
secondary exceptions. A proved `ImpossibleRelease` supplies no executable
exceptional entrance, but its original completion, chain, edges and snapshots
remain in the product.

A site does not execute both an explicit and a natural capture plan. If selected
candidates cannot account for its original handler choices in one model, P6 expands
the affected natural protection nodes into explicit graphs before publication.
This changes plans, not original execution facts, and preserves independent regions.
P7 receives either natural ownership or the complete site-specific chain; it does
not infer a handler relation from printing position. Catch-all is not automatically
finally.

## Monitor capture and scope proof

Lock analysis describes method-local acquisition obligations under P2's completion
contract, not a claim about all locks held by the calling thread. An untracked
acquire or an uncertain join remains a concrete `LockGap`, not proof of malformed
monitor use. Normal acquisition, normal release and exceptional release outcomes
remain distinct.

A persistent, method-private stack and work queue propagate those obligations.
Repeating one acquisition site before its release becomes uncertainty rather than
an unbounded stack. Captured-reference stability consumes P4 identities and P5
storage reads/writes. Before/after positions distinguish a change before the exit
operand read from one after release. A shared static root or an allocation-source
set alone does not establish the same dynamic object: relevant writes or repeated
births in the capture window defeat that proof. This is not another register or
alias environment.

Under the P2 lock-completion model, a successful acquire supplies a non-null,
current-thread-owned capture. A matching stable release with established ownership
has a local `ImpossibleRelease` exception disposition. P6 retains its raw P2
completion and all original exception edges; the disposition is not a general
callee-behavior or VM-admission proof. An unresolved lock relationship retains
explicit monitor operations and the original exceptional outcome.

`MonitorPlan` records `Natural(SyncId)` or `Explicit(LockSeparation)` for each
acquisition. Scope construction follows all executable normal and exceptional
held paths. Each path stays held, diverges, or reaches its own stable matching
release. A return or uncaught exception while still held rejects naturalization.
A closed divergent body needs neither an invented release nor an invented exit.
Acquisition failure does not enter the held body. Nested acquisitions remain
distinct obligations even when they capture the same object.

`SyncPlan::held_nodes()` records original held execution. `body()` additionally
contains syntax-owned outlet anchors, not unlocked operations moved under the
lock. Each `SyncExit` records its original release/link/target, routing, P5 plans
before and after release, and continuation. Multiple releases are supported.
A scope has at most one shared normal follow; other established outlets are
operand-free returns, enclosing-loop break/continue, or rethrows of existing
exception values. Loop outlets require the selected enclosing loop, not merely
a loop analysis candidate. A rethrow retains the exact original P5 read. Its
capture identifies provenance, not permission to substitute the current handler
exception or the capture local.

Branched cleanup operations, nested handlers, handler-only receives and secondary
exceptions remain at their original positions in the held graph. Only the matched
monitor event is represented by the synchronized release. Every original terminal
and operation retains one emission owner; cleanup effects are not silently
absorbed into an implicit finally. A failing cleanup that actually escapes before
release keeps the monitor scope explicit rather than acquiring an invented release.

Outlet classification does not authorize copy motion. One normal outlet can
execute its own post-release plans after the scope closes. Multiple normal outlets
or abrupt outlets require their post-release plans to be complete and have no
execution steps; P6 does not hoist path-specific assignments into the locked body
or generate a dispatcher. Distinct unlocked continuations that cannot be expressed
by the established outlet forms retain `IncompatibleContinuation`; unsatisfied
post-release plans retain `PostReleaseCopy`. This also preserves value-returning
paths when a natural combination has not been established. Scope/region rejection
retains the original plans and a concrete reason, not a method failure.

The synchronized entry and every release outlet retain their exact P5 obligations.
Successful body completion executes the selected release before its after-release
plans and original continuation. Synchronized syntax and ordinary monitor emission
do not both own the same event. An explicit goto does not add release, finally,
heap rollback or initialization. Independent proven nested regions remain available.

## Correspondence and evidence

Regions retain original block/operation/terminal ownership, normal edge targets,
P5 copies, source-site completions, handler priority and snapshot positions.
Synchronized outlet phases retain original release and continuation locations.
These obligations are not established merely by constructing a region.

The previous core test was removed by operator decision. Ordinary requests do not
run independent symbolic replay or a verification switch. Corpus observations do
not prove arbitrary graphs, callers, callees, monitor patterns or resource paths.
