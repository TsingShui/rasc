# 07 — Regions and control structure

Code: `crates/diana-decomp/src/p7_regions/`; entry: `plan.rs::build`.

P7 reads the P6 storage product and chooses a plan for structured control flow,
a dispatcher, or protected regions. It also handles switch, exception and monitor
planning. Natural structure and dispatcher consume the same checked operations,
storage and copy plans.

P7 MUST preserve original reachable blocks, transfers, effect order and exception
and lock facts; selecting a prettier structure does not establish semantic
equivalence. If the current implementation cannot plan a shape, a downstream
decline is not proof that the original DEX is invalid.

## Ordinary structures and the dispatcher

### An executable dispatcher

Eligibility is: no handler, no monitor, no unhandled uninit, and P5/P6 sealed. All locals are
declared in the method scope, pc starts at the entry block, and each reachable block is one case
that executes the original ops and the terminator:

```text
Jump(e):             copies(e); pc=target(e); continue
Branch(c,t,f):       if(c){copies(t);pc=target(t);}else{copies(f);pc=target(f);} continue
Switch(v,keys,d):    select the same EdgeId for the materialized v with an integer if/else chain; copies(edge); pc=target(edge); continue
Return/Throw:        execute the original return/throw; do not set a fake fallthrough
```

The pc default branch exists only as an unreachable internal error; the implementation validates
the initial pc, the target of every assignment and the closure of the case set.
The local store at each case entry corresponds to the original block entry, and the copy happens
before the pc update; this is an execution relation, not a span count.
Both condition and selector are already evaluated values. There is no Object[] register
interpreter, exception dispatcher or dynamic boxing.

### Deterministic recognition rules for natural structures

On the read-only CFG, compute the normal RPO, the dominators and the postdominators (return/throw
join a virtual Exit), using an explicit stack rather than unbounded recursion. The dominators MAY
be computed with RPO iteration of the immediate dominator and an intersect over ancestor chains;
postdominators are computed the same way on the reverse graph.
Every change enqueues the affected nodes, an ancestor walk is charged per actual step, and SCCs
use an explicit-stack Tarjan; the graph is not rewritten.

The private RegionArena starts with one node per block; only the following implemented schemas
are accepted, and each reduction decreases the node count:
- Sequence: a has only the next normal edge to b, b has a single normal entry, and the
  protection context is the same; the edge copies are preserved.
- Diamond: the branch arms reach a common postdom join, the arms have single entries and their
  interiors can be reduced by the current schemas; head
  ops execute once, and the two transfers each consume their own CopySet; abrupt arms MAY also
  use the virtual Exit as follow.
- Loop: the back-edge target dominates the source, the natural loop is found from the reverse
  predecessor closure, and several latches merge into the same header; external entries MUST
  reach the header only. The uniform output is a labelled `while(true)`, and the header ops
  execute on every iteration.
  Header exit edges copy first and then break; a latch to the header copies first and then
  continues. Additional exits are accepted only when the current or enclosing loop already
  determined a follow, emitting a named break respectively; other multi-exit shapes go to the
  dispatcher.
- IntegerSwitch: the selector is evaluated once; let n be the key count and R=0 (n=0) or checked
  i64(max-min)+1.
  A Java switch is emitted only when R<=4*n+16; a sparser graph uses an if/else chain inside the
  dispatcher, which avoids target-table blowup.
  Case labels MAY be merged when the target and the incoming copy are the same. Fallthrough
  is accepted only as an acyclic case-body chain, the edge copies MUST be preserved on the chain,
  and direct-entry copies MUST NOT execute twice.
  When natural fallthrough cannot express a shared tail/copy conflict or a case jump, the whole
  method uses the dispatcher; the implementation MUST NOT first create trampolines or
  duplicate effectful tails to “optimize” the source.

Schemas are attempted in the existing graph's readiness relation; the implementation does not
sort for stable text and does not seek a global optimum. Before a candidate, it computes an upper
bound for the charged nodes, transfers, conditions and copies; an over-limit candidate is not
created. An ordinary mismatch continues with the next schema, and when no natural structure
matches but the method is eligible, the dispatcher is selected; a certificate missing an edge or
site is a BUG and MUST NOT be called an unsupported pattern.
Required Java cases such as D007 accept a legal dispatcher, and natural source shape is not a
success condition.

### Structure validator

Validate `BlockId→executed node`, `EdgeId→transfer` and the original op order of every block; a
producer/result MAY merge into one source expression together with synthetic pure steps, but a
dynamic effect MUST NOT be omitted or repeated. The normal transfer's CopySet and target, the loop
continue/break targets, the switch keys and default, and return/throw are validated separately.
Protection and monitor checks run separately per §10/§11, and exception edges MUST NOT be placed
into normal dominators.

