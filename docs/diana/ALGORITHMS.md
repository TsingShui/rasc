# Diana core algorithm technical specification

This document specifies the algorithms, state transitions and rejection conditions for DEX
decoding, analysis and Java/debug representation.
Interfaces and ownership are defined in [DESIGN.md](DESIGN.md); validation rules are in
[TASKS.md](TASKS.md).

## 0. Semantic model and correctness conditions

**Rules** define the algorithms; **derivations** state their correctness preconditions.
Independent execution evidence defined by TASKS MUST NOT be replaced by a derivation.
The arguments below are not machine-checked proofs.

### 0.1 Objects and states

| Notation | Definition | MUST NOT be conflated with |
|---|---|---|
| `N` / `pc` | physical byte count / code-unit position within the method | `FileByte`, `CodeUnit`, UTF16 and source UTF8 byte |
| `I,B,E,X` | original instruction count, reachable block count, normal edge count, exception edge count | different edges to the same target are still different choices |
| `R,S,P,A` | register words, requested Slot set, retained Phi count, total Phi incoming count | a register is not a ValueId or LocalId |
| `sigma` | runtime original register store, a wide value being two related words | undefined is not 0, uninit is not null |
| `lambda` | typed locals store of the generated program | an initial value added by DA is not a semantic definition |
| `H,K` | heap / current lock stack | a local copy MUST NOT pretend to be a heap operation or a lock operation |
| `tau` | ordered observable trace of calls, reads and writes, exceptions, synchronization and initialization | the display order of an independent analysis report does not belong to tau |

A ValueId is a **static definition site**, not a runtime value that stays constant across loop
iterations. A proof MAY write `value(v, activation)` to distinguish the instances that one
Op/Phi produces on two block entries; the implementation does not store an activation
counter. After each entry Phi assignment or Op execution, lambda represents the current
instance rather than the set of historical instances.
This is also why §11 checks, beyond the same root, that the root producer does not re-execute
while the lock is held.

### 0.2 Preservation relation and applicability preconditions

The block-entry relation `Rel(b,sigma,lambda)` holds when, for every live-in slot of b, the
word value of its Phi local interpreted by the source type equals sigma, and the Defs
already executed earlier in this block also correspond to the current instance. The observable
parts of the applied heap and locks agree, and the original event prefix tau agrees.
Non-live slots, unread DA initial values and pc/temp are not required to be equal.
A pure literal is interpreted through a chosen view of its original bits; it is not required
to have a local.

The goal is: every original normal or exception step is simulated by finitely many Java
steps; added steps only read and write unobservable locals; the same edge is selected, the
same observable events are produced, and the subsequent Rel holds. The reverse direction
MUST NOT add observable paths either; the verifier MUST prove that the illegal-pc branch of
the dispatcher and the false branch of the static-initialization `if(true)` are unreachable.
Finite copy and scheduling steps MUST NOT stutter indefinitely and make an originally
terminating method diverge. A finite Driver provides evidence for the tested inputs only
and does not replace this argument.

The preconditions include a matching declaration and linking environment, the selected Java
semantics profile, and no implementation-related VM resource failure being triggered.
Asynchronous exceptions and actual OOM/GC/stack-frame differences lie outside the
per-instruction simulation guarantee of this document; that exclusion does not authorize
moving an original allocation, may-throw, volatile access, lock or class-init. Synchronous
exceptions and the original protection domains are still preserved site by site. Evidence
from JVM21 does not automatically become a guarantee for historical JVMs or for all ART
behavior; in particular, a single-thread trace MUST NOT be called a complete JMM proof.

### 0.3 Decision trade-offs

| Decision | Reason for choosing it | Explicitly not adopted |
|---|---|---|
| original addresses + borrowed bytes | one coordinate system verifies original identity and range, and 041 shared data stays undistorted | rewrite the header and then call the copy raw truth |
| split the CFG after atoms | leader rules cannot split a producer/result into a wrong definition timing | expose a successful result to catch |
| fixed entry Phi | separates the identity problem from information propagation and yields an easy finite measure | rewrite aliases and Phi alternately until they “look unchanged” |
| choose the family first, then fix the source type | separates bit interpretation, nominal type and use-conversion duties | one Unknown variable doing join, guessed cast and printed type at the same time |
| independent locals + parallel copy | sacrifices compactness to avoid coalescing interference and lost old values | reuse a Java local per physical reg |
| explicit transfer and lexical scope | copy, catch and lock can be checked per edge and per site | coverage count equals a semantic proof |

## 1. Reader, format tables and raw instructions (T02–T04)

### 1.1 Address space and read functions

`Cursor` holds `bytes: &[u8], pos: usize, end: usize`; every read returns a Result, and the
implementation MUST NOT index first and check afterwards. `range(off,n,stride)` verifies through
`n.checked_mul(stride)` and `off.checked_add(size)` and only then takes the slice.
Fixed records keep a borrowed view; a variable-length record that needs decoding first verifies
the remaining bytes and the minimum record size, charges the budget, and only then calls
try_reserve. Offsets use `FileByte` and `CodeUnit` separately; a call site MUST NOT multiply by
2 inline.

- Accept the defined 035/037/038/039/040/041 containers; 036 and unknown versions are
  UnsupportedContainer. Version recognition does not mean that every Java opcode of that version
  is supported. reverse-endian is Unsupported.
- 035–040 require header_offset=0, header_size=0x70, and file_size equal to the physical DEX
  length of this read.
- 041 requires header_size=0x78; the 0x70/0x74 fields are container_size/header_offset
  respectively. Advance from 0 by each header's file_size with checked arithmetic and end exactly
  at container_size; every header has a container_size equal to the physical length and a
  header_offset equal to its actual position, and the step is at least 0x78.
- The section/data/code/debug offsets of 041 are absolute offsets from the **physical container
  start**. They MAY reference data of a later logical member and MUST NOT reference data before
  the current header. The implementation MUST NOT truncate shared data by logical file_size and
  MUST NOT add header_offset twice. data_size/data_off are not used as a data-region boundary in
  041.
- Every input view is the original container plus header_offset. The implementation MUST NOT
  overlay the header and MUST NOT recompute “original bytes”. It verifies the physical header
  chain once and builds the catalog of a logical member only after selecting that member; the same
  physical slice MUST NOT count twice toward input bytes.
- Keep header checksum and signature as original fields only; they are not a memory-safety proof
  and do not claim complete ART file authentication. The fixture content lock and the ZIP
  extraction CRC are independent contracts and are not cancelled by this.

#### Header and 041 section checks

All offsets in the following table are bytes relative to the current header;
stored section offsets address the physical container, not this table.

| Header byte offset | Field / interpretation |
|---|---|
| `0x00 / 0x08 / 0x0c` | magic[8] / checksum u32 / signature[20] |
| `0x20 / 0x24 / 0x28` | file_size / header_size / endian_tag, each u32 |
| `0x2c / 0x30 / 0x34` | link_size in bytes / link_off / map_off |
| `0x38 / 0x40 / 0x48` | string_ids / type_ids / proto_ids: u32 count followed by u32 offset |
| `0x50 / 0x58 / 0x60` | field_ids / method_ids / class_defs: the same count/offset pair |
| `0x68 / 0x6c` | data_size / data_off; unused as 041 data-region bounds |
| `0x70 / 0x74` | 041-only container_size / header_offset |

For 041, each non-final header step is four-byte aligned. Every counted section
has both size/count and offset zero, or both non-zero. A non-empty section lies
after the header that defines it and within the physical container. References
reached transitively through shared tables obey that same lower bound for every
logical member using them. The reader MUST check the complete item range, not
only its first byte; a forward start with an end beyond container_size is invalid.

The reader MUST apply the DEX format/constraints alignment per item type. Header,
ID tables, class_defs, map_list, type_list, annotation sets/directories and
code_item use four-byte alignment; byte-encoded strings, debug, annotations,
class_data and encoded arrays do not acquire an invented four-byte restriction.
Inside one member's map, types occur once, entries follow physical offset order,
and actual item extents do not overlap. Variable item extents come from checked
parsing, not the count times a guessed stride. The map's header entry describes
this header with its version-specific size; ID entries agree with this member's
header. A later header is not data that an earlier code/string item may swallow.

Different logical members may share the same permitted data range; a global
"all member ranges disjoint" check would reject legal sharing. Sharing bytes
never shares an index namespace: refs inside a method are resolved only against
its selected member's tables. The reader MUST NOT merge declarations by equal
numeric index or descriptor. Unknown container versions are UnsupportedContainer;
a recognized 041 with a broken chain, illegal range, alignment or conflicting
map is Malformed. These checks follow the published DEX format and constraints,
not the assumptions of a 035 parser with an overlaid header.

Fixed table strides: string_id/type_id=4, proto_id=12, field_id/method_id=8, class_def=32.
The reader first verifies map_list's `4+12*n`, type uniqueness, the offset order required by the
specification, and consistency of the fixed tables with the header; the reader MAY read
references only after that. The type and proto count limit is 65535; other tables are constrained
by the actual
range, index width and resource quotas, and the reader MUST NOT cast a u32 count to u16 first.
After the four class_data counts it accumulates the field and method ULEB deltas separately;
each group starts from 0, uses checked_add, and publishes declarations only after validation of
the index, the owner it belongs to and duplicate definitions.

ULEB32 spans at most 5 bytes; the 5th byte MUST NOT continue the encoding and its payload is
<=0x0f. The 5th SLEB32 byte payload MUST be 0x00..0x07 or 0x78..0x7f. Accumulate in u64
first, then sign-extend and range-check; do not take abs on MIN. The reader MUST NOT add a
“shortest encoding required” restriction that the specification does not require.

`code_item`: 16-byte header → insns_size*2 → 2-byte padding when insns is odd and tries>0 →
8*tries → encoded_catch_handler_list. handler_off is relative to the **handler list start**;
branch and payload are relative to the original instruction PC; handler size is handled as i64
for negative values. Try ranges are ordered and non-overlapping, start and end on valid
instruction boundaries, and their targets are not payloads. code_off=0 for abstract/native
produces a declaration only and does not run the method pipeline.

MUTF-8 produces `Vec<u16>`: 00 terminates, C0 80 encodes NUL; one/two/three-byte units are
verified per the specification, surrogate units are kept, and the ordinary UTF-8 four-byte path
MUST NOT be accepted in place of this encoding. The declared utf16 length MUST match exactly.
String-pool literals may hold isolated surrogates; descriptors and names are additionally
validated against DEX declaration syntax, and the implementation MUST NOT look them up as
members after a lossy conversion. Display, Java literals and original identity are three
different purposes.

### 1.2 Format bit fields and the complete opcode registry

Let `w0..w4` be u16 code units; `a8=w0>>8`, `a4=(w0>>8)&15`, `b4=w0>>12`.
`s8/s16/s32` denote interpretation of that width as signed; 32/64-bit words are always
concatenated little-endian.

| Format | code units | Decoded fields |
|---|---:|---|
| 10x / 12x / 11n / 11x / 10t | 1 | no operand and high byte 0 / A=a4,B=b4 / A=a4,lit=s4(b4) / A=a8 / delta=s8(a8) |
| 20t / 22x | 2 | high byte 0,delta=s16(w1) / A=a8,B=w1 |
| 21t / 21s / 21h / 21c | 2 | A=a8, and respectively delta=s16(w1), lit=s16(w1), high-bits=w1, index=w1 |
| 23x / 22b | 2 | A=a8,B=w1&255,C=w1>>8 / lit=s8(w1>>8) |
| 22t / 22s / 22c | 2 | A=a4,B=b4, and respectively delta=s16(w1), lit=s16(w1), index=w1 |
| 30t / 32x | 3 | high byte 0,delta=s32(w1|w2<<16) / high byte 0,A=w1,B=w2 |
| 31i / 31t / 31c | 3 | A=a8, and respectively bits32, signed payload delta, unsigned jumbo index, assembled from w1/w2 |
| 35c | 3 | count=b4<=5,G=a4,index=w1; C/D/E/F are the four nibbles of w2 from low to high, and the actual registers are the first count items of [C,D,E,F,G] |
| 3rc | 3 | word_count=a8,index=w1,first=w2; when count>0, check first+count<=registers_size with checked arithmetic |
| 45cc / 4rcc | 4 | follow the 35c/3rc layout respectively, with w3 additionally holding the proto index; 038+ |
| 51l | 5 | A=a8,bits64=w1|w2<<16|w3<<32|w4<<48 |

