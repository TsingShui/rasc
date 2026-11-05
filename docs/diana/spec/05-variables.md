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
allocation carrier, exact literal bits or a concrete storage gap. `ValueId` and
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