### Mathematical specification of dominators and postdominators

In the normally reachable subgraph: `Dom(entry)={entry}`, and for the others
`Dom(b)={b} union intersection Dom(pred)`.
Initializing the other nodes with the full reachable set and intersecting repeatedly gives a
finite reference specification that only decreases; the implementation follows §6.2 with RPO
and the idom tree instead of a resident B×B matrix. intersect walks from the node with the later
RPO number towards its idom, each ancestor visit is charged, and the implementation MUST NOT
bound only the outer rounds while letting one ancestor chain run unbounded.

Postdominators are computed only on the reverse subgraph that reaches the selected normal Exit;
a real return/throw or the already defined follow of the current region may connect the analysis
Exit. The analysis MUST NOT add a fake runtime exit to a **non-terminating SCC**; its ipdom may be
absent, and absence MUST NOT justify creating a break or return. A natural loop without an exit
MAY be `while(true)`, and a structure that cannot be proven still uses the dispatcher that
satisfies eligibility. Exception catches MUST NOT be mixed into the normal postdom graph.

### Local execution relation of structure reduction

A Sequence join consumes the original edge transfer exactly once; the Diamond head ops and
condition execute once, and the two arms MUST NOT recompute the condition; a Loop keeps the
header inside the body of every iteration, and continue performs the latch copy first; the Switch
selector is materialized, and the case key/default selects the original EdgeId. Reduction
decreases RegionArena nodes without modifying the original CFG; one effect node MUST NOT acquire
two source owners to “share a tail”.

When the entry/follow/exit mapping a schema needs is incomplete, the schema is Declined, and the
printer MUST NOT guess. The candidate-space estimate includes the old plan and the new nodes alive
at the same time; when work or memory is insufficient, the implementation MUST NOT create a huge
candidate first. An ordinary graph MAY choose the existing dispatcher representation, while a
protected graph MUST NOT use it to skip catch/monitor constraints. A logic BUG is still red and
MUST NOT be called “another unsupported case”. Literal similarity, span coverage and the path
execution relation are different checks.

## Try/catch and protection domains

### Inputs and matching conditions

Every throw site keeps the ordered `HandlerChain=[(catch type|all,target)]` and MUST NOT sort it by
type or target.
The normal structure and each handler entry are constructed separately with the natural schemas of
§6, and the dispatcher MUST NOT be used for a protected graph.
A shared handler entry MAY be referenced by several raw catches, but an ordinary edge MUST NOT
enter move-exception.

Single-try matching algorithm:
1. Select the single-entry body region from the raw try start/end leaders; its normal exit is a
   common follow or a return/throw; the ownership sets of all participating blocks and handler
   bodies MUST be disjoint (the common follow does not belong to the handler).
2. Each handler reduces independently from its entry to the follow or an abrupt exit; a jump back
   into the protected body, an ordinary cross-catch reentry or a shared tail of effects that has
   not been separated is rejected. Only original effects are consumed, and cleanup MUST NOT be
   duplicated.
3. Generate the catches in the original chain order. When several types share the same handler
   body, multi-catch is allowed only when no type is a subtype of another and the body does not
   reassign the catch parameter; otherwise each catch emits its own original path, or the profile
   is explicitly unsupported.
4. An ordinary subtype catch MUST precede the supertype catch; catch-all is Throwable. P4 supplies
   the throws obligation and P8 performs the final check.
   The before-throw shadow is placed immediately before the corresponding op; the handler
   exception value binds the catch parameter, and the ingress copies are at the beginning.
   If a rethrow still carries that handler's original exception through pure moves or Phis of the
   same root, the printer emits `throw catchParam` directly and keeps catchParam effectively-final;
   it MUST NOT change it to `throw throwableTemp` and lose Java precise-rethrow typing. The
   corresponding escaping checked set is the declared throws of the original protected operation
   filtered by the catches, joined with the cleanup's own throws, and MUST NOT be widened to
   Throwable uniformly; when the exception is unknown or replaced, this rule cannot be used.
5. Recompute the actual catch selection of every possibly throwing operation in the output; cleanup
   outside the original try MUST NOT be wrapped into the old catch.

### One nesting level and the validation algorithm

T18 uses **bounded bottom-up try reduction**, not the guess that “catch-all means finally”:
- Candidate handler groups come from a consecutive prefix of the original chain; an unconsumed
  suffix is recorded as the ambient protection context.
- If throw sites inside a handler body point to another handler, record the
  inner-handler→outer-handler dependency; only an acyclic relation that forms one level of lexical
  containment is accepted, and the inner try is reduced together with its handlers into one node
  first.
- A body candidate is a run of whole nodes in the same normal region or one complete subtree; if
  the boundary would cut through an if/loop that is not yet reduced, reduce its interior first or
  split the sequence, and when that still cannot be explained, the profile is unsupported rather
  than duplicating the condition or effect.