The opcode registry expands over the following closed ranges; one `OpcodeSpec` holds the format,
version, mnemonic, ref kind, operand shape, read/write slots, effect and Java lowering kind,
and the decoder and raw printer read only this table:

| opcode | format / meaning |
|---|---|
| 00 | 10x nop; 0100/0200/0300 complete words are additionally recognized as payload signatures |
| 01..09 | move / move-wide / move-object, three each, in order 12x, 22x, 32x |
| 0a..0d / 0e / 0f..11 | 11x result/result-wide/result-object/exception; 10x return-void; 11x return/return-wide/return-object |
| 12..15 | 11n const/4, 21s const/16, 31i const, 21h const/high16 |
| 16..19 | 21s const-wide/16, 31i const-wide/32, 51l const-wide, 21h const-wide/high16 |
| 1a..1c | 21c const-string, 31c const-string/jumbo, 21c const-class |
| 1d..1f / 20 / 21 | 11x monitor-enter/exit; 21c check-cast; 22c instance-of; 12x array-length |
| 22..27 | 21c new-instance, 22c new-array, 35c/3rc filled-new-array[/range], 31t fill-array-data, 11x throw |
| 28..2c | 10t/20t/30t goto[/16,/32]; 31t packed/sparse-switch |
| 2d..31 | 23x cmpl-float/cmpg-float/cmpl-double/cmpg-double/cmp-long |
| 32..37 / 38..3d | 22t if-eq/ne/lt/ge/gt/le; 21t the corresponding if-*z |
| 44..51 | 23x the seven aget forms / the corresponding seven aput forms: ordinary, wide, object, boolean, byte, char, short |
| 52..5f / 60..6d | 22c iget/iput for the same seven kinds; 21c sget/sput for the same seven kinds |
| 6e..72 / 74..78 | 35c virtual/super/direct/static/interface; 3rc the corresponding range forms |
| 7b..8f | 12x neg/not and primitive conversions, registered in the order of the Dalvik bytecode name table |
| 90..af / b0..cf | 23x binary and 12x /2addr; 11 kinds each for int/long, 5 kinds each for float/double |
| d0..d7 / d8..e2 | 22s /lit16: add,rsub,mul,div,rem,and,or,xor; 22b /lit8 additionally shl,shr,ushr |
| fa/fb/fc/fd | 038+: 45cc/4rcc invoke-polymorphic[/range], 35c/3rc invoke-custom[/range] |
| fe/ff | 039+: 21c const-method-handle, const-method-type |

The unary 7b..8f order is neg-int,not-int,neg-long,not-long,neg-float,neg-double,
int-to-long/float/double,long-to-int/float/double,float-to-int/long/double,
double-to-int/long/float,int-to-byte/char/short. The binary int/long order is
add,sub,mul,div,rem,and,or,xor,shl,shr,ushr; FP has only add,sub,mul,div,rem.
long/double operands and results are Two; the count of a long shift is still One; a cmp result
is always One; a conversion takes its two widths from its name, and /2addr reads the old A and B
before writing A instead of being treated as reading B only.

The remaining 035 reserved/unused values MUST NOT have their width guessed and MUST NOT be
treated as nop; optimized and quickened formats MUST NOT be mixed into the standard table.
const/high16 shifts left by 16 bits and wide/high16 by 48 bits; wide/16 and wide/32 sign-extend
before becoming 64 bits. The count of 35c/3rc is a **word count**, not an argument count; a wide
argument MUST occupy two adjacent words.

Payload lengths: packed=`4+2*n`, sparse=`2+4*n`, fill-array=`4+ceil(element_width*n/2)`.
Compute them with checked integers first; a payload MUST be 4-byte aligned, the array width
accepts only 1/2/4/8, and trailing padding of odd bytes is not an element. Switch keys MUST
increase and be unique per the specification, and MUST NOT be reordered to repair them; every
target is relative to the switch PC that references it.

Each registered family has one lowering classification: scalar/move/conversion
and comparison use §4; branches/switch/return use §6; heap/fill use §2.4 and §9;
call/allocation use §9; throw/catch/monitor use §10–§11. nop remains a raw site.
move-result/move-exception are consumed by the binding rules of §1.3/§2, not
printed as independent Java assignments before their producer succeeds. Known
newer raw-only families follow §2 and §8. A classification dispatches to those
rules; it does not authorize lowering without their type/effect/binding checks.
The implementation MUST NOT keep a second width/mnemonic table in the printer or
scanner; The analysis-oriented debug snapshot is defined in §7.3.

### 1.3 Two-pass validation and CFG splitting

```text
pass1:
  each step first runs charge/check_cancel, then reads the complete format/payload with checked arithmetic
  record InsnStart/PayloadStart and RawInsn(original words, operands, refs, origin)
pass2:
  check register spans, index kinds, branch/payload targets and alignment
  check try/debug boundaries, handler entries, invoke word/proto and result/exception adjacency
  seal ValidatedRawMethod only after everything passes
```

Unreachable code is also validated for width and explicit references; the implementation MUST
NOT hide truncation with DCE. An explicit branch into the middle of an instruction is always
rejected; fallthrough into a payload or past the end of the code is a reachable-execution check,
and dead padding after a return MUST NOT be misjudged as an actual execution overrun. raw keeps
every decoded instruction, while the published FlowBody contains only the reachable part. The
delta of goto/goto16 and if-* MUST NOT be 0, and goto32 MAY be 0; a legal self-loop MUST NOT be
rejected wholesale. CFG leaders are the entry, jump targets, the point after a terminator, try
boundaries, handler entries, and the point after a protected may-throw operation. Every
protected may-throw is the last operation of its block, and normal and exception edges are kept
separate; the same target under different cases or ranks still yields different EdgeIds.

invoke/filled-new-array and the immediately following move-result are first bound into one
semantic atom, and only then is the CFG split along atoms, keeping both raw sites; the result
writes its target register on the normal path only, and discarding a result still keeps the call.
The implementation MUST reject a result kind/return kind mismatch, a branch into move-result, and
an illegal move-exception entry. The original protection domain covers only genuine may-throw
sites; Java operand evaluation and other calls MUST NOT be widened into the try.

### 1.4 Deterministic rules for metadata decoding

- The low 5 bits of an encoded_value header are the tag and the high 3 bits are value_arg.
  byte/short/int/long and the corresponding reference indices are read at the allowed width and
  integers are sign-extended; float/double restore the original bits by right-padding with 0 and
  undergo no numeric conversion. The arg of array/annotation/null MUST be 0; the arg of boolean is
  0/1 with no following payload.
- annotation and array use an explicit bounded stack; each level first charges the element count
  and depth, and reference indices are checked against their own tables. Debug decoding reads
  line_start and parameter names with ulebp1; NO_INDEX does not become pool index 0.
- Debug opcode 0 ends, 1 is advance_pc, 2 is advance_line, 3/4 are start_local[/extended], 5 is
  end_local, 6 is restart_local, 7/8 are prologue/epilogue, 9 is set_file; >=0x0a updates by
  `adj=op-0x0a; pc+=adj/15; line+=-4+adj%15`. Addresses and lines are checked, and local lifetimes
  are valid.
- When metadata cannot be decoded, report the actual failure and missing range without
  discarding other checked code/declaration facts. Debug coverage MUST NOT claim that the
  missing information was decoded; header damage and method-local failures stay distinct.

### 1.5 Derivation and execution order of range checks

For `n` records with stride `s`, the valid condition is `off<=N` and `n<=(N-off)/s` (s>0).
The implementation first computes the division-form upper bound or checked multiply-add and only
then takes the slice; it MUST NOT compute the possibly overflowing `off+n*s<=N` first. A record
count that satisfies the minimum byte bound is only a necessary condition: every record is then
checked for its variable-length tail and references. Reads do not allocate; when a Vec is needed,
the implementation checks the input facts, the semantic count and the resource quota first and
then calls try_reserve, and none of the three substitutes for another.

If the minimum byte count of an already read field directly proves an overrun of the remaining
bytes, record Malformed first; the implementation MUST NOT report BudgetExceeded instead merely
because the same count also exceeds the analysis limit. A cancellation or a request-admission
resource failure before that field has been read still returns its own reason, and parsing MUST
NOT be forced to continue for diagnosis.

```text
read_table(off, n, stride):
  reject stride==0 or off>N
  reject n>(N-off)/stride
  end=checked_add(off, checked_mul(n,stride))
  return bytes.get(off..end) or Malformed
```

This is a local memory-safety derivation; it does not prove that map or type semantics are
correct. 041 has three further intervals: physical `[0,N)`, logical span `[h,h+file_size)`, and
the later physical range that the current member may reference; the span is the header-walk
interval, not an upper bound for referenced data. For example, with h=0x100, file_size=0x80 and
N=0x500, data at 0x400 may be legal, and 0x80 is not forward data that this member may reference.
Section type, alignment and overlap rules are still checked separately, and another header MUST
NOT be treated as string or code merely because it lies inside the physical range. All addresses
keep their original values; the implementation MUST NOT add h or copy shared regions to correct
them.

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
assigned a fixed ValueId. A pure numeric const also has a ValueId and original bits; P8 MAY spell
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
   expressibility of typed/uninit values is left to P4/P5 and is not called Undefined.
5. After validation, release the Env and live work tables; keep the Phi inputs, the bound uses
   and the necessary edge facts for P5/P6.

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
the two incomings of p are `(L->J,dL)` and `(R->J,dR)`. P6 executes `local(p)=local(dL/dR)` only
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

## 4. Types and Java expressions (T10/T11/T15)

### 4.1 Domain, constraints and solving order

P4 supplies declaration and proto constraints first, P5 then solves value and use types, and P8
finally checks the source binding. Definition identity is not rebuilt because a type changed.

```text
Family = Integral32 | Float32 | Integral64 | Float64 | Reference
ValueFacts { width, family_group, producer_origins, integral_bits, integral_range, use_requirements }
UseConversion = Identity | LiteralView | PrimitiveCast | ProvenNarrow
              | BooleanToInt | Proven01ToBoolean | ProvenUpcast | OriginalCheckCast
```

The implementation MUST NOT use one variable that simultaneously “narrows the type, widens the
join and replaces Unknown”. It uses the separated algorithms of §4.3–§4.5: first close the
representation-family constraints, then monotonically propagate producer and value facts, and
finally choose the source type once and check every use. The availability of uninit is given
separately by §9 and MUST NOT be mixed into the ordinary Reference family where it would
disappear.

A pure numeric const keeps only bits and width, and **selects its view per use**; a string/class
const with effects and a non-literal MUST NOT be duplicated per use. P4 supplies fixed
declaration and proto facts and does not require knowing P5's final source type in advance.
A sealed source type is not reselected in P6/P8 for printing convenience; a failed check MUST
NOT be repaired with an arbitrary cast.

