# 04 — Types and declaration constraints

Code: `crates/diana-decomp/src/p4_types/`; entry: `normal.rs::solve`.
Declaration queries and constraints are in `p4_types/declarations/`.

P4 consumes the P3/P4 products and propagates value types, primitive views,
reference shapes and call-use conversions. Its initialization products track
allocation and constructor use. P4 does not alter the original control-flow graph.

The original bit pattern and register width MUST remain recoverable. Integer,
floating-point and reference interpretations are not interchangeable facts.
Unknown hierarchy or use conversion MUST NOT be reported as a proven conversion.

The current solver can decline when a type or use conversion is not supported;
a declined result currently prevents later Java lowering. Readable output under
uncertainty is a product requirement but is not yet generally implemented.

## Zero constant use views

P4 builds word-zero provenance from original zero constants, ordinary moves and
Phi inputs using a finite worklist. A Phi is known zero only when every incoming
origin is zero. Parameters, runtime computations and unanchored cycles cannot
establish zero provenance. A zero input does not force a Phi with nonzero
reference inputs to have an integer type. A Phi whose inputs are all proven zeros
retains an integer storage view when no other type constraint selects its
representation.

P4 supplies sealed zero evidence and a typed view: integer `0`, boolean `false`,
float positive zero, or reference `null`. P5 uses that evidence when a normal-edge
Phi copy needs a different representation. Nonzero inputs still require compatible
storage types; the engine MUST NOT reinterpret an arbitrary integer as a reference.
Zero provenance traversal charges work and checks cancellation. P4 caches these
facts in `TypedBody`; independent source replay checks the original definition
graph rather than treating cached zero evidence as complete method validation.

## Call and reference uses

P2 call formals use the same `DeclarationType` as fields, returns and arrays.
P4 consumes those formals directly for numeric-family constraints and argument
views; a call's owner remains its original DEX identity, not the receiver's
storage type. Every completed call-use product contains all argument views.

An ordinary receiver contains a `TypedUse`, so Object, String, Class, Throwable,
arrays and nominal or projected references do not require an invented TypeId.
A constructor receiver names the existing initialization product. A super
receiver retains its checked this origin. P4 does not add a second alias tracker
or infer initialization from the method name alone.

Calls, reference assignments, casts and returns consume the same zero and
reference-value facts. A zero use selects a null literal; other reference uses
preserve the existing value without adding a throwing cast. These views do not
prove source subtype or access legality. Numeric families, boolean conversions
and lossless call/return narrowing retain their own requirements; primitive
field or array stores retain their format-defined write conversions.

## 4. Types and Java expressions (T10/T11/T15)

### 4.1 Domain, constraints and solving order

