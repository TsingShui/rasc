# 02 — Flow and effects

Code: `crates/diana-decomp/src/p2_flow/`; entry: `normal.rs::decode`.

P2 consumes a checked P1 method input. It decodes reachable operations, binds
instruction results, identifies block boundaries and produces normal and
exceptional control-flow facts. Calls, fields, arrays, casts, allocation and
fill participate in the shared operation/effect representation.

Instruction and argument order, throwing sites and original code-unit identity
MUST be retained. A reader MUST NOT guess an unknown instruction width. Malformed
layout and resource or cancellation failures retain their classifications.

Current P2 declines some legal shapes as `JavaAnalysisUnavailable`; a decline
does not prove the original method is invalid. Its `JavaCheckFailed` diagnostic
can identify the failed check and, for instruction-local checks, its code unit.

## 2. Op and effect contract (T06/T10/T11/T15)

The representation uses three-address Ops whose operands are register slots or ValueIds and
MUST NOT store recursive expressions. `Slot={start:u16, width:One|Two}`; every read reads the
old value first, and only then do all normal writes happen, which supports overlapping
move-wide.

| Op | reads / normal writes | effect and main Java path |
|---|---|---|
| numeric const/move | none or a source slot / one slot | pure; const keeps bits, and move makes no type guess |
| int/long arithmetic, bitwise operations, conversion | operands / result | preserves width and overflow; div/rem may throw ArithmeticException, and the other operations add no exception through arithmetic |
| FP arithmetic/cmp | operands / result | IEEE semantics, no reassociation; cmp keeps the NaN bias |
| string/class const | pool ref / result | resolution/may-throw; it is not a pure numeric literal that may be copied freely |
| array/field/cast | receiver/index/value/ref / result or none | heap, null/bounds/type/resolution/access, volatile/init conservatively; an unused operation is not deleted |
| invoke | args in word order after the receiver / optional result | unknown read/write/call/throw/init conservatively, no DCE, no early or repeated evaluation |
| new/init | allocation and arguments / uninit then ref | AllocationId runs through, paired per §9, and the constructor MUST NOT be treated as an ordinary function |
| monitor | lock ref / none | synchronization; push only after a successful enter, and exit is handled by its special exception PC |
| branch/return/throw | selector/condition/value / none | Terminator; throw has no normal successor |

`RawOrigin` keeps the operation and the producer/result sites; every effect site has an explicit
landing point in the Java plan. The known format of invoke-custom[/range] and its
call_site/method_handle/proto metadata are fully validated and kept by the raw layer; the main
Java path is Unsupported/InvokeCustomUnsupported, and the implementation MUST NOT guess an
ordinary invoke or an unknown width. A rejection affects that method only, and the class can
still show other recovered members and the original debug information of that method.
The design has no optimizer: no heap CSE, no call-purity inference, no constant-field propagation
and no deletion of exception paths.
The handler selection for a `monitor-exit` exception follows Dalvik's “PC has advanced” rule and
takes the next PC; the pre-throw PC MUST NOT be applied uniformly. The accepted synchronized
profile separately proves that legal releases raise no lock-mismatch exception.

### 2.1 Unified transfer and the exact meaning of “old state”

Two different frame transfers are defined for an Op, instead of reusing one out environment for
all successors:

```text
args = read_all(sigma, Reads(op))
normal:      (result,H',events)=execute(op,args,H); sigma'=commit(sigma,result)
exceptional: (ex,H',events)=execute_throw(op,args,H); frame'=sigma
```

An exception only fails to commit the **target register of that Op**; it does not mean a heap
rollback. When a callee writes a field and then throws, catch observes both the old register and
the heap effect that already happened. The new type on the successful path of check-cast does not
enter the exception frame either. The throw terminator itself has no normal successor, but it has
operand reads and a possible handler transfer, so liveness and snapshots MUST NOT be skipped
merely because it is not in the ops array.

### 2.2 Order of wide values and overlapping writes