After solving, the source type is selected by the following rules, without depending on the
first use or on HashMap order:
- A determined producer type is kept with priority; byte/short/char are promoted to int as Java
  arithmetic operands and narrowed explicitly at the write-back site.
- boolean and int are kept separate. A known boolean that needs an int view becomes `b ? 1 : 0`;
  the reverse direction accepts only proven 0/1 or an original boolean definition, and the
  implementation MUST NOT write an arbitrary int as `x != 0` to hide a type conflict. One finite
  Known01 fact is kept: boolean parameters, 0/1 literals, known boolean bitwise operations and
  `x & 1` can prove a 0/1 result; a non-literal int MAY be converted to boolean by `x != 0` only
  with that evidence. §4.9 propagates producer bounds only for lossless narrowing and performs
  no arithmetic range inference.
- A Phi needs one representable result type; primitive/ref values or incompatible 32/64-bit
  interpretations MUST NOT be mixed into Object. A literal MAY select different LiteralViews in
  different Phis and uses, while a non-literal MUST NOT disguise a numeric cast as a bitcast.
- References MAY be upcast without throwing only given complete subtype facts; an unknown
  hierarchy MAY use Object to hold the join, but any narrower use still needs a proof of no added
  runtime check, and otherwise is Unsupported.
- An initialized Ref differs from Uninit; AllocationId/UninitThis MUST NOT enter an ordinary
  Object Phi or a default null.

After sealing, every retained Value and use has a type, a conversion or an explicit failure, and
P6 no longer guesses types on its own.

### 4.2 Required numeric lowering

| Original semantics | Java expression adopted |
|---|---|
| int/long add/sub/mul, bitwise operations, shift | the corresponding Java operation; the original result width is not promoted; a long shift count is an int and the mask defined by Java is kept |
| rsub literal | `literal - value`, and the implementation MUST NOT write value-literal by mistake |
| byte/char/short conversion | `(byte)x` / `(char)x` / `(short)x`; the result is stored at the original definition site and not recomputed at a later use |
| div/rem | the corresponding `/` and `%`; a typed temp is assigned even when the result is unused; MIN/-1 and division by zero are preserved by Java semantics |
| DEX bitwise operations involving boolean | when needed, obtain an int carrier with `b?1:0`, then use `&`/`|`/`^`; a Known01 result may convert back to boolean; the implementation MUST NOT generate `boolean ^ 1` or rewrite it into short-circuit `&&`/`||` |
| eqz/nez | int comparison with 0, boolean negation or identity, ref comparison with null, chosen by the solved use type |
| cmp-long | `x < y ? -1 : x > y ? 1 : 0` |
| cmpl/cmpg FP | `x < y ? -1 : x > y ? 1 : x == y ? 0 : bias` with bias=-1/+1 |
| FP→int/long, double→float | the corresponding Java cast; a host Rust cast MUST NOT precompute the original runtime result |

Both x and y of cmp are already materialized locals, so the three reads do not repeat the
original operand effects; this expansion needs no extra calls such as Float.isNaN, preserves
NaN and ±0, and MUST NOT collapse cmp+branch wholesale into `<`. The implementation performs no
FP reassociation or fastmath.

A finite FP literal is read from its original bits and then written with a round-trip spelling
and the f/d suffix; -0 is written explicitly as `-0.0f/d`, and Infinity as `1.0f/0.0f` or its d
version (negative infinity takes the negative form). A canonical NaN MAY use `0.0f/0.0f`, only
under semantics that do not require exact payload preservation. An arbitrary NaN payload or a
non-literal bit reinterpret is Unsupported when no existing expression rule covers it; the
implementation MUST NOT add standard-library calls for recovery. Calls already present in the
original program are still bound per §9.
JVM-only validation uses the established JDK21/Java8 source target and does not thereby
guarantee FP details related to every historical JVM/ART implementation.

Java literals: an ordinary NUL uses a legal octal/string escape, and newline/tab use `\n`/`\t`;
the printer MUST NOT output a `\u000a` that Java Unicode preprocessing would turn into a source
line break. An isolated surrogate is emitted as its corresponding UTF16 escape; an actual
backslash uses `\\` so that it does not form an unintended Unicode escape. Diagnostic lossy text
MUST NOT enter a literal.

### 4.3 Representation family: finite constraints, not a global type guess

A Family is not a final Java type: boolean/byte/short/char/int all belong to Integral32, but each
producer may keep its own legal declared type. I32 and F32, and I64 and F64, MUST NOT be
interchanged merely because they share a word width; a numeric cast changes the value and is not
a bit reinterpretation. Reference and Integral32 are also not merged because of a const 0.

Build two different graphs, and MUST NOT treat “compatible type” as “same value”:

- The family equality graph: move/Phi inputs and outputs share a family; both sides of a
  two-register if-eq/ne need the same comparable family. equality constrains only the
  representation and propagates no object identity or value. Scalar opcodes, descriptors,
  move-object/move-wide and similar give the allowed family sets; if-*z MAY allow Integral32 or
  Reference, and FP conditions MUST go through cmp.
- The value-facts graph: only value-preserving move/Phi edges propagate producer origins;
  comparing two numbers does not make their origins equal. check-cast and primitive conversion
  are independent producers, and old type conclusions do not propagate across them.

Before selecting a family, compute the finite closure of the origins already fixed by
descriptors along move/Phi and the array-component rules, and give aget/aget-wide the family
constraint of the actual array component; the implementation MUST NOT default to I32 first and
then discover that the input is float[]. Without array evidence the Java type of that load MUST
NOT be sealed. This descriptor closure only performs unions and does not depend on the final
source storage type, so P4 and P5 do not wait on each other.

A pure const creates **type-view records** `LiteralUse=(ValueId,use-site)` and does not allocate a
new SSA ValueId. One Reference use and one Integral use of the same const 0 are not the same
family node; otherwise a polymorphic literal would wrongly become a whole-method type conflict.
Non-literals are related only through real value flow and constraints, and views MUST NOT be
duplicated for them.

```text
create family nodes for ordinary values and LiteralUse
merge equality edges with union-find (per Phi incoming, move, and eq/ne that need the same family)
for each group G: Allowed[G] = intersection(width/opcode/descriptor requirements of all members)
restrict LiteralUse further: 32 bit MAY be I32/F32, 0 MAY be Ref; 64 bit MAY be I64/F64
if Allowed is empty: that Java representation is unsupported; do not default to Object
if several remain: pick the single family required by the collected uses;
  if no use distinguishes them, One defaults to I32 and Two to I64
after sealing, check every original operand constraint and do not reselect in P6
```