P4 supplies declaration and proto constraints first, P4 then solves value and use types, and P7
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
declaration and proto facts and does not require knowing P4's final source type in advance.
A sealed source type is not reselected in P5/P7 for printing convenience; a failed check MUST
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
P5 no longer guesses types on its own.

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
source storage type, so P4 and P4 do not wait on each other.

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
after sealing, check every original operand constraint and do not reselect in P5
```

The defaults only resolve literal interpretations that no producer or use can distinguish; they
MUST NOT override a known F32 producer. When several non-literal uses require incompatible
families, the implementation MUST NOT search cast combinations or introduce a bitcast library.
Building the family graph is near linear (union-find amortized
O((values+constraints)*alpha(values)), and every edge, merge and set intersection grows through checked reservation.

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
grow per new fact; the implementation MUST NOT claim that this step is strictly linear in
the input bytes. The worst case is roughly the number of reachable facts multiplied by their
propagation edges, and it is bounded by the finite value and constraint domains.

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
avoid an unintended Unicode escape; Java source escaping is not reused for JSON debug snapshots. Classfile MUTF8 limits do not constrain readable source. The printer preserves
all original UTF-16 units within actual platform capacity and configured output
capacity checks; it does not split values, change intern identity or invent runtime helpers
for compilation.

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
Each change and propagation has bounded progress; the implementation MUST NOT add a general
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

## Declaration constraints and call binding

Code: `crates/diana-decomp/src/p4_types/declarations/`; entry: `normal.rs::collect`.
Declaration evidence is supplied by the request-level `context::Context` cache of
P1 declaration products; original method and field references come from P1.

P4 associates SSA call and field sites with original identities, invoke kinds,
signatures and available declaration facts. It also collects initialization,
throws, ownership and binding obligations for later stages. A DEX reference
contains an owner, name and prototype; it does not alone establish complete
overloads, access flags, hierarchy or declared exceptions.

## 9. Call binding, objects and constructors (T11/T12)

### 9.1 CallEvidence generation and discharge

P4 keeps the original owner/name/proto/invoke kind, the word operands, the complete or unknown
declaration set, and access/throws/class-init facts. Each owner/name group includes
inheritance/hiding candidates, and an incomplete candidate set MUST NOT be marked Complete.
P4 generates only P4 constraints and BindingObligation and does not require the P4 source types in
advance.

P7 first fixes the MemberEmission of the class request per §12.1. The following steps apply to an
ordinary Java call that is actually printed; an original call already consumed by a complete
Generated template is validated by that rule and MUST NOT pretend to be an independent source
call undergoing overload selection. The ordinary-call candidates are source-visible declaration
projections, and a proven compiler bridge MUST NOT be treated as another hand-written
return-only overload; the original runtime directory is still fully kept for checking the
original invoke/dispatch identity.

P7 prints each call from the original reference and invoke kind using the P4
receiver and argument views. It MUST preserve original receiver/argument order,
formal descriptors and dispatch identity. It MUST NOT add a throwing cast, boxing,
helper or changed dispatch merely to make source compile.
Java overload/access/checked-throws acceptance is not a method output gate.
Original Throws annotations are retained, not inferred from catch parameters.
Field reads and writes use original references; absent declaration flags remain
unknown, not presumed public, nonvolatile or nonconstant.

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
assigning null to uninit. Constructor target/prototype and receiver origin remain checked; source access,
overload and checked-throws legality do not gate output. T22 has separate rules for outer capture, and a general pre-super
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
binding candidates. P4 follows SSA definitions rather than maintaining register
roots. It checks that the receiver originates at UninitThis, that move families
and argument word/reference/wide families agree, and that a reference argument is
not UninitThis. The graph consumer discharges the recorded binding and checks
acyclic this-delegation until the original parent link. Missing evidence or a cycle
leaves initialization unproved; it never supplies a default link.

P7 MUST consume the graph certificate before sealing a delegating constructor.
Contributors MUST NOT use an entry product to admit the callee body as Java or to
bypass that body's ordinary P1–P8 chain. A failure after the entry endpoint does
not erase the checked link or change the callee's own classified result. The query
uses the selected method's existing cancellation control; it is not a
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
the availability of each use to P4 and MUST NOT add a new physical write to SSA.

P5 uses `LocalBinding::DeferredAllocation(A)`: the pre-init aliases have no readable Java local and
are also not assigned a default null. One ConstructorExpr in P7 consumes the new and init sites
and materializes the obj local on success; afterwards these aliases map to the same obj, and an
original reg overwritten later is another ValueId. This is a dedicated storage rule for
constructor legalization, not a general local coalescing.

It preserves the allocation→init→use order and rejects printing `new; effectfulArg(); init` as
computing arg before new. Truly supporting that wider shape requires proving the internal order of
allocation, arguments and init in Java new; the implementation MUST NOT invent a carrier/helper or
reorder by default. Inside a constructor, UninitThis up to super/this likewise becomes a Ref only
after normal completion, and leading local declarations MUST NOT move before the Java constructor
invocation.

### 9.4 Original binding and source spelling

DEX references and invoke kinds identify the original operation. P7 keeps those
facts independently of Java source spelling. Complete Java overload resolution,
accessibility and qualifier scope are not prerequisites for readable output.
P4 typed views still preserve original word interpretation; a source-level mismatch
MUST NOT be repaired with fabricated conversions or changed targets.

### 9.5 Original Throws metadata

P4 retains original Throws annotation descriptors for P7 headers. Missing external
throws facts remain unknown. Production does not enumerate declaration-bounded
escaping exceptions to prove Java catch reachability or rethrow legality.
Original exception edges, handler priority and pre-throw values remain governed
by P2–P6 and checked against P7 control/operation products.

#### 3.4 Input declaration facts and minimal call binding

The concrete immutable data tables are the P1 declaration products cached by
`context::Context`: class declaration, method declaration, field evidence and
hierarchy edges. Field flags come only from input definitions. Missing flags stay
unknown; source constantness is not inferred as an output qualification. Each
owner/candidate group has Complete/Partial/Unknown. A partial directory
retains its readable members and classified issues, and it MUST NOT establish
member absence or hierarchy-wide uniqueness. Missing evidence affects only the
corresponding Java capability; the context MUST NOT read files, query the
network, spawn a process or consult an external database. One context belongs
to one request.

```text
lookup_type(TypeRef) -> Fact<TypeDecl>
lookup_field(FieldRef) -> Fact<FieldDecl>
lookup_method(MethodRef) -> Fact<MethodDecl>
overloads(owner, name, invoke_kind) -> {candidates, completeness}
subtype(a,b) -> ProvenYes|ProvenNo|Unknown
```

subtype traverses the known parent/interface graph with a visited set and an explicit queue, and charges loops/depth/candidate count; it MUST return ProvenNo only when the complete ancestor set was exhausted without a hit, and it MUST report a missing node as Unknown. overloads includes the facts required for inheritance/hiding and access; P4 MUST NOT let a candidate traversal replace the Java selection with the first arriving item.

P7 consumes fixed-arity original calls and P4 argument views without requiring
Java overload-selection proof. Generated-member transformations retain their own
identity/template obligations.