A Slot is treated as the half-open word interval `[start,start+width)`; two slots overlap if and
only if their intervals intersect. A wide read MUST obtain a complete pair of the same value
and MUST NOT independently combine two words that each have a definition but do not belong to
the same wide value. An Op reads all sources before invalidating overlapping old slots, so the
overlapping copy `move-wide v1,v0` does not lose its source. Conversely, in
`const-wide v0; const v1; return-wide v0` the high word has been overwritten, so the
implementation MUST reject it instead of continuing to read merely because the old ValueId still
exists; the independent fact of D058 is exactly this invariant.

### 2.3 No hidden Java evaluation

All heap/call/array operands are materialized in their original order first, and Java expressions
read only those locals and safe literals. `arr[i]=value` MUST NOT become `arr[i]+=call()`
incidentally; automatic boxing/unboxing, compound assignment, a repeated receiver or merging
several volatile reads can add or move exceptions and accesses. An unused field/array/cast/div
still emits a legal typed assignment that preserves its execution; a call with no result still
emits the call. Primitive narrowing is explicit semantics of the original Op or the original
store, not a conversion added arbitrarily so that a Phi copy “can compile”.

### 2.4 Deterministic expansion of fill-array-data

This Op writes the payload into an **existing primitive array** and is always registered as
may-throw; the implementation MUST NOT delete its check because a third-party opcode table
misses CAN_THROW. P1 checks the payload range/count/element_width; P5 additionally proves that
the component matches the width: Z/B=1, S/C=2, I/F=4, J/D=8. A known reference array does not
satisfy that DEX constraint; when the information is insufficient, the implementation MUST NOT
guess int[] or float[] from the width.

P8 constructs only the following two FillPlans; every other case is Unsupported/ArrayFillShape
and keeps raw:

| Condition | Java expansion |
|---|---|
| count=0, with a known primitive component/width | execute `int unusedLength = a.length;` at the original site, with no store |
| count>0, satisfying the FreshArray proof below | read `a.length` at the same position first, then emit typed stores a[0]..a[count-1] in the original order |

The FreshArray proof MUST contain:
1. a comes uniquely from new-array, its length is a proven non-negative integer constant L with
   L>=count, the original allocation position is kept, and fill and new MUST NOT be merged into
   a new expression that would evaluate element computations earlier or move exception
   positions.
2. From the normal completion of new to fill there is an acyclic single-entry normal chain. The
   array value passes only through move-object or a pass-through Phi with a single incoming; no
   external normal entry or back edge lets another instance enter fill.
3. The chain allows only numeric consts, the alias moves above and pure control passthrough; it
   contains no call, heap access or publication, monitor or other may-throw operation. The array
   has not yet been written to a field or another array, passed as an argument or returned; all
   aliases are included in the check, and the implementation MUST NOT look only at the current
   physical reg.
4. Each element MUST be expressible exactly per §4.8. The Z payload is limited to 0/1, B/S are
   signed, C is unsigned; F/D are interpreted by their original bits and keep -0/Infinity; a NaN
   payload that cannot be preserved exactly by the specified Java literal is rejected for Java.

```text
lower_fill(op, facts):
  require known primitive component and matching width
  if count>0: require FreshArray(op.array, length>=count)
  require every payload element has exact Java literal
  charge all length-temp/store/literal nodes before construction
  emit unusedLength = arrayLocal.length at original protection context
  for i in 0..count: emit arrayLocal[i] = exact_literal(payload[i])
```

count=0 still keeps the length read, so a null argument still throws NPE and the call does not
return normally merely because the loop runs zero times. The accepted form with count>0 is
already proven non-null with sufficient length, so the primitive stores raise no
NPE/bounds/ArrayStore exception; the array has not escaped and the expansion contains no call or
effectful operand, so the intermediate writes are not observable by the program. Afterwards the
first count elements equal the payload, the tail keeps its original default values, and the
register store is unchanged.