The defaults only resolve literal interpretations that no producer or use can distinguish; they
MUST NOT override a known F32 producer. When several non-literal uses require incompatible
families, the implementation MUST NOT search cast combinations or introduce a bitcast library.
Building the family graph is near linear (union-find amortized
O((values+constraints)*alpha(values)), and every edge, merge and set intersection is charged.

### 4.4 Producers and Known01: the second finite fixed point

Descriptor origins are closed per the previous section first, to constrain typed loads; after the
family is sealed, value facts and integral domains continue to be checked, still using only
unions over finite sets, and the family is never changed in reverse:

```text
Origins(v): finite set<primitive producer tag | qualified reference type>
IntegralBits(v): subset {Zero, One, Other}
```

Parameters, fields, returns and conversions provide concrete producer seeds; move copies the
source facts and Phi unions its incomings. Original type names come only from already read
descriptors/catalogs and their bounded array components, and generic types MUST NOT be
synthesized indefinitely. An ordinary int parameter has all three IntegralBits, a boolean has
{Zero,One}, const 0/1 have a single element, other integral literals have {Other}; every other
arithmetic operation defaults to all three and MUST NOT be given an inferred range arbitrarily.

`and-int x,1` directly yields {Zero,One}; when both sides of and/or/xor can only be 0/1, the
result can only be 0/1 as well.
The empty set reached by facts is Bottom, not “proven false”. The transfer function is monotone
with respect to possible-value-set inclusion, an unknown source only widens the possible set, and
the worklist propagates only when an actual increase happens. Known01 holds only when the final
set is non-empty and contains no Other; a non-sealed set that happens to contain only 0 MUST NOT
justify deleting a branch or sealing a type.

Component facts of array-get are derived from proven array origins by stripping one `[` to obtain
the component; without array evidence no cast is invented. Every derived component still belongs
to the finite descriptor closure. Reference sets may grow considerably, and constraints and bytes
are charged per new fact; the implementation MUST NOT claim that this step is strictly linear in
the input bytes. The worst case is roughly the number of reachable facts multiplied by their
propagation edges, and it is bounded by the method/request quotas.

### 4.5 Determine the source storage type once

The producer itself keeps the type decided by the declaration and original semantics; a common
storage type is chosen only for a Phi:

| family / incoming facts | Phi result choice | copy/use rule |
|---|---|---|
| Integral32, all non-literal sources are boolean | boolean | literals limited to 0/1; an int use uses BooleanToInt |
| Integral32, containing byte/short/char/int or another numeric source | int | narrow producers are read per Java promotion; boolean sources are explicitly resolved to 0/1 |
| an Integral32 merge of literals only | boolean when all typed uses are boolean and all bits are 0/1, otherwise int | this does not call an arbitrary non-zero value true |
| Float32 / Integral64 / Float64 | float / long / double | literals follow the bits view; a cross-family numeric cast MUST NOT disguise a copy |
| Reference, containing a non-null source | the known common nominal superclass; Object when no narrower one can be found | interface/array rules use only proven assignability; a later narrow use still needs evidence |
| Reference, all incomings are null | a known ref type that satisfies all uses at once; Object when unconstrained | several incompatible narrow uses without a single type are Unsupported; no universal downcast is generated |
| Uninit or a Ref/primitive mixture | outside the ordinary Phi profile | submit to the restricted construction rules of §9 or reject Java |

The common superclass is the intersection along the complete known super chain; a missing
hierarchy MUST NOT lead to a guessed subtype. That every Java reference may be safely upcast to
Object is a language fact, but it does not imply that Object can be passed to an arbitrary
String or array formal. The type selection order is semantic policy, not an ordering for stable
source text; the implementation MUST NOT choose a type by “the first producer that arrives”.

Every UseRequirement is then checked as
`interpret(source type, value) == original word interpretation` and yields a single proven
conversion. A Phi edge allows only a value-preserving, no-throw view or widening;
an ordinary use allows the ProvenNarrow of §4.9, which truncates no possible value. A byte/char
conversion that really changes the value MUST correspond to the original conversion or store
semantics; a narrow store to an array or field MUST NOT be used as a Phi repair.
A check that reaches Unknown is Unsupported/TypeEvidenceMissing; a definite violation of the
input types requires independent rule evidence, and the implementation MUST NOT report every type
representation conflict as Malformed. There is no general CSP backtracking or implicit-boxing
solver.

### 4.6 Derivations for three error-prone examples

- D013: a boolean parameter may keep a boolean local, and the operand of a DEX int bitwise
  operation obtains its word with `a?1:0`. The Known01 closure of xor/and makes `v!=0` safe at the
  return. A non-short-circuit bitwise operation MAY also be emitted for an already proven boolean
  result shape, but the original boolean and the digit 1 MUST NOT be put directly on both sides of
  a Java operator; a particular printed text is not required.
- D014: the literal 0 emits null in a ref use, false in a boolean use and 0 in an int use; these
  three interpretations do not turn one real Object value into an int. After a literal passes
  through a non-literal merge, the single storage rule of the Phi still applies.
- D015: the cmp result is int(-1/0/1) and the original FP inputs are read only from materialized
  locals; the NaN bias is producer semantics, and it MUST NOT be deleted as a type simplification
  merely because the successor looks like `<`. ±0 compare equal and do not take the NaN branch.

### 4.7 Termination and checker duties

The family phase performs only finite merges and intersections, and the facts phase performs only
finite unions; the source type is chosen only after both are sealed.
Therefore no oscillation exists in which the same variable becomes Object, then String, and the
analysis reruns. The algorithm is deliberately incomplete: some real equivalent Java exists but
still fails beyond this profile, and the implementation MUST NOT claim that “all legal DEX is
expressed”.
The checker validates the producer source, the selected family, the Phi incoming conversions and
the original check-cast site per raw operand; a primitive template also needs public behavior
evidence. Type consistency does not prove that writing sub as add is correct.

### 4.8 Literal encoding: construct from bits, do not guess values from pretty strings

I32/I64 MAY uniformly use 8/16-digit hex (I64 with a trailing L), for example 0x80000000 and
0x8000000000000000L, which avoids spelling the positive 2147483648 as an illegal int token.
boolean is only 0/1 and a Reference literal is only 0; a narrow literal view MUST keep the
original word interpretation, and the implementation MUST NOT change an original int 255 into
byte -1 arbitrarily in an ordinary call. Truncation allowed by an original narrow store or
conversion stays in the corresponding Op.

A finite FP MAY emit an exact Java hex literal directly from the IEEE bits, with no runtime
library call:

| Type | normal | subnormal |
|---|---|---|
| float | `[-]0x1.<(frac<<1):06hex>p(E-127)f` | `[-]0x0.<(frac<<1):06hex>p-126f` |
| double | `[-]0x1.<frac:13hex>p(E-1023)d` | `[-]0x0.<frac:13hex>p-1022d` |

frac is the original fraction integer, the sign is independent, and the exponent is emitted as a
signed decimal integer; zero/Infinity/NaN follow the separate branches of §4.2. The single 0 bit
appended to float is only hex alignment and performs no rounding; the smallest float subnormal is
0x0.000002p-126f. A proven round-trip spelling MAY also be used, but the implementation MUST NOT
evaluate it on the host and then drop the negative-zero/NaN policy.

A String is escaped unit by unit as UTF16 without NFC/NFKC normalization, and a surrogate pair
MUST NOT be combined before counting the length.
quote/backslash/CR/LF use language-specific escapes, an original backslash is output as a pair to
avoid an unintended Unicode escape; Java source escaping is not reused for JSON debug snapshots. The
CONSTANT_Utf8 of a Java classfile has a further 65535-byte bound: per u16 unit, MUTF8 length is
0→2, 1..127→1, 128..2047→2 and everything else→3. The six-character `\uXXXX` in Java source is
not the same measure as this length; exceeding the bound MUST NOT be avoided by concatenating
compile-time constants, because javac folds them.
To work around the bound the implementation MUST NOT introduce StringBuilder, chunked helpers or
changed intern identity; exceeding the bound is Unsupported/JavaLiteralTooLarge, and raw is
selected per §8.

### 4.9 Lossless use conversion of narrow integers after a Phi

A Java carrier of int does not mean the value range is all of I32. The following producer bounds
are attached to Integral32, and the implementation MUST NOT infer a range backwards from actual
test values, debug names or “the callee formal wants a byte”:

| producer | fixed range |
|---|---|
| boolean parameter/field/return/definition | [0,1] |
| byte parameter/aget-byte/iget-byte/return/int-to-byte | [-128,127] |
| the corresponding short producer/int-to-short | [-32768,32767] |
| the corresponding char producer/int-to-char | [0,65535] |
| ordinary int parameter/field/return and integer arithmetic | [-2147483648,2147483647] |
| Integral literal view | the single point of the original bits interpreted as signed I32 |

int-to-byte and similar operations **establish a new producer**, and the earlier int range MUST
NOT be carried on as the result range; conversely, an ordinary add/sub yields the full I32 range
again even when both operands are bytes, and algebraic conclusions such as x+0 or x-x MUST NOT be
inferred.
move copies the bounds and Phi takes the hull of the incoming closed intervals; Bottom means that
no fact exists yet, not [0,0]. Lower bounds only descend and upper bounds only rise, and all
endpoints come from fixed producer bounds or literals, so the closure is finite and monotone.
Each change and propagation is charged as work; the implementation MUST NOT add a general
interval/widening solver.

After the facts are sealed, existing Known01 evidence MAY intersect the checked range with [0,1]
without writing back or restarting the earlier closure.
For a target B/S/C, ProvenNarrow is generated only when the final range is non-empty and fully
contained in the corresponding target range:

```text
prove_narrow(v, target):
  require family(v)==Integral32 and source carrier is int-like
  R = finalized producer range; if Known01(v): R = R intersect [0,1]
  require R != Bottom and R subset range(target)
  return explicit (byte)/(short)/(char) cast + range evidence
```

This primitive cast throws nothing, allocates nothing and is the identity on every value in R; it
is not a truncation used to repair a wrong value. It is used only to match the static source type
for ordinary uses such as return/call, and it neither changes the Phi carrier nor narrows
arbitrarily on an edge. boolean still uses the Known01 rule, and no legal Java `(boolean)int`
exists.

Example: for `byte id(byte x){return x;}`, the EntrySeed is byte and the entry Phi MAY store an
int, but the range is still [-128,127], so `return (byte)phiLocal;` preserves the value. A
byte+short Phi MUST NOT be converted to byte unconditionally; a byte that took part in an
addition MUST NOT be converted to byte through its old origin either. An unknown or out-of-range
range rejects that Java use; the implementation MUST NOT add a runtime if or change the original
descriptor. The original lossy conversion of D012 is still executed at the original Op and is not
superseded by this rule.

## 5. Locals, Phi copies and exception shadows (T08/T17)

### 5.1 Parallel copies on normal edges

A Parameter binds the original parameter; every other initialized, non-pure-literal retained
Value and Phi has its own independent Local and is not coalesced. The uninit aliases of §9.3 use
DeferredAllocation and MUST NOT be given a fake null local.
For every Phi incoming a `CopySet[IncomingEdge]` is created; EntrySeed copies are placed at the
method entry (constructors are constrained by the prologue-substitution and post-normal
materialization rules of §9.2), and the other copies are executed only **after that edge is
selected**. The type comes from P5, and the source is a materialized local or a pure literal.

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

## 6. Ordinary structures and the dispatcher (T09/T13/T14)

### 6.1 An executable dispatcher

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

### 6.2 Deterministic recognition rules for natural structures

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

### 6.3 Structure validator

Validate `BlockId→executed node`, `EdgeId→transfer` and the original op order of every block; a
producer/result MAY merge into one source expression together with synthetic pure steps, but a
dynamic effect MUST NOT be omitted or repeated. The normal transfer's CopySet and target, the loop
continue/break targets, the switch keys and default, and return/throw are validated separately.
Protection and monitor checks run separately per §10/§11, and exception edges MUST NOT be placed
into normal dominators.

### 6.4 Mathematical specification of dominators and postdominators

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

### 6.5 Local execution relation of structure reduction

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

## 7. Original DEX debug output

When Java recovery fails, Diana retains the original method identity, typed reason
and available original facts for an Agent to inspect. This is an analysis view,
not executable source, a standard Smali document or a promise of reassembly.
Production uses the checked original bytes and decoded facts; it does not run an
assembler, another engine or another recovery attempt.

### 7.1 Facts and coverage

The debug snapshot contains only original facts, never a failed Java AST, guessed
types, invented registers or reconstructed effects. It records:

| Fact | Required information when safely readable |
|---|---|
| Identity and declaration | Physical/logical origin, class and method indices, owner, full name/proto and raw flags; no-code declarations remain code-free |
| Code layout | Original code range, code-unit count, register/parameter-word counts and instruction PCs/widths |
| Instructions | Original mnemonic, ordered register words, exact literal bits and full references; invoke word order is not collapsed into source arguments |
| Control and data | Branch targets at original code units, switch keys/targets and array payload width/count/bits |
| Exceptions | Original protected intervals and handlers in priority order |
| Referenced values | Exact UTF16 units, field/method/type/proto references and used call-site/method-handle data |
| Context | Available hierarchy, fields, encoded values and their ownership; debug names/annotations may be included without requiring Smali directives |
| Missing information | Explicit undecoded ranges and the reason; safe original bytes may accompany them |

A complete decoded view retains every original instruction, including dead nops,
and the associated targets, payloads, handlers and referenced values. Optional
source/debug names and annotation syntax are not output-completeness gates.
A partial view MUST identify what is missing; it MUST NOT claim complete coverage.
Unknown instruction width stops decoding that range. The renderer MUST NOT guess
NOP, advance by a guessed width or invent a standard instruction. Local damage
MUST NOT erase facts already read safely or completed Java from other members.

### 7.2 One read-only exit

After a normal failure, release unused analysis scratch and produce debug output
from the original checked facts within the remaining RunContext resources and any
explicit fallback quota. The existing fallback_* fields govern this debug work;
there is no separate recovery subsystem. Cancellation stops immediately. A known
Malformed or BUG remains a hard failure even when useful debug information exists.
Insufficient resources leave an explicit Unavailable reason and any safe excerpt;
Diana MUST NOT retry normal analysis, increase limits or silently truncate a view
marked complete. Omitted quotas remain None, not zero or an invented default cap.

### 7.3 Readable snapshot, not an assembly language

Debug source is a readable JSON snapshot, tagged `diana.dex-debug/1`, with explicit
coverage. It exposes original declaration/code facts rather than SSA, CFG or AST
objects. Default text places that snapshot at the failed member; JSON exposes the
same member source with language `dex_debug`. No extra format switch is needed.
Object key order and whitespace are not stable. Instruction, argument, payload and
handler arrays retain program order. The acceptance adapter independently parses
the actual source slice and compares its facts with the certified original input.

The snapshot carries class/method declaration records, code records and referenced
call-site/method-handle data. A code record contains original register/count facts,
ordered instruction records and protected intervals. No-code declarations use a
null code record, not an invented body. Partial decoding identifies undecoded
ranges separately from decoded records. An unavailable method may instead show
only checked offsets and original byte/word values, clearly marked as an excerpt.
A method-word excerpt exposes its original member identity, `code_unit_start` and
`words` (u16 values) as readable JSON. A physical-byte excerpt instead names its
file-byte coordinate and byte values; neither assigns an unknown opcode width.

Strings and arbitrary UTF16 values use exact u16 vectors where UTF8 cannot carry
them; readable spellings never replace their units. Numeric literals and payloads
retain their original width/bits, without FP conversion. JSON escaping handles
quotes, controls, backslash-u and text containing */. Debug output is never put
inside a Java comment or a fake executable method. No Smali labels, directives,
class shell, assembler grammar or byte-for-byte rebuilt DEX is required.

## 8. Deterministic rules for representation selection

Each method runs exactly one analysis chain and selects a result by the following conditions:

1. Cancellation stops the request immediately and MUST NOT continue raw per method.
2. A proven Malformed yields Unavailable with any safe original debug excerpt; it never becomes a normal success. abstract/native produce DeclarationOnly only when declaration and target checks pass; otherwise retain their code-free declaration as debug facts or Unavailable.
3. When all Java checks pass, the result is Java; a failing method does not print its unvalidated
   half AST, and other completed methods are not discarded.
4. Unsupported/BudgetExceeded enter the debug exit once. Read only facts whose original ranges/widths are checked, retain their coverage and mark unavailable regions. Any further read uses the same request resources and explicit fallback quota. A confirmed Malformed MUST NOT be revoked by re-reading.
5. When a decoded debug view cannot be produced safely within available resources, the result is Unavailable with any safe excerpt. When raw
   newly discovers definite damage, keep the Malformed fact; InternalInvariant keeps its BUG
   status even with raw attached. The implementation MUST NOT rerun normal or change the engine
   or limits.

When the caller explicitly sets normal work to 0 and raw is still available, the result MAY be
a decoded debug view; when the explicit fallback quota is also 0, the result is Unavailable.
None is not 0, and an unlimited default quota MUST NOT be treated as an exhausted budget.
This is a single branch from a failure result to the read-only raw exit, not a recovery
scheduler.

### 8.1 Result presentation is separate from Java eligibility

P8 decides language eligibility, P9 only reads the existing result, and the application MUST NOT
refuse to call the renderer because a class is Mixed:
1. JavaUnit uses the complete Java printer; a single-method Java or DeclarationOnly result prints
   the corresponding declaration or body without an index prefix.
2. Mixed first states the original class identity and “not a complete Java compilation unit”, and
   keeps the shell/fields according to the real DeclarationOutput. Members are presented
   segment by segment according to their original MethodKey: Java keeps the validated declaration
   or body; Generated references only the proven owner syntax and the original key mapping and
   MUST NOT fabricate an independent method; Debug keeps the available original facts, their coverage and the reason;
   Unavailable keeps the identity, the reason and any available excerpt. An unexplainable shell is
   marked as such, and available methods MUST NOT be deleted because of it.
3. The language/reason marker and the source segment are charged UTF8 bytes separately, and
   OriginSpan is relocated to the actual text of this request; the implementation MUST NOT put raw
   into a Java comment or fake body that could close accidentally, and MUST NOT use stubs to fill
   out the class. Untrusted names and messages use the established display escaping, while string
   and payload contents are still preserved completely per language. No output ordering is
   required.
4. text and json share the same Report, and MUST NOT rerun the analysis or trigger a second
   fallback. Complete rendering happens within the original quota, and the payload is checked
   before writing; allowing Mixed does not allow truncation. A single Unsupported member is not a
   ban on presenting the whole class.
5. Result completeness/language eligibility is separate from error classification: a complete
   Unsupported result MAY be presented normally; Malformed, BudgetExceeded, InternalInvariant and
   similar keep their failure status even when a renderable Report already exists, and the
   achieved output is not cleared; Cancelled MUST NOT synthesize a Report and continue raw, and a
   render or I/O failure MUST NOT be passed off as complete delivery.

## 9. Call binding, objects and constructors (T11/T12)

### 9.1 CallEvidence generation and discharge

P4 keeps the original owner/name/proto/invoke kind, the word operands, the complete or unknown
declaration set, and access/throws/class-init facts. Each owner/name group includes
inheritance/hiding candidates, and an incomplete candidate set MUST NOT be marked Complete.
P4 generates only P5 constraints and BindingObligation and does not require the P5 source types in
advance.

P8 first fixes the MemberEmission of the class request per §12.1. The following steps apply to an
ordinary Java call that is actually printed; an original call already consumed by a complete
Generated template is validated by that rule and MUST NOT pretend to be an independent source
call undergoing overload selection. The ordinary-call candidates are source-visible declaration
projections, and a proven compiler bridge MUST NOT be treated as another hand-written
return-only overload; the original runtime directory is still fully kept for checking the
original invoke/dispatch identity.

P8 processes each ordinary Java call in the following fixed order:
1. Fix the receiver and arguments as locals or literal views per P5, preserving the original
   receiver/argument evaluation order.
2. Make the source static type of every argument exactly match the target formal descriptor;
   only Identity, a proven upcast, the original check-cast or a validated primitive conversion is
   allowed. `(T)null` is safe; the implementation MUST NOT add downcasts arbitrarily.
3. Enumerate the complete and accessible source-visible candidates, first collapsing overridden
   parent declarations per inheritance/override/hiding facts, and then checking fixed-arity
   overloads against the fixed formal types; a normal covariant override MUST NOT be mistaken for
   two hand-written overloads. varargs follow the original array argument and are not unpacked or
   boxed. The selected erased member/dispatch MUST match the original invoke; when the remaining
   valid declarations differ only in return type with no legal override or Generated explanation,
   or when an unclear generic binding remains, the result is Unsupported.
4. static uses a type qualifier that provably resolves to the same resolution/initialization
   target; virtual/interface keeps the receiver; direct is allowed only in the current legal
   private/constructor form; super uses `super.m` and checks the same super resolution target.
   When interface-special, cross-owner access or the source shell cannot express the call, the
   implementation MUST NOT rewrite it as virtual to hide the problem.
5. For a read of static final primitive/String, additionally check
   `JavaConstantKind=Constant|NonConstant|Unknown`. An internal unit uses the blank-final/static
   block scheme of §12 and is marked NonConstant; if an external one is Constant, a Java source
   read may be inlined and lose the class-init or the original read, so sget MUST NOT be lowered
   directly and the result is Unsupported/JavaConstantFieldRead. Unknown yields
   Unsupported/BindingEvidenceMissing; the implementation MUST NOT trigger initialization
   additionally through reflection or a helper.
6. Accumulate the escaping checked exceptions and subtract the ones catchable in order; the
   remaining throws MUST be compatible with the original override/declaration contract. An
   unknown throws set or parent contract is rejected; the implementation MUST NOT use sneaky
   throw, wrap in catch or widen the public contract.

A method without calls produces an empty but valid BindingTable; a whole-platform catalog is not
required. const and primitive operations do not depend on fabricated library facts. The
implementation binds only calls already present in the original program and MUST NOT add
intrinsics for primitives or bit reinterpretation.

### 9.2 allocation/init

new-instance defines `Uninit(AllocationId)`, and only a pure move-object tracks the aliases before
initialization, but before init the value MUST NOT be used as an ordinary Ref, passed to an
arbitrary call or placed into a general Phi. The pairing requires a unique new→init in the same
block, with only non-throwing, effect-free alias moves in between; the arguments are already
materialized before new or are pure literals.
One `new T(args)` is generated, consuming both the allocation and init sites; after successful
initialization all aliases of that allocation are the same Ref; exception paths keep the
protection domain of each original site. When the protection domains differ and cannot be mapped
accurately, the method is rejected.

The first Java statement of a real constructor MUST be a legal `super(...)`/`this(...)`. Only the
leading parameter moves and numeric literals MAY be folded into that call; the printer MUST NOT
output local declarations, field writes or helper calls before it.

Only afterwards are the ordinary locals and body emitted. If the only incoming of a fixed entry
Phi is EntrySeed, the prologue MAY trace the original formal/literal along that pass-through Phi
and the pure parameter moves and use them directly for the super/this arguments; UninitThis
likewise acts as a receiver only through this restricted entry chain. The implementation MUST NOT
open multi-incoming or back-edge Phis or globally merge ValueIds.
The local declarations, EntrySeed copies and prefix move materialization involved happen only after
the call completes normally, and the implementation MUST prove that no other use or handler can
observe them before the call; a failing path MUST NOT materialize the initialized this. This way
the output neither reads an unassigned Java local before super nor fabricates compilation by
assigning null to uninit. Inheritance, access, ctor overload and checked throws are still checked
by the same binding rules. T22 has separate rules for outer capture, and a general pre-super
reordering MUST NOT be opened.

#### First-entry initialization evidence

A constructor-delegation query needs the first initialization link, not a successful
analysis of the callee's remaining body. P1 supplies a distinct method envelope
with checked declaration and code ranges; it does not interpret register values or
claim complete instruction/metadata validity. P2 produces an entry window through
the first direct constructor invocation using the ordinary local-operation and
call-site decoders. The window admits contiguous nop, move and numeric-literal
operations; another operation or control transfer before the invocation leaves the
link unproved. The entry-window profile rejects try metadata. P2 checks the
window's register ranges, parameter words and original invoke operands before use.
It does not decode instructions after the window endpoint.

P3 binds the window with the ordinary parameter-seeding, read-binding and
definition-writing operations. Its sealed entry SSA describes first activation
from the original parameters; it contains no fabricated terminator, CFG or full
method-success claim. Independent replay checks original literals, word ownership,
latest definitions and invoke word order. Ordinary constructor proofs use the same
SSA-origin query, with only a unique EntrySeed permitted through a full-method
entry Phi; the full-method back-edge restrictions remain unchanged.

P4 records original formal families and this/parent scope plus constructor
binding candidates. P5 follows SSA definitions rather than maintaining register
roots. It checks that the receiver originates at UninitThis, that move families
and argument word/reference/wide families agree, and that a reference argument is
not UninitThis. The graph consumer discharges the recorded binding and checks
acyclic this-delegation until the original parent link. Missing evidence or a cycle
leaves initialization unproved; it never supplies a default link.

P8 MUST consume the graph certificate before sealing a delegating constructor.
Contributors MUST NOT use an entry product to admit the callee body as Java or to
bypass that body's ordinary P1–P9 chain. A failure after the entry endpoint does
not erase the checked link or change the callee's own classified result. The query
uses the selected method's existing budget and cancellation control; it is not a
retry from the raw-output exit.

### 9.3 uninit is a state of the use position, not one globally changed ValueType

An AllocationId marks a static allocation site; whether initialization succeeded varies with the
control position. After discovering one init, the implementation MUST NOT globally change that
ValueId into Ref and thereby give a catch on the failing init path an already completed object.

The same-block pairing is checked in the original order: between new and its unique init only
alias moves are allowed, the receiver is that A, and no ordinary heap/call/return/throw use is
allowed; ordinary use is allowed only after init completes normally.
A cross-block use MUST be dominated by this normal completion, and the exception transfer keeps
the Uninit state; when this cannot be proven, the current Java profile is unsupported. P4 hands
the availability of each use to P5 and MUST NOT add a new physical write to SSA.

P6 uses `LocalBinding::DeferredAllocation(A)`: the pre-init aliases have no readable Java local and
are also not assigned a default null. One ConstructorExpr in P8 consumes the new and init sites
and materializes the obj local on success; afterwards these aliases map to the same obj, and an
original reg overwritten later is another ValueId. This is a dedicated storage rule for
constructor legalization, not a general local coalescing.

It preserves the allocation→init→use order and rejects printing `new; effectfulArg(); init` as
computing arg before new. Truly supporting that wider shape requires proving the internal order of
allocation, arguments and init in Java new; the implementation MUST NOT invent a carrier/helper or
reorder by default. Inside a constructor, UninitThis up to super/this likewise becomes a Ref only
after normal completion, and leading local declarations MUST NOT move before the Java constructor
invocation.

### 9.4 Executable decision of Java binding

For the target formal vector F, the actual source static types are fixed to F before the
candidates are judged, instead of looking only at which overload the printed text resembles. For a
complete candidate m, fixed-arity applicability holds when the arities are equal and each F_i
converts to m.formal_i by an identity/widening conversion allowed in this profile; the decision
performs no boxing, generic inference or varargs expansion, and these shapes MUST NOT be treated as
“competing candidates that do not exist” when they influence selection. When the candidate set
contains such shapes that could influence selection, the Java binding is rejected. Among the
applicable candidates, only a unique most-specific target whose original owner/proto/dispatch all
match is accepted.

The null literal of D019 MUST have the static type Object formal, otherwise pick(String) wins;
the super of D021 is not an ordinary virtual call on the same receiver. A varargs call uses an
array argument at fixed arity, and a declaration that MUST keep its varargs flag still needs the
legal `T...` form; not expanding the call MUST NOT be equated with freely changing the
declaration.

Completeness covers declarations in the owner/name/inheritance/access scope, not “the pool
contains that method_id”. A missing parent, a hidden candidate and a return-only collision MUST
NOT default to success. Static qualifiers, fields and type spellings MUST also be resolved in the
actual source scope: even fully qualified text can be shadowed by a field or type of the same
name, so the implementation MUST NOT merely concatenate descriptors. When the source name cannot
be bound faithfully, the related Java is rejected and the original key/name is unchanged. No
complete whole-platform catalog is needed.

### 9.5 Boundaries of checked throws and declaration proofs

P4 collects declaration and binding obligations and MUST NOT widen a method's throws in advance by
the catch parameter's Throwable. After the lexical catch structure is fixed, P8 separates the
declared checked set of the original operation, the part already caught in priority order, the new
exceptions of the handler itself and the rethrow of the original exception; the precise-rethrow
rule of §10 applies only to an effectively-final original catch parameter. Constructor,
initializer and override have different allowed throws sets, and `throws Throwable` MUST NOT be
added uniformly.

An Unknown fact does not mean that no checked exception exists, and Complete does not prove that an
external callee never throws another object at runtime through sneaky or native means; it is
primarily the declaration contract used for Java compilation binding. The semantics still keep the
original call and exception propagation, and a declared throws MUST NOT delete a real exception
edge. The original catch priority MUST NOT be reordered by the throws set either.

## 10. try/catch and protection domains (T16–T18)

### 10.1 Inputs and matching conditions

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

### 10.2 One nesting level and the validation algorithm

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

## 11. monitor and synchronized (T19)

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

## 12. class, initialization and compiler-generated members (T20–T23)

### 12.1 unit assembly order

Determine the owner/package/kind/hierarchy and the member declarations first, and associate results
by the original ClassKey/MethodKey rather than by source name or arrival order. Before ordinary
method lowering, a class request first determines MemberEmission from the P1-validated raw and the
complete declaration facts by checking the direct templates of §12.3; the templates check
parameter/receiver sources, required casts, exact target/field/order and the absence of extra
effects, and they do not depend on ordinary Java overload inference or on evidence patched in after
a conflict is dropped.
Only complete evidence selects Generated; a mismatch keeps raw/Explicit and the Mixed reason, and
the implementation MUST NOT silently delete the member. This table keeps only the owner/original
key/template evidence, does not hold whole-class SSA, and introduces no scheduling that reruns
methods after a guess.

The source-visible shell is validated next, and the Explicit methods are analyzed one by one; a
failure of shell source eligibility records only a class reason, and while the original raw remains
locatable the analysis of each method continues, so one Unsupported MUST NOT skip the remaining
members early. Only hard terminations such as cancellation, global damage or request resources
stop per the request rules, and an unprocessed prefix MUST NOT be passed off as a complete result.
Finally the unit-level checks of final/constructor/override and of the actual Generated layout are
performed. The original code of a Generated member is consumed by the template rule, and budget and
target admission still apply; its body is not required to be an independently valid Java method
first. A single-method request without owner-generating syntax still returns debug information for that
original method per §12.3. Any shell or member lacking evidence makes the class Mixed, while
independent explicit methods MAY still be Java. An illegal original Java member name MUST NOT be
renamed globally; only synthetic locals MAY be named safely.

Identifiers are validated by the Java8 rules: the first character has the letter categories
Lu/Ll/Lt/Lm/Lo/Nl, Sc or Pc, and later characters MAY additionally include Nd/Mn/Mc;
identifier-ignorable control and Cf characters are rejected. Keywords, true/false/null are not
identifiers, and constructor special names are handled separately. The implementation uses a
native Unicode general-category library whose version is locked by Cargo, MUST NOT query JVM
Character or lossily rename everything outside ASCII, and reports unsupported source explicitly
when the Unicode case is unsupported.

#### Source package and type spelling

For an ordinary class descriptor `Lpkg/path/Name;`, the package comes from the
slash-separated prefix and the class name from the last segment. No slash means
the unnamed package. Every package/name segment passes the identifier check
above. `$` is an ordinary identifier character; splitting it into nested owners
requires the complete ownership/template evidence of §12.3, not string splitting.
The source file name and CLI dotted selector never determine the package.

P8 fixes a source-name plan before sealing Java; P9 only emits it. The plan emits
no imports or static imports. Primitive descriptors map to primitive keywords,
arrays to the component spelling plus `[]`, and known reference types to their
qualified source names. A named nested type uses its proven source owner chain;
an unrelated top-level name containing `$` keeps `$`.

For each type/static qualifier, P8 resolves the spelling in the actual package,
lexical owner, inherited member/type and expression scope under §9.4. A fully
qualified spelling is not automatically safe: its leading package identifier can
be shadowed in an expression. P8 MAY use an in-scope owner/simple spelling only
when resolution proves the same original type/member and dispatch. Otherwise it
rejects the affected Java rather than renaming the package/member or inserting a
helper. An unnamed-package type referenced from a named package is not guessed
accessible. Implicit `java.lang` lookup does not override an original type with
the same simple name. Synthetic locals avoid the identifiers the sealed plan
needs; P9 MUST NOT change their names in a way that invalidates that binding evidence.

### 12.2 static/final and constructor chain

- encoded_values are bound in the original static_fields order, and missing values follow the DEX
  type; the implementation MUST NOT sort by field name.
- Source fields are declared first. A non-default encoded initial value of a non-final field goes at
  the start of `static { ... }`, followed by the original clinit. For a final field, if the original
  clinit never writes it, the encoded or implicit default value is also assigned explicitly once; if
  there is an original write and the preparation value is the default, no assignment is added and
  the original write completes it; a non-default preparation value plus a later original write
  would assign the final twice, so that declaration profile is rejected. A shape where a read
  happens before the blank final assignment and Java DA cannot prove it is also rejected; the
  implementation MUST NOT delete the original read or add an assignment.
- **static final primitive/String MUST NOT get a constant-expression field initializer**, which
  would let javac inline the sget and eliminate the original class-init trigger. Generated enum and
  inner hidden members are consumed by their owner rules and their initial values are not emitted
  again.
- final uses the finite `{0,1,Many}` set of writes along a path; a join unions, a write makes 0→1,
  and 1/Many becomes Many.
  Only normal completion paths are checked, and an abrupt initialization failure propagates the
  original exception. Every normal constructor chain/static initialization MUST be written exactly
  once; when an overwrite, a missing path or an unknown path exists, the implementation MUST NOT
  fabricate a default assignment to fill the gap.
- The this()-delegation graph first checks acyclicity and the target identity; a delegating ctor
  inherits the write set of the callee and MUST NOT emit field initializations twice. The
  super/this call precedes the body and parent behavior is not omitted. The original field-write
  positions are not beautified to the declaration.
- clinit emits `static { initExit: { ... } }`, the original return-void becomes `break initExit`, and
  the printer MUST NOT print a return inside a Java initializer. If the original body is
  necessarily abrupt, wrap it in `if (true) { ... }` to satisfy the normal-completion check of a
  Java initializer; the condition is constant with no dynamic false path, and this is not an
  exception stub or a skipped original body.
  A known clinit without code needs no method; initialization exceptions are not wrapped and are
  handled by the target VM's class-initialization rules.

#### Checked source initialization profile

P1 field preparation retains decoded scalar bits or String UTF-16 units alongside
original encoded ranges/tags and bytes. A failed suffix remains partial and never
acquires an implicit default. P8 selects a source expression only for a matching
supported descriptor/value form; noncanonical NaN preparation remains Unsupported.
Finite FP and signed zero use exact hexadecimal or zero literals; infinity uses
constant division. String preparation checks modified UTF-8 target limits before
escaping original UTF-16 units.

P4 records own constructor/clinit final writes as initialization obligations,
not ordinary field-write permission. P5 WriteThis supplies constructor receiver
provenance. P8 consumes shared checked operations, storage and normal edges to
check the finite path-count set. The supported source graph is an unprotected
single-block Sequence or exclusive acyclic Diamond; affected explicit field reads,
cycles, switches and unsupported completion shapes reject this profile. P7 builds
these regions from the same P6 products. P8 replays final lexical control and
original field operations before publishing the method certificate.

A delegating constructor with no own final writes retains its checked original
target in the P8 certificate. Unit verification follows only certificates from
already-recovered original constructors, rejecting missing/non-constructor targets,
cycles and an own write combined with delegation. It does not analyze a callee
again. Missing assignments leave the unit Mixed without withdrawing independently
available methods.

P8 unit assembly emits preparation blocks in original field order after all
field declarations. A static final with checked original writes requires default
preparation and exactly one normal-path write; without such a write, unit assembly
emits the supported encoded or implicit default once. Non-default preparation
combined with a later final write rejects the unit. Static final assignments use
an already checked unqualified own-field form; fields colliding with synthetic
local spellings are rejected rather than rebound. P9 prints the sealed unit
preparation and original initializer bodies without decoding or choosing values.

### 12.3 A generated member is not DeclarationOnly

Bridges, the values/valueOf of enums, hidden constructor parameters and inner outer-capture are
language-generated shapes.
P8 uses `MemberEmission::Explicit | Generated(owner-node, rule)`; Generated is still Java member
evidence with code and is not counted as abstract/native DeclarationOnly. The unit printer emits
only the owner syntax and MUST NOT also print duplicate declarations that conflict with
javac-generated members; the raw identity is still mapped to the actual source spans of that
owner.
When a single get-method is requested and the generated member has no independently valid Java
declaration, the printer returns debug information for that original method and MUST NOT fabricate a
wrapper.

Concretely, only the following rules are accepted:
- Covariant bridge: bridge/synthetic markers, complete parent/override facts, and a body with only
  parameter moves and the required check-cast → an exact target call on the same receiver →
  return, with no extra effect or handler. Mark Generated only when the selected source
  override/generic declaration certainly generates a bridge with the same signature and the same
  conversion behavior; otherwise the class is Mixed, and a return-only overload MUST NOT be hidden
  behind renaming.
- Named inner: InnerClass/Enclosing metadata determines the owner, and `$` MUST NOT be guessed.
  When the single synthetic outer field/hidden first ctor parameter of a nonstatic inner and its
  canonical assignment match, express it with `Outer.this` and `outer.new Inner(args)`; that hidden
  assignment is owned by javac, and a general pre-super field write MUST NOT be opened. Any other
  capture/effect mismatch means no anonymous/lambda or helper is generated.

- Enum: constant static fields and order, hidden name/ordinal parameters, unique private storage,
  values copying and the valueOf call MUST be the canonical pattern; the hidden storage has no
  actual reference outside that pattern. Emit only the Java enum constants and user members and
  record the matched hidden members as Generated; the printer MUST NOT emit `class extends Enum`.
- Annotation: interface element return/default legality, runtime retention and complete value
  preservation; arrays and Strings are output per UTF16/original bits, and class/enum values use the
  exact descriptor. System metadata MUST NOT be fabricated as a Java annotation.

The native named-member profile checks reciprocal ownership before source-name
resolution. P8 MUST preserve the original binary identity of the source owner/name chain;
metadata that would require external renaming is not source-admitted. P8 rejects
an inner simple name equal to an enclosing type name. Static public nested classes
consume ordinary member/unit verification. The instance profile accepts a public
inner with one synthetic final outer field and a constructor whose sole explicit
DEX parameter is that outer. Its original prefix is the outer assignment followed
by Object initialization; later writes or capture-field references in that
constructor reject the certificate. Other instance field/constructor layouts keep
Mixed and original facts rather than receiving an inferred capture scheme.

P4 records the exact capture declaration. P5 proves receiver/parameter SSA origins;
P6 and P8 independently replay the omitted entry operation. Capture-dependent
source requires certificates for the original constructors before lowering.
A capture read becomes `Outer.this` only for a proved entry receiver; reading
another object's capture field is not interchangeable with that expression.
The generated field product retains its original FieldKey and constructor
MethodKey. Non-public static calls to the top-level enclosing owner require
complete same-nest declarations and matching native access. Existing synthetic
static accessors remain Explicit with their original names and ordinary verified
bodies; no new accessor is synthesized and no cross-class private DEX access is
granted by lexical ownership alone.

P8 supplies a flat owner preorder with checked depths and sealed nested emission
plans. P9 MUST render those plans without inferring hidden fields from flags or
splitting binary names. Original class and method spans use final UTF-8 byte
coordinates, including nested descendants inside their owner spans. Closure work,
depth-stack allocation and output consume request budgets; no numeric default cap
or recursive report tree is introduced. A rejected nested unit makes the owner
Mixed while retaining available original nested results. A single inner fragment
is not an independent JavaUnit.

The canonical enum profile accepts a public final enum whose only fields are
public static final constants and the private synthetic `$VALUES` array. Its
complete method set is the `$values` array helper, clinit, hidden String/int
constructor, values and valueOf. Native/catalog declarations and the original
self-bound Enum Signature agree. Field preparation is complete/default; field
annotations and additional members reject this profile.

P4 binds declarations and the required standard Enum constructor/valueOf
contracts. P8 consumes validated original code and checks the constructor entry,
constant allocation/name/ordinal/store sequence, helper array order, values
clone/cast and valueOf class/argument/cast. Exact references, registers, normal
returns and absence of extra operations/protection are required. Declaration
order comes from checked initializer stores, not field-table order. Executable
references to backing storage outside these methods reject the owner. An extra
constructor effect remains original Debug/Mixed rather than being silently
consumed by enum syntax.

Generated method evidence distinguishes bridge method owners from enum class
owners; generated field evidence records constant ordinals or backing storage.
P8 checks unique and complete original field/method coverage before sealing the
enum unit binding. P9 renders the checked enum constants and maps generated method
identities to the final class source span. Mixed output and standalone retrieval
retain original snapshots. A local proof parse error remains a classified class
issue while other members are retrieved; cancellation/resource/invariant errors
terminate without a partial successful unit.

The native covariant profile accepts a no-argument Object-returning bridge to
the same owner's String-returning instance override. P4 requires the original
public nonfinal override contract and complete nongeneric ancestor evidence.
P8 checks bridge/synthetic flags, the original entry receiver, exact virtual
target, adjacent object result and return, with no extra instruction or protection.
Class requests determine this emission plan before ordinary Explicit analysis;
a passing template retains its original method identity and raw snapshot.
Standalone requests do not use class-owned bridge generation. P8 unit admission
requires the explicit target's Java result and maps the Generated identity to
that owner's verified source span. A failed owner unit displays the retained
original bridge snapshot rather than a synthetic successful Java method.

A bounded parameter bridge additionally supports one evidenced
interface<String> specialization. P1 supplies original class Signature fragments;
P4 checks their equality to the caller facts, the interface's Object-bound class
parameter and its (TT;)TT; method contract. P8 checks the original String cast,
entry receiver/argument, exact String target and return. The emitted interface
specialization is required to make javac generate that same bridge. Missing or
contradicted specialization never permits deleting the bridge.

Raw generic method declarations require original/catalog Signature equality and
checked descriptor erasure. The supported profiles are parameterized reference
returns and one Object-bounded identity variable. Static identity calls use exact
Object source formals and result views; other applicable generic competitors reject
binding. Raw parent/interface profiles require a complete Object-bound class
Signature and variable-independent member contracts. P8 retains original method
Signature alongside the erased declaration; it does not infer arbitrary generic
parent substitutions from the descriptor.

Generated is a checked core emission rule, not “delete whatever looks synthetic”. A failure of any
rule MUST NOT produce an exception stub for an arbitrary method; the corresponding required
full-unit case remains failing.

### 12.4 Java target: direct checks separate from real compilation

The Java target is Java8/classfile 52, and the development compile contract is
`javac --release 8 -proc:none -g:none` from Temurin21.0.8+9. Production does not invoke javac and
MUST NOT predict the compiler's code size.
P8 checks only the original semantics, the source rules, the declaration facts and the format items
it can determine directly.

| Directly determinable item | Rule |
|---|---|
| method parameter words | implicit this (1 for non-static) + sum(2 for J/D of the formals, 1 for the rest including arrays) <=255 |
| a single MUTF8 item such as an actual descriptor/name/String/annotation | encoded byte count <=65535, not the length of the source escape |
| the determined field/method count per class | including the proven compiler-owned members, <=65535 respectively |

The original binary proto is the truth for the parameter check: the hidden outer parameter of an
inner class and the hidden name/ordinal of an enum are counted; the return type is not.
255 static ints, or 254 instance ints plus this, satisfy the limit, while 256 static ints or 255
instance ints plus this MUST be rejected; abstract/native have no Code, but their declaration checks MUST NOT be
omitted. The implementation MUST NOT delete unused parameters, rename, split the method/class or
create helpers to bypass the real limits. When an over-limit is confirmed directly, the result is
Unsupported/JavaTargetLimit with the actual item/count/limit, and it MUST NOT be misreported as
Malformed or as an analysis BudgetExceeded.

The final classfile MUST still satisfy the real JVMS limits: with Code, `0 < code_length < 65536`;
max_locals/max_stack, exception rows and the u2 count fields are range-checked per the format;
constant_pool_count includes index 0 and the double slot of long/double and is <=65535. These are
limits of the actual artifact, not AST or ValueId quotas.
V4 performs a real compilation and reads the classfile independently to check it; when production
has no actual classfile, it MUST NOT claim to have proven its size, and unknown compiled size MUST
NOT be used as an Unsupported reason. Verified only marks the native checks above as complete; it
is not a certificate of successful compilation.
A real javac failure of required Java is still an acceptance failure, and “size not predicted” MUST
NOT turn the failure into a pass.

### 12.5 Source size is not an estimated rejection threshold

The implementation MUST NOT estimate JVM code/locals/stack/handler/pool from a fixed byte cost per
AST node, a fixed per-method overhead, DEX code units, the number of source locals or the number of
catches/monitors and then reject Java; it MUST NOT define JavaTargetCostUnknown either.
It MUST NOT build a second JVM backend or estimator, nor convert an unknown cost into a raw reason.

Three-address operands and printing macros are still expanded per the actual source semantics, and
every operation/type/evaluation order of cmp-with-bias, BoolToInt, ProvenNarrow and FillExpansion
MUST be validated. The syntax contains only expressions, assignments, if/loop/integer switch,
try/catch and synchronized that already have explicit lowering rules; string concat, boxing,
lambda, foreach, try-with-resources or finally lacking a semantic rule do not become supported
capabilities because of this.
Actual container and text growth checks only platform capacity and the caller's explicit quota (if
any), and no size threshold is added by default.
The long call sequence of D077 MUST be recovered completely and pass a real compilation; the
implementation MUST NOT shrink the method by deleting calls, creating a loop or a helper.

