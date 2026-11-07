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
`LabelledBlock`, `Try`, `Synchronized` and explicit `Graph` nodes. Enclosing
completion interfaces constrain child construction before ownership commitment. External entrances, disjoint child
ownership, protection boundaries and P5 construction-group membership constrain
composition. Crossing candidates remain explicit rather than duplicating their
shared content. Empty branch arms contain no fabricated instruction.

Private `CompletionInterface` records retain exact node coverage, original normal entrance
and outlet LinkIds, executable exceptional entrance EdgeIds, and original abrupt
terminal NodeIds. An outlet references its actual target and unchanged P5 copies;
a terminal references its original operands, completion, phases and site through
its node. Return/throw categories do not compare values or merge terminals.
Storage availability is not a topology predicate: original accesses and CopyPlans
retain their local gaps, and dependent P7 composition still requires availability.

Private parts carry entry, continuation and membership constraints. Their common
boundary consists of actual outlet targets plus their declared normal follow.
If, Condition, Loop and Switch proposals use this boundary without classifying
an outlet as break or continue. Parent paths supply only selected scope context
when P6 later chooses transfers. All graph walks,
ownership composition and context walks use explicit stacks or finite queues.
Candidate frontiers are derived from the input; a mismatch does not restart method
analysis or launch an unbounded template search. Scratch and result allocation are
fallible and checked, without a configured numeric quota.

## Ordinary branch boundaries

An ordinary branch first proposes its normal postdominator follow when it lies
in the enclosing part, otherwise that part's established follow. Both original
arm reach sets stop at the same enclosing completion boundary. The
primary boundary is admissible when neither arm reaches the predicate, the arms
are disjoint, and their union with the predicate satisfies
`protection::closed_entry`. P6 MUST enqueue only that primary proposal when it is
boundary-admissible. A later owner/barrier rejection MUST NOT trigger another
boundary search or reopen an accepted owner.

Otherwise, distinct original successors supply the only alternative boundaries.
For boundary F and other successor S, A is all normal content reachable from S
without visiting F; the candidate owns only `{predicate} union A` and follows F.
The arm targeting F is empty, with no entry and follow F. The other arm enters S
and follows F. Header/true/false child positions and original link identities do
not change. P6 MUST retain the selected edge's copies even for an empty arm.

Before committing a successor-boundary candidate, P6 MUST establish:

- The predicate is outside A and F is outside the candidate. After contracting
  wholly contained, already-selected natural Loop regions, the remaining normal
  arm graph is acyclic. Only Loop owners are contracted; their explicit
  descendants remain in the product. An arbitrary Graph owner is not a cycle
  abstraction.
- The unchanged `closed_entry` predicate accepts all normal and executable
  exceptional arrivals. If the candidate contains method entry node 0, its
  predicate is node 0.
- Every original normal link leaving a successor-boundary candidate targets F. Original
  return/throw terminals and contained divergent loops retain their behavior;
  exceptional outlets retain their site-specific facts. Reachability is not
  clipped to a guessed enclosing scope or abrupt outlet.
- Existing lock boundaries, P5 construction atomicity, exact/disjoint parts and
  whole-current-owner containment all hold. A frontier cutting an owner rejects
  only that composition; duplicate, foreign or missing content remains a
  classified construction-invariant failure.

Before committing either a primary or successor-boundary branch candidate U,
P6 MUST require laminar membership with every established LoopPlan's lexical
candidate membership: the branch is disjoint, wholly contained, or contains that
membership in full. P6 MUST reject a partial branch that owns the loop's continue
target. A contained branch can own a latch source only when it owns a proved private
exit corridor and its boundary is the original continue target. Its arms then complete
that loop-body execution rather than taking ownership of the evaluation entry.
Other partial latch ownership is rejected. These constraints apply before Loop region selection, alongside unchanged
entrance, arm-reachability, whole-owner and construction checks. Rejection retains
explicit control without reopening an accepted owner.

