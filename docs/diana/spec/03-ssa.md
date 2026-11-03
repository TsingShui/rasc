# 03 — SSA

Code: `crates/diana-decomp/src/p3_ssa/`; entry: `normal.rs::build`.

P3 consumes the P2 flow product and associates register uses with static
definitions. Block-entry Phi values represent incoming register-word state;
normal and exceptional edges carry their respective environments. A throwing
operation's exceptional edge uses the state before its result is committed.

A definition identity is local to the current method analysis. P3 MUST preserve
word widths and distinguish an unavailable definition from an uncertain source
type. Missing definitions and inconsistent wide registers retain classified
failures rather than becoming default values.

## 3. Fixed entry Phi construction: bind once, do not iterate on ValueId guesses (T07/T16)

### 3.1 Algorithm and data adopted

**Algorithm: live-in slot → fixed entry Phi → one pass per block → connect incomings → finite
validation.**
The algorithm MUST NOT switch repeatedly between “single-value alias” and “Phi” during
propagation and MUST NOT perform trivial-Phi elimination; a few more Phis and locals buy a
simple closed loop.
Even a block with a single predecessor allocates live-in Phis, and a cross-block use uniformly
goes through the entry Phi instead of binding an OpResult of another block directly. Parameters
have virtual EntrySeed incomings; therefore an actual entry with a back edge can still
distinguish first-iteration and loop values correctly.

```text
Sym = Def(ValueId) | Phi(PhiId) | Bad(Undefined | PartialWide)
Phi { block, slot, result: ValueId, inputs: Vec<(IncomingEdge, Sym)> }
IncomingEdge = EntrySeed | Normal(EdgeId) | Exceptional(ExceptionEdgeId)
Env = sparse map<Slot, Sym>
```

Every parameter, OpResult, normal check-cast refinement and handler ExceptionAt is first
assigned a fixed ValueId. A pure numeric const also has a ValueId and original bits; P7 MAY spell
it per use. `move-exception` defines an independent exception value and is not conflated with the
ordinary incoming register of the handler. IDs are not reused and are not required to be equal
across runs.

### 3.2 live-in: the only data-flow fixed point required

The analysis first computes the structurally reachable set over normal and exception edges and
performs no path-predicate reasoning. Slot demand comes only from actual Op/terminator reads and
successor demand. A write that overlaps a slot kills the old slot, including the case where one
word of a wide value is overwritten; normal gen and exception-before-write are kept separate.

```text
LiveIn[*] = empty
worklist = reachable blocks
reverse scan of a block:
  live = Reads(terminator) union LiveIn[normal successors]
         union LiveIn[eligible handlers of a throwing terminator]
  for each op in reverse order:
    handler_live = union LiveIn[eligible handlers at this throw site]
    live = Reads(op) union (live minus SlotsOverlapping(NormalWrites(op))) union handler_live
  if live grows, update LiveIn[b] and enqueue its normal/exception predecessors
```

The set only grows, and its universe is the finite set of `(block, slot)` demands; each new
element and each scan step is charged first. Store sparse slot sets and do not build a
register_count×edge_count matrix first. The callback is checked on pop, on op scan and during
long set operations.

### 3.3 Single binding, connection and sealing

```text
for every LiveIn slot of every reachable block, create a fixed Phi and result ValueId
per block (any ready/traversal order, no stable output required):
  env = {slot -> entry Phi of this block}
  per op in original order:
    Uses(op) = read(env, requested slots)          # read all sources first
    if the op may throw, save the old env lookups that each handler LiveIn needs
    for each normal write:
      delete the slots in env that overlap the write
      env[write.slot] = the Def already pre-allocated for this op
  bind terminator reads; if the terminator can throw, save the old env its handlers need
  record the env lookups for the slots the normal successors need; do not clone the whole frame
per Phi and per incoming:
  take EntrySeed from the descriptor parameter/this; the rest from the corresponding normal/throw snapshot
  a missing slot is Bad; an overlapping but incomplete wide is PartialWide, and the high word is never guessed
  connect every eligible incoming exactly once; an unconnected incoming is an internal error, not a default 0
```

`read` prefers an exact slot; when that slot is absent but an incompatible overlapping slot
exists, it returns PartialWide, and otherwise Undefined. A block entry MAY record several
pending slot queries that are still to be validated; the final incoming MUST provide the correct
width for each of them. Every ordinary Op use MUST reference only the entry Phi of its block or a
preceding local Def; cross-block values appear only in Phi inputs.

Sealing uses two graph propagations instead of a ValueId fixed point:

1. Trace Phi backwards from all actual Op/terminator uses to obtain the retained Phi closure; an
   unused Phi does not take part in semantic error decisions or storage allocation, but the
   original instructions are still kept and every read of a reachable Op MUST still be validated.