### 12.6 compiler-owned members and declaration completeness

A Generated rule MUST supply the actual binary member list, signatures, hidden parameters and
behavior evidence, and MUST NOT give only the owner name.
Bridges, enum values/valueOf/backing storage and inner outer-capture are matched item by item per
§12.3; guessed code or pool cost MUST NOT be attached to these members. Shared types, descriptors
and names MUST NOT become a class rejection reason through a fixed multiplier per repeated mention.
The 2200 abstract interface methods of D078 MUST be kept as a complete JavaUnit without fabricating
constructors or bodies, and the actual pool is checked by an independent classfile read.

When an encoded initial value makes the source produce a language initialization member, include it
in the layout and check the actual initialization order; it MUST NOT be treated as a new public
helper. When compiler-owned methods, fields or hidden parameters do not match the semantic template,
or the source would generate unproven members, the implementation still MUST NOT construct a
JavaUnit; that is missing member-semantics/completeness evidence, not a size estimate.
When an ordinary class has no original constructor, the javac automatic default-constructor problem
MUST be handled, and a fake constructor that never existed in the original MUST NOT be produced. A
rule failure keeps Mixed/raw and the original key, and members MUST NOT be dropped silently.
V4 compares the actually generated members with the original manifest and rules, and the Driver
separately verifies behavior; it MUST NOT trust a builder that fills in true.