- The outer level MAY contain the reduced inner try and its handlers as one whole node. A later
  chain entry covered by an earlier entry MAY be ignored only under complete subtype facts;
  otherwise the order MUST be preserved or the method rejected, and unreachability MUST NOT be
  guessed.
- Every reduction consumes at least one unassigned handler group; candidate counts, node visits
  and subtype queries are charged step by step. A shared or crossing graph that does not satisfy
  exclusive entry/lexical containment does not expand the search further.

The final validator maintains a catch stack recursively over the AST (implemented with an explicit
stack): for every original throw site it expands its enclosing Java catches from inner to outer,
removes the shadowed entries that can never beat an earlier entry according to the complete catch
type relation, and requires the first matching target for every possible type to agree with the
original chain. Unknown type relations MUST NOT justify removing entries; an added pure copy throws
nothing and creates no extra catch site. The catch stack of a handler body contains only its
enclosing catches, so a sibling catch of the same try MUST NOT catch the exception the handler
throws itself.

This directly covers the old values of D029/D030, the priority of D031, cleanup outside the try of
D032, the explicit return/throw cleanup paths of D033/D034 and the one nesting level of D035.
Existing duplicate cleanup and Throwable rethrow are preserved and MUST NOT be sugared into
finally; when the implementation does not reach a required behavior, that case is still incomplete
and MUST NOT be changed to a debug expectation.

## Monitor and synchronized

P7 first matches the canonical monitor/release-handler groups and then hands them to the
normal/try reduction as whole nodes with ownership; a not-yet-recognized monitor cleanup MUST NOT
be rejected first by the general try profile. Nested scopes are checked level by level against the
lock stack, and natural loops/sequences inside the body are analyzed on the original graph without
duplicating effects or modifying the catch chain.
When the lock scope/identity/freshness proof fails, the result is Unsupported/MonitorScopeUnproved,
and the implementation MUST NOT add a lock or default to damaged format; other confirmed Malformed
facts remain irrevocable.

First compute finite reference-root facts: Parameter/this and other Ref producers each have a root,
a pure move and a successful check-cast take the root along their source; Phi unions its incomings
over the finite domain `None→One(root)→Many`, and only One MAY be used for the same-root check
(grounded is separately validated). The literal null has an independent constant root; Many MUST
NOT guess the same object. These facts are also used for the original exception rethrow of §10,
without rewriting SSA identity or performing a general ValueRecovery.

`LockIdentity` additionally needs a **dynamic freshness check**: the Op that defines the root MUST
precede the matching enter and lie outside the candidate body/release-handler; the candidate is
single-entry with every exit released, and the root producer MUST NOT re-execute while the lock is
held. Parameter/this cannot be redefined anyway. The same AllocationId/ValueId is not enough to
prove that a new or field read executed twice by a loop yields the same object, so it MUST NOT
justify accepting a new object at the exit. Unknown alias/freshness MUST NOT be guessed; the solved
lock value is materialized once and then used for synchronized.

A bounded lock stack (root+enter site) is propagated along every normal and exception transfer,
starting empty; a successful enter pushes, a failed enter keeps the old stack, and a successful exit
pops only the top entry with the same identity. The join requires identical stacks, and the height
of a single propagation stack is constrained by the explicit max_nesting (if any); no numeric depth
limit is set by default, and validation still MUST NOT use host recursion or unbounded path
enumeration. The exception PC of a protected exit is determined per §2 first.

A synchronized region is matched only when all of the following hold:
- the single entry lies after a successful enter, and the lock expression has been evaluated once;
  every normal exit goes through an exit on the same lock;
- an exception thrown by the body releases the same lock through the original catch-all and then
  rethrows the **original exception**, with no extra cleanup effect;
- each return value is already computed before the release, nested/reentrant cases are matched per
  stack level, and every path releases exactly one level;
- the evidence excludes a null exit or lock mismatch within the accepted profile; an enter failure
  path MUST NOT be treated as already holding the lock;
- every body op and original try boundary stays in the same scope, and the printer MUST NOT emit an
  empty synchronized and then execute the call outside it.

After a match the printer emits `synchronized(lockLocal){body}`, consuming the original
enter/exit/pure release handler. A method-level modifier is used only when the original declaration
already carries the synchronized marker, the matched scope is the **whole method** on the
this/static Class lock, and there is no effect before or after it; an ordinary block MUST NOT be
turned into a reflectively visible synchronized method, and the DECLARED_SYNCHRONIZED marker of a
managed method does not itself authorize adding a lock. A native method without a body emits a
declaration according to its real execution flag only.
The raw flags are still kept separately. When balance cannot be proven or extra release logic
exists, that Java profile is unsupported.