An ordinary argument with insufficient length MUST NOT be expanded element by element: it may
change the first few elements and then throw, which contradicts the check/write contract of the
original bulk operation. The implementation MUST NOT synthesize exception objects, add helpers,
insert new library calls or build a general array recovery here. FillExpansion uniformly owns
the original site, with the length check and the stores as its ordered children, so a coverage
counter does not mistakenly count them as several executions of the original fill; the snapshot
is taken before the whole expansion. D027 is a positive example whose int/float fresh-array
shape and payload MUST be proven item by item by an independent input certificate; the
implementation MUST NOT infer from Java literal text that the original DEX contained a fill.
Runtime cost is O(count), AST and constants are charged by actual growth, only explicit quotas
are checked, and the JVM code cost MUST NOT be predicted.
The payload itself borrows the original bytes and MUST NOT be copied into a huge Vec first.

### 1.6 Atoms, leaders and results across try boundaries

`Atom { raw_sites, reads, normal_defs, throw_site, protection }` is a P2-private construction
representation and MUST NOT add a public seam. Raw adjacency and the ban on branching into
move-result are validated first, and only then is the result merged into the producer. The
protection chain is queried at the original pc of the genuine throw_site, not at the last raw
site of the atom.

If a raw try starts or ends exactly before the move-result, the implementation MUST NOT both
force that pc to be a leader and treat the result as a separate throwable operation. It commits
only for this proven pure result, and projects the **graph split point** to after the atom; the
original try range, raw metadata and original pc stay unchanged. If any other real operation or
target would be crossed, the implementation MUST reject that fusion instead of moving it.
Therefore call-inside-try with result-outside-try and call-outside-try with result-inside-try
both have an exact throw protection chain.

The execution order is fixed: read receiver/args → save the required old frame → execute the
producer → commit the result on success. The assignment MUST need no boxing and no new downcast;
otherwise “commit does not throw” MUST NOT be used as a derivation. This rule also explains
D029: even when the target reg has the same number as the old variable, catch MUST read only the
value from before the failure.

### 1.7 CFG construction and bad tails

1. Record the boundary table and payload ranges over the complete raw stream, and validate all
   explicit targets and refs even when their sites are unreachable.
2. Build atom boundaries and the raw-pc-to-atom mapping; try splitting follows §1.6, and handler
   entries MUST still be legal entries.
3. Compute structural reachability from the entry along candidate normal and exception edges.
   Raw data or dead code may follow a terminator, but a reachable atom that can continue executing
   MUST have a legal next; the implementation MUST NOT invent a return or nop when there is no
   next.
4. Publish Block and Edge only for reachable atoms; every protected may-throw/throw terminator
   has its own exception site, and the chain order is unchanged. An uncaught exception is an
   abrupt exit, and the implementation MUST NOT create a pseudo handler node.

The base cost of the scan, boundary validation and reachability is O(I+E+X) with storage
O(I+B+E+X); variable-length metadata and payloads are charged separately by actual elements.
Charging happens before edge/leader insertion and queue operations. An unknown width stops the
first pass: even if a later word looks like a return, it MUST NOT restore a trustworthy
instruction boundary.

## 5. Identity, origin and product invariants

#### 5.1 IDs and immutable raw

"Fixed/stable identity" in this section means only that **references are consistent within a single analysis and arena IDs are not reused**; it does not mean result ordering or determinism across runs. Different worker arrival orders may generate different synthetic IDs and local names, and the verifier MUST check every reference/certificate/final span against this request's identity.

```rust
struct DexId(u32);
struct ClassKey { dex: DexId, class_def_idx: u32, descriptor: String }
struct MethodSelector { name: String, proto: String }
struct MethodKey { dex: DexId, method_idx: u32 } // a method_id uniquely resolves to owner/name/proto
struct FieldKey { dex: DexId, field_idx: u32 }
struct InsnId(u32);    // decode order within the method, never reused
struct BlockId(u32);   // identity is not reused after a split in the same request; numbers are not guaranteed to be equal across runs
struct EdgeId(u32);    // two different switch/exception edges to the same block are not merged
struct ValueId(u32);   // definition identity, not equal to a physical register
struct LocalId(u32);   // source-variable identity, one LocalId per ValueId by default
struct Site { method: MethodKey, code_unit: u32 }
struct Origin { sites: Vec<Site>, synthetic_reason: Option<SyntheticReason> }
```