P6 MUST preserve predicate evaluation, original arm roles, effect order and all
selected-edge copy, exception, lock and construction obligations. F has one outer
owner, not an execution copy in either arm.

One explicit-stack construction selects disjoint enclosing proposals by decreasing
membership size, then Loop/Synchronized/Try/Condition/ordinary priority, full catch
frontier, entry and original successor-boundary identity. It constructs each
selected proposal's exact parts within that interface before committing the
parent once. Every syntax-owned predicate is excluded from its descendant
proposals. A child cannot acquire an incompatible follow before its enclosing
interface is selected. Whole construction groups, contained loop plans and
natural lock scopes cannot be split across parts; the same whole-current-owner
check applies when an owner already exists.

Successor-arm acyclicity is decided once after child selection, contracting only
wholly contained selected Loop owners. A refusal assembles those same original
parts explicitly, without another boundary, solver or reconstruction attempt.
The ordinary part assembler remains the sole Sequence/Graph constructor. There
is no Graph-first retry, alternative method chain or owner reopening. Reverse LT
uses each original exit vertex; void returns do not acquire a canonical runtime
return node.

## Ordered shared-result conditions

A conditional terminal can own one `Condition` region when its two original
arms, within their common completion boundary, are nested rather than disjoint:
one arm is a proper subset of the other, and that shared subgraph has a single
entry `S` reached by one original link of the first predicate. P6 MUST retain
both original links of every test and MUST require each later test to select the
same `S` with one of its own original links; the other link continues into the
next test's original block run or reaches the remaining result arm.

Those ordered tests select between two result arms. The arm their selected
original links reach is the shared arm the region owns. The arm a link from the
tests' own side also merges inside is the second result arm, and only that
merge is the region's normal exit: when such a merge exists P6 MUST derive it
from the predicate's original links inside the enclosing completion boundary and
no other node, so the ordered tests keep one exit instead of leaving at an
unrelated established follow. When the tests' other links only leave at the
established follow, or at the last test's other original link, the region keeps
that exit and owns the arm it reaches as before.

The region owns the test runs, the shared arm and the optional other arm in that
order. A test run is the original block's parts from the run entry to its
terminal predicate; P6 MUST NOT move an operation out of its block or reorder
tests. A run that does not end in another conditional starts the other arm. When
the last other link leaves the region directly, the region owns no other arm and
MUST keep that original link and its copies.