## 13. Concrete wiring of control and resources

open/index, method analysis and render share one `&mut RunContext`, and Limits belong only to that
context.
By default every numeric quota is None, so no fixed threshold is added and no fixed raw
work/memory is reserved. The context stores no IR, spawns nothing and is not shared globally;
cancellation is checked in every loop pop, instruction, set growth/copy and long string chunk.
Cancellation ends immediately and MUST NOT switch to raw. Convergence of the analyses is guaranteed
by the finite state or single-schedule measures of each algorithm, and a fixed step count MUST NOT
hide an algorithm that cannot terminate.

Only items the caller explicitly sets to Some(n) have their quota checked, and accumulated used
amounts are never refunded. The normal and raw work sub-quotas are independent and both count toward
the request; method-derived allocation totals include normal and raw. Only explicitly finite
fallback work/allocation amounts claim from the parent quota, and normal MUST NOT use held amounts;
when unconfigured, raw uses only the remaining resources and does not claim an infinite reservation.
Unconsumed held amounts MAY be returned, used quota does not roll back, and no retry path bypasses
the limit.

Every allocation first checks the platform-representable size, the minimum input record size and
the applicable explicit quota, and then calls try_reserve; a real allocation failure is reported
explicitly as BudgetExceeded and MUST NOT be turned into format damage. Env, metadata, UTF16, AST, spans
and text all follow this rule. Traversal and release of untrusted depth use an explicit stack or
arena and do not rely on host recursion or recursive Drop; max_nesting acts as a numeric threshold
only when set explicitly.
Artifacts are dropped after their last consumption, and the Report keeps only the necessary owned
facts rather than the whole SSA/Flow history graph.