`RawMethodView` proves only that recorded original ranges can be read safely. P1 validates format, operand, reference, target and metadata facts before using them in Java analysis. The debug exit may retain individually checked facts when the complete method cannot be validated, with explicit coverage and missing ranges; it does not certify ART validity. Later definite invalidity remains Malformed/Unavailable with useful original information, never a successful Java or valid-program claim.

The offset unit MUST be expressed in the type/field name: Diana MUST NOT mix DEX code_unit with file_byte, or a UTF-16 code unit with an output UTF-8 byte. A debug local name is only a hint and MUST NOT decide identity or type. The original bytes are held by the root request scope, and P1 borrows a DexImage whose header was not rewritten; P1 MUST validate and record the method's code/try/debug ranges independently. Diana MUST NOT reassemble the supposed original input with a third-party `Instruction::bytes()` [DX-11].

Each DEX instruction MUST be consumed by one Op/Terminator or explicit metadata (payload); a move-result is covered by the producer across both sites, a goto is covered by the terminator, and the pipeline MUST NOT silently delete the instruction as a "Nop". Only a real NOP is marked NoEffect; an unsupported instruction that cannot be expressed in Java may still fall back to raw in its known format.

#### 5.2 CFG and effects

`FlowBody` owns the blocks and the ordered ops; the block terminator is separate. normal edges are Goto/Branch/Switch/Return; exceptional edges are `(throw_site, handler_order, handler_target, catch_type)`, and P2 MUST NOT add them to the normal successor list of the ordinary dominator algorithms. P3 MUST consume the normal and exception graphs separately.

A protected may-throw op MUST split the site: the last op of its block is that possibly throwing operation, and the normal result is visible only in the normal successor. The handler chain MUST retain the original try-table ranges, the typed catch order and the catch-all as the last item; P2 MUST NOT eliminate priority by sorting on type or target. P2 MUST build an exceptional edge that accepts the pre-error state and MUST NOT commit the default result; if a particular opcode has a special rule, P2 MUST build an explicit transfer and MUST NOT use one rule to guess every instruction.

The `Effect` table is driven by fixed opcode metadata: heap read/write, call, may_throw, monitor, class_init. An unknown external call means full conservatism; P2 MUST NOT delete a field/array read even without a side-effect write, because it may throw, be volatile or trigger init. Integer div/rem may throw, while float div/rem does not throw for that reason. P2 MUST NOT reassociate FP arithmetic; the printer MUST still emit a temporary assignment for an unused computation by default. A pure local copy/constant may be used for register shuffling, and it MUST NOT introduce a checked cast.

#### 5.3 SSA scheme

[03-ssa](03-ssa.md)'s **fixed-entry Phi construction** applies:

- `Slot={start,width}`; a wide value is read as one slot, the builder MUST read every source before the normal writes, and a write MUST invalidate overlapping old slots.
- The builder MUST compute the normal/exception live-in requirements first, then allocate a Phi for every live-in slot of every block (a single predecessor is kept as well). A virtual EntrySeed supplies the parameters/this, so the entry back edge also has a correct first value.
- The builder MUST scan each block once, bind uses from the entry Phi and the earlier Defs of this block, and connect each normal/exception incoming once. An exception MUST use the old environment of the corresponding site, not the successful result.
- The builder MUST retain the Phi closure from real uses, MUST propagate Bad monotonically along Phi users, and MUST propagate grounded monotonically from parameters/OpResult/ExceptionAt. The builder MUST remove unused Phis and MUST reject an actual missing definition, a partial wide or an unrooted definition cycle explicitly.
- **The builder MUST NOT eliminate trivial Phis and MUST NOT do alias reduction**. Every state/traversal MUST have a finite measure and be charged; the builder MUST drop the construction Env work table after sealing and MUST NOT carry the R×E matrix all the way to P8.
- The verifier MUST replay the locally latest definition from the raw read/write order and the edge transfer, MUST check the Phi sources, the word width and the old exception state, and MUST NOT trust a builder bool or a normal dominator.

Complex exception SSA is a phase risk; the phase MAY support no handler first, then a single protected invoke, then widen. The phase support matrix MUST explain the degradation reason, and it MUST NOT claim that general SSA is complete while ignoring the exception environment.
