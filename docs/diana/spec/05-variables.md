# 05 — Storage and copies

Code: `crates/diana-decomp/src/p5_variables/`; entry: `allocate` in
`normal.rs`.

P5 consumes typed values and plans source storage, normal-edge parallel copies
and exception snapshots. Copy plans preserve the original transfer semantics
when sources and destinations overlap. Exception snapshots represent the
register state at the relevant throwing site, not a handler-global latest value.

Storage and copy construction MUST retain original definition identity, effect
order and classified failure/resource behavior. A synthetic source initializer
MUST NOT be presented as an original DEX definition.

## 5. Locals, Phi copies and exception shadows (T08/T17)

### 5.1 Parallel copies on normal edges

A Parameter binds the original parameter; every other initialized, non-pure-literal retained
Value and Phi has its own independent Local and is not coalesced. The uninit aliases of §9.3 use
DeferredAllocation and MUST NOT be given a fake null local.
For every Phi incoming a `CopySet[IncomingEdge]` is created; EntrySeed copies are placed at the
method entry (constructors are constrained by the prologue-substitution and post-normal
materialization rules of §9.2), and the other copies are executed only **after that edge is
selected**. The type comes from P4, and the source is a materialized local or a pure literal.
`CopySource::Zero` retains P4 zero evidence, its use view and the target storage
type. It creates no local or runtime computation. Local sources participate in
copy dependencies; literal sources do not. A literal destination still waits
until pending copies have read its old value. The copy verifier replays literal
and local sources as distinct symbolic values, and Java verification checks
literal provenance against original SSA definitions.

```text
delete self-copies with the same Local and the same type/conversion
while pending is non-empty:
  charge and check cancellation
  if some dst does not occur in any remaining source:
    emit that copy and remove it
  otherwise:
    pick a remaining local source s; create a new exact-type temp t
    emit t=s; rewrite every source==s to t
```

Each emitted copy or broken finite cycle terminates the loop; temps, scans and output are charged
per item, and a stable tie-break is not required.
`a<-b,b<-a` may yield `t=b;b=a;a=t`, and a three-cycle likewise preserves the sources before they
are overwritten.
The sources correspond to the reviewed local correspondence with Boissinot Out-of-SSA and DX;
their coalescing and relaxed casts are not copied.

The copy verifier records each specified destination once, replays sequential
steps and compares all original local values with the simultaneous assignment.
Duplicate destinations, uninitialized temporary reads and changed literal views
remain classified invariant failures. An empty copy set with no steps or
temporaries needs only a cancellation check; it does not allocate a replay arena.

The Definition check replays from EntrySeed along each block's entry copies, local writes and uses;
every path obeys the inductive relation “entry Phi assigned → Op reads old value then commits →
edge parallel copy”. The 0/null that Java requires for DA is allowed only on ordinary locals for
which that relation proves the value unobservable; it MUST NOT repair an Undefined or initialize
an unconstructed object. The constructor prologue replays with the proven formal/literal
substitution of §9.2 and MUST NOT claim that a not-yet-materialized Phi local is assigned; after
super/this completes normally and the deferred entry copies execute, the ordinary body invariants
above are restored.

### 5.2 Handler ingress

An exception edge has no executable edge block. For every `(handler, retained Phi)` an independent
ShadowLocal is created; the exception incoming is saved in the shadow that each handler possibly
selected at that site needs, immediately before the throw op.
All sources are already materialized, available, initialized and converted without throwing; no
cast, boxing or call happens in the snapshot.
Several snapshots do not share slots and no cross-handler alias optimization is performed. The
catch entry binds the exception object first and then copies the shadows into the handler Phi
locals; the definition and source of the original move-exception are still consumed, and the
exception object MUST NOT be defined twice.

D029: the successful call value enters only the normal Def; the Phi used by catch takes its source
from the snapshot before the call.
D030: each site saves its own old value, and catch MUST NOT read a handler-global “most recent”
register table.
A throw inside a nested catch saves the shadow the outer handler needs by the same rule, and the
slot is uniquely identified by handler+Phi.
When availability, type or protection position is unproven, the implementation MUST stop that Java
profile and MUST NOT add a guessed default value.