normal/raw/report output checks only its own explicitly set length items; the final JSON also sets
no default hard cap, see DESIGN §7/§10.3. Check against the actual escaped UTF8 increment, and
MUST NOT first build a huge String and then claim the budget was enforced beforehand.
Concatenating one member into the report does not charge the member length twice, while real work
and allocation are still accumulated; unconfigured upper limits do not silently acquire defaults.
Performance records come from authorized release runs, and the ledger does not replace RSS or actual
elapsed time.

## 14. Application input, scanning and extraction

The following uses the determined native scheme; it MUST NOT carry over the old session/MCP or build
a general file framework. Safe file access uses the cap-std Dir and the ordinary-file safe
interface, cross-platform file identity uses same-file, and private temporary files use tempfile;
compression continues the reviewed bounded native inflate approach and CRC uses crc32fast. The
dependencies are locked by Cargo. Diana code uses no unsafe; the file, decompression and CRC calls
go through safe interfaces.

### 14.1 InputLease and directories

1. Fix the allowed root and the Dir handle first, and open the explicit input by the
   capability-relative rule; devices and FIFOs are forbidden, Unix adds O_NONBLOCK/O_NOCTTY through
   the safe OpenOptionsExt so that opening a FIFO or terminal neither blocks nor changes the process
   terminal, and Windows rejects the device/pipe namespace. After obtaining the handle, re-check
   regular-file status, len and file identity.
2. A small input is copied into owned bytes and a large APK into a private snapshot file; read it in
   chunks by the entry file length and check cancellation and any explicit input quota. An early
   EOF, bytes beyond the entry length, or a change of the original handle's identity/len/mtime
   before or after all report INPUT_CHANGED and discard the snapshot; an unlimited quota also does
   not chase a continuously growing file. This promises immutability after the copy only, and does
   not claim
   that a stat check proves an atomic version under arbitrary concurrent writes of the original
   file; the analysis reads only the snapshot and never the original path again.