The candidate enters the shared enclosing-interface construction and MUST satisfy the
same original normal/exceptional entrance, disjoint/exact part, whole-current-owner, construction,
lock and loop-laminarity checks. The established loop checks apply to a
`Condition` candidate exactly as to an `If`, using established lexical membership
and the narrow proved-private-exit/continue-boundary latch rule from
[ordinary branch boundaries](#ordinary-branch-boundaries). Partial overlap and
partial ownership of the continue target reject the composition. P6 MUST require each result
arm to be a closed interface: the shared arm is reachable only from its entry
inside the shared set and every link entering it is one of the tests' selected
original normal links, every normal link leaving it reaches the common follow or an enclosing actual outlet,
the other arm is entered normally only by the last test's other original link
and leaves normally only at that same completion boundary, and no test run is entered in its
middle. P6 MUST also inspect executable exceptional arrivals for each child,
including arrivals from another child of the same candidate. An exceptional
arrival from outside a child rejects composition unless it enters the first run
at the whole candidate's entry from outside the candidate. Exceptions internal
to one child retain its ownership; outward exceptional propagation is not a
normal follow and remains unchanged. Only the existing `ImpossibleRelease`
proof excludes an exceptional edge from this check. A shared body therefore
has one owner and no execution copy, and the other result arm cannot jump into
the natural shared arm. Rejection retains the previous owners and the explicit
Graph schedule; P6 MUST NOT retry another boundary or build a Graph first.
An original abrupt result arm needs no common concrete global postdominator.
Condition predicates are syntax-owned by their region. Predicate nodes, links,
copy placements and source origins are delivered unchanged.

After establishing region ownership and transfer plans, P6 finalizes an explicit
alternative for every accepted Condition in the same construction chain.
`ConditionSchedule` references the selected test-run children in order, then the
optional other arm, then the shared arm. Each child retains its entry, internal
placement, operations, terminal phases, copies and abrupt-exit scopes. P6 supplies
one `ConditionCompletion` for every child. A source `Parent` completion denotes
the source parent's follow, not the next printed sibling: a nonfinal child gets
a planned goto to that established follow. A `Next` completion retains implicit
delivery only when the selected next child's entry is its original target;
otherwise it gets a planned goto. Existing `Goto` and `None` completions retain
their meanings. The final child's `Parent` completion uses the condition's own
retained continuation.

P6 constructs child membership from the selected region products, without
flattening nested syntax. The alternative names both original test links and
original `Next` links crossing a child boundary as explicit gotos unless the
child's established follow and selected implicit normal completion already
deliver that destination. Internal links retain their established delivery. Break and Continue retain their target
scopes because the alternative changes neither child containment nor internal
placement; normal completion after an internal scope exit passes through the
child's selected completion. Return, throw, exceptional delivery and lock outlets
retain their original ownership and phases. Planned continuations execute no
second edge copy and invent no cleanup. These rules cover implicit and empty-arm
completion as well as explicit links. Neither the schedule nor its selection
changes source child roles or continuation evidence. Existing closed-interface,
whole-owner and protection checks reject unrepresentable compositions locally
before commitment. Resource and construction-invariant failures retain their
classified errors, not a missing-plan success.

P7 MUST consume the delivered plan: it composes each test's scheduled prelude
only at its original evaluation position and prints one short-circuit condition,
or keeps the established explicit if/goto schedule from the same region. When a
prelude cannot keep operand evaluation order, copy facts or the established
branch role, P7 MUST choose that complete explicit schedule for the conditions
that depend on the missing prerequisite instead of publishing an optimistic
compound layout. The explicit schedule restores each test's terminal at its
delivered predicate portion, after all preceding portions, and keeps every
original branch edge and label. P7 MUST NOT infer shared reachability, split
owners, re-derive the tests or discover the explicit alternative's order by
walking descendant controls. P7 lowers the selected order into `EmissionPlan`;
the retained control IDs, children and Condition roles remain unchanged.

## Loops, switches and continuations

`LoopForm` is `While(LoopCondition)`, `DoWhile(LoopCondition)` or `Infinite`.
`LoopCondition` identifies the original predicate node, repeating link and exiting
link. Its polarity is preserved by those original edge roles. The body and the
syntax-owned predicate have separate execution ownership.

`LoopPlan::evaluation()` records the ordered original predicate evaluation nodes.
For a head test, P6 follows only the natural header's private `WithinBlock` prefix
to the same original block's conditional terminal. Every interior portion has
exactly its original preceding normal link and no executable exceptional arrival.
The prefix stays in the backedge core and contains no monitor operation. P6 does
not cross an original branch, rotate a tail test or hoist initialization. Original
operations, terminals, protection sites and P5 positions remain unchanged; candidate
partitioning still rejects split construction groups, scopes and existing owners.

The head evaluation and repeated body are distinct children. A head-loop continue
returns to the original evaluation entry, including its operations, not merely to
the terminal portion. If the head candidate fails, P6 retains the existing tail
candidate or Infinite form. A genuine tail test keeps its original predicate and
continue target at the tail. Traditional for, enhanced for and for-each are not
region forms. An evaluation prelude that cannot compose into a compact expression
keeps the existing explicit predicate-phase notation.

Private parts distinguish `Ordinary` completion from `LoopBody(LoopId)`.
Only an accepted Loop candidate supplies `LoopBody` to its body part before
composition; its predicate and all other parts remain Ordinary. P6 MUST keep the
body follow equal to that LoopPlan's continue target. During body composition,
original links to that target complete the lexical body execution and are omitted only
from the private sibling-successor relation, even when the target is inside the
body. Original links, edge/link plans and copies remain unchanged; subsequent
transfer planning retains Continue/LoopCondition selection. A link leaving a selected
loop's lexical content cannot become Next merely because an outer Sequence or
branch follows its target. It retains an explicit goto unless an enclosing
break/continue scope establishes that original transfer.

P6 MUST NOT project another cycle-closing link, use an unselected loop as context,
or infer loop completion from `follow == entry` in an Ordinary part. Exported
entries, one successor per child, no repeated remaining child, complete root
coverage, whole-owner membership and Graph selection retain their existing checks.
A selected loop child contributes every exit outside its lexical membership to
its parent's sibling relation. Proved private corridor content inside that membership
is not an independent sibling successor.

A conditional loop's normal continuation is the actual destination of its exiting
predicate link. It is not a common postdominator beyond exit-side work. Other loop
exits retain their own targets. A non-terminating region does not acquire a
synthetic exit to make postdomination convenient. `LoopPlan::members()` retains
the original backedge core, latches and exits. `lexical_members()` separately records
the proposed lexical content, including closed synchronized scopes and their
release/break corridors. `private_exits()` identifies original exits with admitted
private corridors. The predicate's actual exiting link and destination remain
the loop follow; lexical ownership does not skip exit-side work.

Before candidate enumeration, P6 can add a private corridor from another
original loop exit. Admission requires a finite single-normal-successor path ending
at its original return/throw or at the predicate's actual normal follow. The follow
remains outside lexical membership. The corridor has that exit as its only normal
entrance and no executable exceptional entrance. The path cannot reach the core,
intersect another loop core or an established try/synchronized plan, contain a
monitor, or throw to a local handler. Construction groups stay whole in the enlarged
loop; candidate partitioning also keeps them whole in each child. Shared corridor work, cycles, branching corridors and unknown completion retain
explicit control. The selected body interface retains private operations and the
original outgoing copies before its selected break; predicate-exhaustion copies
remain on their distinct original edge. A candidate loop supplies no break scope
until that Loop region is selected.
P6 does not compare returned values, merge terminals or manufacture an edge for a
throw. The enlarged candidate still passes normal/exceptional entry closure,
whole-current-owner, scope and partition checks before commitment. Rejection affects
only that composition; resource or construction-invariant failure keeps its existing
classified RegionStop behavior.

Original switch case keys, default, roles and duplicate-target edges remain distinct.
A switch with no concrete postdominator can use one unique first shared actual
normal target as its external continuation. Reach counts identify sharing of
original targets, not equality of returned values. Multiple first shared targets
remain unresolved and explicit.

Before child construction, P6 can give a switch one private child per original
arm instead of one multi-entry body. `private_cases` identifies this child layout:
selector first, then bodies in original arm order. Admission requires distinct
arm targets, disjoint normal reach sets inside the common completion boundary,
only the selector's original normal entrances, no executable exceptional arrivals,
whole construction groups/scopes and complete body coverage. Each arm has no
normal cycle and completes by original return/throw or an established outlet.
A default targeting the follow has an empty child, not missing selected-edge
copies. Shared follow work has one external owner. Normal completion of a case
body becomes Break only against the selected Switch; successful abrupt terminals
retain their identities. No shared tail is duplicated or arm disjointness relaxed.

A rejected private-case composition retains the existing single body, including
its local graph with multiple case entrances and fallthrough. Duplicate-target
arms retain that layout. The selector has one execution owner; recovery does not
introduce a state variable, generated dispatcher or repeated selector evaluation.

`Continuation` records whether a completed region finishes its source parent,
reaches a confirmed next position, or needs an explicit goto/break/continue.
Only selected ancestors resolve the latter scope completions. A region with no
normal completion toward its follow receives no fabricated onward continuation;
an original break/continue that already leaves that region does not execute a
second planned jump. Completion emerging from a contained scope still proceeds
through the enclosing child's completion, including the explicit Condition
alternative. P7 MUST consume this
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