2. Any Bad input of a retained Phi marks it bad, and the mark propagates along the reverse Phi
   user table; each bad mark happens at most once. An actual use that is directly Bad or depends
   on a bad Phi generates no Java. When a DEX definition/word constraint is definitely violated
   or there is actual reachable bad-read evidence, the result is Malformed (D057/D058); when only
   a conservative edge set prevents proving availability, the result is
   Unsupported/DefinitionAvailabilityUnproved, and unknown legality MUST NOT be misrepresented as
   damage.
3. Seed grounded from the Parameter/OpResult/ExceptionAt inputs of Phi and propagate along Phi
   users; a retained Phi without any traceable concrete definition is a rootless definition
   cycle, and MUST NOT be filled with a default initial value.
4. Check that normal inputs are the post-commit state and exception inputs are the old state of
   the corresponding opcode transfer; a normal dominator MUST NOT replace this check. The Java
   expressibility of typed/uninit values is left to P4/P4 and is not called Undefined.
5. After validation, release the Env and live work tables; keep the Phi inputs, the bound uses
   and the necessary edge facts for P4/P5.

The termination measure is explicit: liveness inserts at most finitely many demands; IDs are
assigned once; binding happens once per block; connection happens once per edge/Phi; and the
retained/bad/grounded marks each happen at most once. There is no repeated switching between
Phi and aliases, and “the budget ran out” MUST NOT be called convergence. POPL 2023 provides the
basis for the finite domain and naming; this scheme does not use its GVN union domain or borrow
its proof directly.

The validator independently replays the block from the raw read/write order and checks the
latest local definition, the Phi sources and the throw snapshot; it MUST NOT trust a bool
reported by the builder. D001/D002/D003/D029/D030 are public behavior evidence and do not freeze
private Phi counts or orders.

### 3.4 Equations, finiteness and complexity

Let `S` contain only the legal Slots that actual reads or successors need, and let Kill be the
slot set “overlapping the normal write interval”. The reverse transfer is
`Reads union (LiveAfter - Kill) union HandlerNeedsBeforeWrite`.
The Kill of the set difference is fixed and independent of the current iteration information,
so the transfer is monotone with respect to set inclusion; starting from the empty set only adds
elements. Each `(block,slot)` joins at most once, so termination does not depend on scheduling
order or on whether IDs are equal. That monotonicity does not authorize treating Unknown as
Undefined first; an unconnected incoming is still a construction-state Option.

The simplest implementation rescans the affected block each time, so the worklist has at most
O(B*|S|) effective growths in the worst case, and one scan still visits Ops and related demand
sets; the implementation MUST NOT claim that all of P3 is always linear. Set elements, edges and
inputs touched during the scan are also charged. The Phi count P does not exceed the final
live-in total, and the incoming count A is taken over real eligible edges; values, phi_inputs
and allocation budgets limit growth only under explicit configuration. Connection and the three
following graph propagations are O(P+A+uses) with extra storage of the same order; the
implementation MUST NOT pre-allocate an R×E matrix.

### 3.5 Item-by-item walkthrough of a diamond

```text
entry: if c goto L else R
L:     r = a; goto J
R:     r = b; goto J
J:     return r
```

The terminator of J first produces live-in(r), which propagates back to L/R and is killed by
their respective writes, which then produce the demand for a/b.
A fixed p is created for J/r; L and R bind to the entry parameter Phi and write the fixed dL/dR;
the two incomings of p are `(L->J,dL)` and `(R->J,dR)`. P5 executes `local(p)=local(dL/dR)` only
on the actual edge that selects L or R. Redundant Phis on the parameter chain are kept and do
not affect the relation. If both copies were placed at the entry, the later one would overwrite
the earlier one; if the return read of J were missed, p would not even be created. This shows
that “a terminator operand is also a read” is not an implementation detail.

### 3.6 Inductive invariants and error attribution

- Initial state: EntrySeed provides parameters and this, and nothing else has a definition out of
  nowhere.
- Inside a block: the k-th Op reads the latest env after the first k-1 normal commits; all
  sources are read before the writes.
- Edges: a normal input takes the env after the last commit; an exception input takes the env
  from before the execution of that site; the handler exception object is the new ExceptionAt
  definition and not an alias of an incoming reg.
- Loops: a back edge and EntrySeed are both fixed incomings, and all are connected at once; the
  implementation MUST NOT seal merely because the entry is the only incoming seen so far. A Phi
  local is reassigned on every entry; a static PhiId does not claim that the dynamic values of
  different iterations are equal.

The Phi closure is retained backwards from real uses so that a method is not rejected merely
because of an Undefined in a slot that is never read; however, “the result is unused” MUST NOT
justify skipping the actual inputs and effects of a reachable Op. grounded excludes only rootless
definition cycles and does not by itself prove that every path has a definition; Bad propagation
MUST also complete independently. Structural reachability is not a complete path-feasibility
proof, the implementation does not use SMT or path solvers, and unknown legality is classified
as described above rather than filled with 0. The validator replays the raw reads and writes and
the saved edge sources; it MUST NOT check only the same counters the builder produced.
