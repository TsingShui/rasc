# P5 storage and transfer products

The algorithm and semantic obligations are defined in
[PIPELINE_DESIGN §5.5](../PIPELINE_DESIGN.md#55-p5按边-out-of-ssa-与存储传递计划).
This chapter describes the active product boundary and implementation in
`crates/diana-decomp/src/p5/`. Execution observations belong in the
[engine README](../../../crates/diana-decomp/README.md).

## Construction and consumers

Engine's method task calls `build_storage(task, typed)` once. The constructor
consumes a mature `TypedMethod` and returns `StorageBuild::Ready(StorageMethod)`
or `StorageBuild::Stopped(StorageStop)`. Only P5 has product construction access.
`StorageMethod` privately owns P4; it provides `StorageFacts`, immutable P4
interpretations and original execution queries. It does not extract a predecessor
or start another analysis chain.

`AnalysisEvidence` is a separate borrowed inspection view. It supplies retained
SSA/type facts to `dbg_shell` and existing core checks, without construction or
mutation authority. P6/P7 consume storage accesses and plans, not this inspection
view as a substitute for a missing P5 fact.

## Bindings, reads and materialization

`StorageFacts::bindings()` has one entry for each original SSA `ValueId`.
A binding identifies an original parameter, independent local, reference carrier,
allocation carrier, deferred construction publication, exact literal bits or a
concrete storage gap. `ValueId` and
`LocalId` have separate identities. P5 does not coalesce general values or build a
second register environment.

P5 consumes P4 families and widths. `StorageType::NumericBits` preserves an
established numeric word/wide representation without choosing int versus float
or long versus double. It does not accommodate an unresolved numeric/reference
union. Literal bindings retain original bits and each existing use interpretation;
they are not default values for an unavailable source.

`UseAccess` records the original use, execution point, state, P4 interpretation
and position-related initialization state. `read()` reports both original source
problems and storage dependency gaps. `original_source()` preserves the earlier
source description for inspection; it does not override `read()` or a local gap.
An unavailable input layout remains an error, not an empty argument list.

`DefinitionPlan` records every encoded operation output, its original state/point,
commit rule and commit PC. Materialization distinguishes stores, exact literals,
allocation carriers, original exception captures and unavailable output states.
Effects and unused producer operations remain in the original execution product.
A missing materialization does not authorize deleting an operation or executing
its producer again at each use.

`ConstructionPlan` retains the original P4 constructor event and corresponding
receiver access. Allocation carriers become available at the allocation's normal
commit, not at the constructor call. Reference carrier storage alone does not
prove initialization. Per-use initialization remains the original P4 fact;
absence of an initialization record does not mean initialized.

## Construction groups

`ConstructionMode` selects either a natural construction group or explicit
operations with a concrete `Separation`. Every new-instance has an allocation
plan, including unmatched or unresolved initialization. `ConstructionGroup`
retains the original allocation, exact constructor, argument reads, original
normal continuation, member operations, interior edges, normal publications and
snapshot obligations. No new SSA value or alternate method chain is created.

The planner consumes P4's position-related allocation binding and initialization.
It requires a same-occurrence receiver before construction, an unshared linear
normal corridor, and transparent object aliases/literal definitions inside that
corridor. Branches, external entries, required intervening transfers, other
operations and unavailable arguments retain explicit plans. Arguments use existing
storage or P4-proven literal views; the planner does not inline arbitrary effectful
computations or move allocation to the constructor site.

A private carrier can be deferred only when original uses cannot observe it
without the constructor's normal transfer. Relevant failure-path reads or uncertain
instance relations prevent contraction. Original definition/commit points remain
facts; `Deferred` marks the selected emission, not a rewritten DEX commit.
Publication occurs on the constructor's original normal continuation. Allocation
failure publishes nothing; constructor failure retains its effects without
publishing a fictitious initialized object.

Ordered handler chains must agree. That is not sufficient by itself: all
before-throw snapshot requirements must be equivalent and their sources available
at the allocation anchor. A common original copy plan becomes the group prelude;
all original snapshot identities remain inspectable. Stage-specific or unavailable
saves prevent contraction rather than being dropped or default-initialized.

Consumers MUST inspect `operation_emission` and `copy_emission`. Construction
members are lowered once through their group, not also emitted as ordinary
operations. Reads of a deferred receiver/alias inside the group name private
construction storage; they are not independently emitted local reads before
publication. Original requirement records remain semantic evidence, not a second
selected emission route. P6 preserves group membership and P7 follows the selected
natural/explicit mode without re-identifying construction patterns.

## Transfer coverage and placement

`PhiTransfer` accounts for every original Phi incoming, retaining its `PhiId`,
`TransferId`, source state and P4 adaptation. Its route identifies a literal,
normal copy, exception save/receive pair or local gap. Consumers MUST inspect the
referenced copy assignment's issue as well as the route; an existing plan ID is
not an established-copy guarantee.

Normal `CopyLocation::Transfer` retains the exact selected transfer. Its site
identifies initial entry, a normal edge, a normal commit successor or a continuation
before its original operation/terminal. Initial-entry copies execute only on the
initial transfer, not on a loop backedge. Result materialization precedes copies
on its normal commit successor. Equal target blocks do not merge distinct
transfers into an unconditional block prologue.

An exceptional incoming uses a separate snapshot local for its handler/Phi.
`BeforeThrow` saves the incoming at the original operation or throw terminal.
`HandlerEntry` receives only on exceptional arrivals and records the original
`MoveException` capture when present. That original capture precedes the receives;
P5 does not synthesize or duplicate the exception object. Snapshot reads receive
an already adapted save value rather than applying its view twice. All incoming
identities remain recorded even when identical saves share one assignment.

## Copy construction and local gaps

`CopyPlan::assignments()` retains simultaneous-copy requirements. `steps()`
contains ordered `Assign` and necessary `Save` steps. Source-reader indexes and
pending-read counts select destinations whose old storage is no longer needed.
When no destination is ready, P5 saves one old destination into matching temporary
storage and redirects all its pending readers. Redirection retains original value,
state and view associations. Only a proven identity self-copy is omitted.

P5 builds storage dependency components when an assignment is unavailable. A gap
propagates through the affected storage-interference component, its destinations,
snapshot receives and uses of those locals. Independent producer locals and
unrelated copy components remain available. Both ordinary `UseAccess` reads and
constructor receiver accesses receive these dependency gaps. Original assignment
sources/destinations remain inspectable; missing copies do not become no-ops.

Consumers MUST preserve unavailable plan components explicitly. They MUST NOT
supply zero/null, silently omit required copies or treat an unavailable snapshot
as an initialized local. `is_complete()` describes assignment availability within
one plan, not method verification or source-language legality.

## Test-only correspondence

The sole scoped core test in `src/dynamic_instructions_test.rs` consumes public
Engine products. It does not construct P5 products, call private scheduling helpers
or add a second analysis entry. The test checks binding/use/output/incoming
coverage against the retained P3/P4 facts, including original state, transfer,
interpretation, commit and handler-capture identities. Those upstream facts are
trusted phase inputs, not independently reconstructed SSA or type conclusions.

After checking incoming requirements against upstream facts, the test evaluates
simultaneous assignment from an immutable old symbolic store and compares it with
P5's ordered steps. Distinct cell symbols expose overwritten old values; temporary
reads require a preceding save. Literal bits and view associations remain explicit.
A slow all-pairs least fixed point, separate from production's union/find worklist,
checks missing-copy interference, consumer availability and reachability from each
reported original missing-copy cause.

Finite traces choose original branch/throw PCs, targets and catch order, not P5
plan IDs. Reference Phi updates use the supplied original incoming. Physical
storage follows P5 copies and materialization. Every original definition occurrence
has a fresh symbol, preventing one iteration from borrowing a previous iteration's
value. Snapshot and temporary storage starts undefined; tests do not seed defaults.
Handler receives follow the original exception capture. Allocation failure produces
no allocation result; constructor failure retains the previously committed carrier.
For construction groups, the physical trace separates internal carrier storage
from published locals. Group prelude saves execute once; successful initialization
publishes each original alias, while both failure paths leave publications absent.
Checks retain original events, reads, commits and once-only member ownership.
These traces model storage correspondence, not concrete VM arithmetic, heap effects,
external invocation behavior or ART admission.

Under the current independent-storage policy, different value bindings have
different locals. P3's exact-slot incoming guarantee means a normal Phi source
shares its destination Phi's slot. Different Phis on one transfer target have
different slots. Therefore, a source cannot be another destination in that copy
set. Snapshot namespaces similarly separate saved sources from destinations.
The test checks this structural invariant on its actual products. This argument
does not verify the generic multi-storage cycle branch for arbitrary constructed
copy graphs, or establish execution coverage of self-adapted temporary saves.
Consumers MUST NOT weaken sealed products or fabricate a private constructor merely
to claim such coverage.

Acceptance: each targeted mutation reaches the relevant obligation and causes a
P5-specific core-test failure after successful compilation. An unreachable branch,
compiler error or unrelated upstream failure does not establish mutation rejection.
All independent checks remain under the existing `cfg(test)` boundary, without a
runtime switch. Execution results and remaining evidence gaps are recorded in the
engine README.

## Stops and safety

`StorageError` distinguishes resource construction errors from internal invariant
failures. Allocation, arena-ID and arithmetic checks remain in the owning stage.
A construction stop preserves mature P4 and a distinct unpublished
`StoragePartial`; the snapshot cannot satisfy `StorageFacts` guarantees. Engine
retains the method stop alongside other selected member outcomes.

A mature storage product also retains ordinary local gaps. Source unavailability,
P4 interpretation problems, representation gaps, input-layout gaps and dependent
storage failures have separate records and locations. Their presence does not
turn the whole product into an unpublished snapshot.

P5 uses no numeric work quota, cancellation callback, external declaration lookup
or runtime verification switch. It does not independently replay SSA, repeat type
solving or validate whole-method correspondence during ordinary construction.
The scoped acceptance obligations in PIPELINE_DESIGN remain separate from build,
inspection and finite corpus observations.