### 5.3 Why the parallel copy selects a leaf by destination

Let the incoming store be lambda0; the specification of the set `d_i <- s_i` is that all sources
are read from lambda0 and then written simultaneously.
A destination is unique (one Phi is assigned once on this edge), while a source may fan out and
may also be a literal. If some d is no longer a pending source, writing d does not destroy any
remaining read; therefore that copy MAY be emitted first. The reverse condition is wrong:
`a<-b,b<-c` MUST emit a=b first, because emitting b=c first would lose the old b for a.

If no such d exists, the remaining graph has a cycle. Emit `t=s` to save the old value first and
rewrite **all** pending reads of s to t; later overwrites of the original s then lose no old
value, and the cycle is broken. For example:

```text
entry a=A,b=B,c=C
parallel a<-b,b<-c,c<-a
order t=b; b=c; c=a; a=t
result a=B,b=C,c=A
```

A self-loop Identity MAY be deleted, but different conversions MUST NOT be deleted merely because
the LocalId is the same. A simple implementation may be O(k²); every scan and replacement of an
element is charged as work, not charged once in the outer loop; temps do not exceed the number of
broken cycles and their capacity is charged first. The implementation need not introduce
coalescing or a global scheduler to reach the paper's minimum-copy goal.

### 5.4 Derivation that shadows and DA initial values are unobservable

For every site s that can enter handler h, the shadow assignment it needs MUST be immediately
before s and consist only of no-throw local/literal copies. If s throws, that snapshot has already
completed and the catch ingress reads exactly sigma_before_s. If s returns normally, a later
overwrite of the snapshot by the next site is harmless: catch consumes only the snapshot of the
throw that actually happened. Distinct handlers own their slots exclusively, which prevents inner
cleanup from corrupting the value the outer handler needs.

Java DA may still require a 0/null initial value for the shadow outside catch. The justification
for adding it is not “it is overwritten in the normal case” but that **every accepted exception
edge** has a complete snapshot and the snapshot itself throws nothing; any counterexample that
misses one snapshot prevents Verified. The throw terminator and throws inside nested handlers MUST
also be included, and the check MUST NOT enumerate only calls with a result. The heap is not
rolled back by the snapshot.

EntrySeed copies, normal edge copies and before-throw snapshots have three different placements
and MUST NOT be merged into one uniform “assignment at block end”. Two edges to the same target
MUST NOT be merged by target alone either; the copy is executed only after the original EdgeId is
selected.

#### 3.5 Storage and lexical plan

```text
CopyPlan {
  entry: ParallelCopySet,
  normal: EdgeId -> ParallelCopySet,
  before_throw: InsnId -> Vec<ShadowSnapshot>,
  ingress: HandlerEntry -> Vec<LocalCopy>,
  temps: Vec<LocalDecl>
}
LocalBinding = Parameter | OrdinaryLocal | InlineLiteral | DeferredAllocation(AllocationId)
ShadowSnapshot { handler, phi, dst_shadow, src_local_or_literal, exact_type }
TransferPlan { original_edge, copies, target, abrupt_kind }
RegionPlan { owned_blocks, owned_edges, nodes, entry, follow }
ProtectionPlan { owner_try, ordered_catches, site_to_catch_stack }
MonitorPlan { enter_site, lock_identity, body, normal_exits, release_handler }
```

P5 MUST select the edge before executing the copies; the exception snapshot MUST sit immediately before the original throw, and a catch parameter MUST bind ExceptionAt before the shadow is copied. An independent replay of the original uses/entry copies/transfer checks that the default local is unobservable, and the verifier MUST NOT trust a boolean certificate. P6 MUST NOT change the CFG and MUST NOT copy effects for natural source. P6 MUST reduce try from inner to outer and validate the actual catch-stack, and it MUST replace synchronized with a monitor only after the finite lock stack and every exit are validated. The concrete matching rules belong to [06-regions](06-regions.md), and the printer MUST NOT fill in a structure that does not satisfy them or widen the protection domain.