3. A large APK reads only the central directory and the entry ranges currently needed; each
   materialized entry has its own owned buffer, a stored entry borrows that buffer and a deflated
   entry borrows the decompression buffer, all held by the lease. There is no unsafe mmap, the
   implementation MUST NOT copy the whole APK into every DexImage, and this safe copy path MUST NOT
   be claimed to be zero-copy.
4. EOCD is searched in at most the last 65535+22 bytes, and the comment length MUST match the file
   end; it is single-disk, and the directory count/bytes are checked first against the actual
   directory range/minimum record size and any explicit limit. On the classic sentinel, verify the
   signatures, lengths, single-disk fields and 64-bit directory range of the ZIP64 locator/EOCD
   records before reading the directory; 0xffffffff MUST NOT be treated as a real allocation amount.
5. Each central record stores an EntryKey by its original ordinal, and the fixed 46 bytes plus the
   name/extra/comment lengths are checked. UTF8-flagged names are decoded strictly and others follow
   ZIP CP437; the original name bytes are kept without basename or normalization. Records with the
   same name are all kept, a non-unique name lookup MUST require the original entry index, and the
   manifest is selected by the original ordinal rule, which the worker completion order MUST NOT
   decide. The local header separately verifies the signature/flags/method and the data range, and
   the final CRC/size of a bit3 data descriptor is determined by the central directory and the
   actual decompression check.

### 14.2 Lightweight definition and xref scanning

1. H03/H09 build no Engine/SSA. Accumulate the four class_data delta groups from class_defs and
   list **actual definitions**, not every field_ids/method_ids reference. Each row carries
   scope/physical/logical/class/member original keys.
2. String matching prefilters with ASCII/raw first; non-ASCII and exact xref MUST decode MUTF8 to
   u16 before comparing, and replacement text MUST NOT be used as lookup truth. Display escaping
   is separate from matching; only the content is filtered and all results are returned.
3. Instruction layout reuses the engine's stateless `raw::instruction_layout(code_bytes, pc_cu,
   version)`,
   returning the checked width and at most two `(RefKind,index,operand_slot)` and initializing no
   ProgramIndex.
   That function is implemented by the same format table as T03; the engine's internal full decode
   uses it too, and the width table MUST NOT be duplicated. Metadata/index are still checked by the
   current scanner against its own logical tables. An unknown width makes this scan fail explicitly,
   and the implementation MUST NOT guess the next address, skip a site and then report success.
4. Every actual direct ref operand generates a site whose identity contains
   MethodKey+code_unit+operand_slot; it MUST NOT be expanded into transitive references of the
   signature or deduplicated per method. The same numeric index in different logical images MUST NOT
   be merged.
5. The worker count follows DESIGN §7, with bounded byte/row queues and weighted admission; results
   are published on completion with no global sort or ordinal waiting. Closing the consumer stops
   the producer; a mid-run failure fails explicitly, and an emitted prefix MUST NOT be passed off as
   complete success. There is no pagination, preview or successful truncation.

### 14.3 Extraction and publication of the original entry

1. Select the EntryKey exactly and first check the local/central header, flags/method consistency
   and the data range. After the structure is valid, reject any encrypted entry first
   (EncryptedEntry, accepting no password or decryption path), and then reject a compression method
   other than 0/8 (UnsupportedCompression, not Malformed for a format-valid unsupported method).
   Both decisions happen before decoding, before the plaintext size/CRC check and before creating
   output; the compressed_size of an encrypted stored entry includes the encryption header, so the
   unencrypted equal-length rule MUST NOT be applied early. Only unencrypted method 0 requires equal
   compressed/uncompressed lengths; method 8 uses bounded inflate within the declared length and any
   explicit limit, checks the actual length and end state, and traverses CRC32 against the central
   CRC. The original payload does not pass through UTF8/JSON, DEX normalization or header rewriting.
2. Output uses only an explicit -o and MUST NOT concatenate the entry name. Open the output parent
   Dir once and hold it; reject a directory or symlink target and a target with the same
   handle/hardlink identity as the input. All temporary and final operations are relative to that
   Dir, and the mutable parent path is not resolved again.
3. In that Dir, create a mode 0600 temporary file with create_new, with a fixed upper bound on
   collision retries; publish only after the write/flush and the CRC/size/budget checks complete.
   Writes and copies check the public request cancellation and budget in chunks of at most 64KiB,
   and one large unchecked write or copy MUST NOT bypass cooperative cancellation. On error or
   cancellation only the own temp is removed, never the user target.
4. no-force publishes atomically with **hard_link(temp, final)** in the same Dir, fails when any
   final item exists, and then deletes the temp. At most one of two publishers succeeds; the
   exists-then-rename sequence MUST NOT be used. When the file system does not support that
   primitive, the operation fails explicitly and MUST NOT degrade to an implementation with an
   overwrite race; the platform evidence for D070 is still not-run.
5. force allows only an ordinary target that is not an input alias at entry, then performs an atomic
   rename relative to the held Dir to replace the directory entry and does not follow a final
   symlink to write elsewhere. It is not a CAS interface that “replaces only the previous inode”,
   and it
   makes no promise that an external process cannot rename after the check; the fixed input path
   itself is forbidden as output, and the bytes of the source handle MUST NOT be written. A platform
   that cannot provide these safe primitives gets no weakened implementation. Atomic visibility MUST
   NOT be passed off as power-failure durability.
6. After the output file succeeds, stdout carries no binary content or status line; the text/JSON of
   query/get is kept separate from the binary output adapter. BrokenPipe or an I/O error stops the
   request, the erroneous target is unchanged before file publication, and no half file remains. By
   default get generates the complete bounded Java/Mixed/Debug/Unavailable presentation per §8.1 and
   then writes it, does not let JavaUnit eligibility block Mixed, and does not equate a
   deliberately marked partial-failure result with a writer truncation.

Domain errors keep distinguishable reasons: AmbiguousEntry, TargetExists, UnsafeTarget,
CrcMismatch, BudgetExceeded, Cancelled, UnsupportedCompression, EncryptedEntry and I/O. The
snake_case names in cases are the semantic projection of these public reasons, and the
implementation MUST NOT guess the stage from stderr or replace a specified CRC/budget/alias
rejection with an arbitrary error.

The H02/H10 core file-safety tests continue through the public domain interfaces and the established
self-made directories; the dependencies, path wrappers and command parsing above get no separate
mechanical or auxiliary tests. Original permissions/provenance, cross-platform not-run results and
release cost are recorded honestly.

### 14.4 Manifest decoding and AXML safety

Selection uses DESIGN §6.7's original physical-entry rule and the same checked
InputLease/inflate/CRC as extraction. ResChunk_header, ResStringPool and
ResXMLTree field layouts follow the AOSP source identified in SOURCE_MAP §7. A bare DEX or a JAR-only input is an input
error; a structurally usable APK/ZIP with no AndroidManifest.xml is no-result.
The manifest Module decodes bytes only after entry integrity passes. It shares
request cancellation and explicit allocation/work/output limits; no manifest
size or nesting quota is invented by default.

| Binary structure | Checked read and semantic rule |
|---|---|
| Document/chunk | Outer type `0x0003`; little-endian `type:u16, header_size:u16, size:u32`; `8 <= header_size <= size`, four-byte chunk alignment, checked end inside the entry, and positive advancement ending exactly at the document extent |
| String pool `0x0001` | Header at least 28 bytes; checked string/style counts and offset arrays, stringsStart/stylesStart relative to this chunk; entries and terminators remain in their own data regions |
| UTF8 / UTF16 pool entry | UTF8 stores UTF16-unit and byte lengths using one/two-byte lengths; UTF16 stores one/two-u16 lengths. Verify both actual length and terminator; this is AXML string encoding, not DEX MUTF8 |
| Style span | Checked name index and UTF16 interval, terminated within style data; styles never justify reading past the pool or altering the XML text |
| Resource map `0x0180` | Optional, before nodes; header and payload length checked, payload divisible by 4, entries associate resource IDs with valid string-pool positions |
| XML node `0x0100..0x0104` | Header at least 16 bytes including line/comment; extensions start at header_size, and their complete fixed record fits before reading any reference |
| Namespace start/end | Checked prefix/URI refs; balanced scope with matching ends. Namespace aliases are scoped, not global substitutions |
| Start element | Checked namespace/name, extension at least 20 bytes; attributeStart is relative to the extension and at least 20, attributeSize at least 20, and the complete count*stride fits this chunk; id/class/style indices are 0 or 1..attributeCount |
| Attribute / typed value | Checked namespace/name/rawValue refs and 8-byte Res_value size/type/data; a missing optional ref is `0xffffffff`, not pool index 0 |
| End element / CDATA | Matching expanded element name on an explicit stack; checked text/typedData; no second root, unmatched close or character data outside the root |

String refs are checked before conversion or lookup. Rendering preserves namespace
meaning, attribute values and node order; it does not sort to make output stable.
Raw string values are preferred when present; otherwise typed values use Android
resource/attribute IDs, booleans, signed/hex integers, FP, packed dimension/fraction
and color formatting from the reviewed reader. Unsupported typed values remain
explicit raw-type/data text, not a guessed resource lookup. The decoder does not
load resources, resolve a network namespace or invoke Android tools.

The renderer escapes XML metacharacters and checks XML character/name legality;
illegal characters are errors, not replacement characters. Plain UTF8 XML entries
require an actual well-formed, single-root check; seeing a leading `<` does not
prove validity. External entity/DTD expansion is unsupported and never accesses
files or the network. Output is complete UTF8 XML or a classified error; line
numbers, byte layout and quote style are not source-byte identity commitments.

Parsing, rendering and release use explicit stacks/arenas and checked growth.
A reused decoder MUST expose the required cancellation/allocation points and
return errors for every reachable malformed read; a skeleton precheck,
`catch_unwind` or the existing PATCHES cannot prove that contract. A known but
unhandled legal chunk/profile is Unsupported; a broken size/ref/stack/string is
Malformed; cancellation/resources retain their own kinds. Extraction remains the
way to retrieve bytes that the XML presentation cannot express.

## 15. Single-use integer expression inlining

When inline_pure is false, return the validated JavaBody directly; when it is true, run one bounded
scan:

1. Match only the adjacent `t = expr; return t;` inside the same original block, where t is an
   ordinary OpResult local with exactly this one use.
   Do not match Phis, shadows, parameters, DeferredAllocation or any other control/protection
   domain.
2. expr contains only int/long literals, local reads, integer unary operations, explicit integer
   conversions, add/sub/mul, bitwise operations and shifts; it contains no div/rem, FP, heap, ref
   cast, call, allocation or synchronization.
3. Check the resources of the body copy, the replaced nodes, the re-validation and the baseline
   complete emit only against explicitly configured finite quotas; an insufficient quota means
   Declined, and no estimated rejection threshold is set by default. Copy one JavaBody, scan once in
   the original AST order and replace disjoint pairs; do not run repeat,
   do not try modifying the original body in place, and build no plugin/option registry.
4. Move the expr tree unchanged into the return, preserving the source type and the Origin of both
   sites; a pure local declaration left without a use MAY be kept. Re-validate
   type/binding/control/effects and the complete emit quota.
5. If it passes, the result is Applied; an ordinary mismatch or a restricted budget is Declined;
   Cancelled stops; an internal invariant failure is a BUG and MUST NOT be wrapped as Declined.
   Acceptance compares the on/off results of the same fixed behavior, not the text or the local
   count.

The expr tree order is unchanged and nothing executes between the two statements, so the operand
reads the same lambda, the integer bits are unchanged and the original return value is unchanged.
FP is excluded to avoid store elimination involving different floating-point execution
environments; throwing/effectful expressions are excluded to avoid adding evaluation/protection
domain obligations. Inlining does not change the CFG/SSA/CopyPlan.
