# 04 — Value facts and use interpretations

Code: `crates/diana-decomp/src/p4/`. The owning contract is
[PIPELINE_DESIGN §5.4](../PIPELINE_DESIGN.md#54-p4值事实与使用解释产出-typedmethod).
Engine's private method task consumes `SsaMethod` through `build_types` and receives
`TypedMethod` or `TypeStop`. P4 retains P3 privately; no preceding product can be
extracted or reconstructed through this interface.

## Input and publication

P4 consumes existing `SsaFacts` and borrowed `ExecutionFacts`: definition origins,
ordered uses, exact widths, Phi incoming/transfer identities, source availability,
operation kinds, original reference signatures and normal/exception completions.
The current class's P0 declaration travels inside the same method product chain
(`DecodedMethod` → `FlowMethod` → `SsaMethod`), so `this`, its method declaration and
the class's direct superclass identity are consumed as delivered facts. A restricted same-DEX superclass view travels with that declaration. P4 does not
receive a separate context parameter or query ProgramIndex itself. It consumes the
P0 relation result through the retained execution view; it cannot invoke a P0
constructor, read method bodies or search another DEX.
P4 MUST NOT decode code, reread reference records, reconstruct register sources or
change SSA identities or control flow.

P4 records an outcome for every retained value, use and Phi incoming. Known source
facts, representation choices and material unknowns remain distinct. An unavailable
SSA source does not become a zero/null literal or a fabricated value. Unavailable
invoke grouping remains an operation-level issue rather than zero arguments.

Local constraints do not gate the method. A missing declaration, incompatible use
or uncertain incoming preserves independent facts and views in `TypedMethod`.
Construction Pending is distinct from an unavailable SSA source and appears only
in unpublished records. Actual construction/resource failure retains P3 and unpublished `TypePartial`;
that snapshot cannot be promoted to a mature product. Allocation and arithmetic
failures retain their classification, and method-task stops do not truncate siblings.

## Products

| Product | Meaning |
|---|---|
| `ValueFacts` | Existing ValueId, allowed representation families, original/derived type sources, literal provenance, integral range and reference identity provenance. |
| `UseView` | Existing UseSite/StateId, semantic or descriptor requirement and established interpretation or concrete problem. |
| `PhiView` | Original Phi identity, literal or runtime representation and an interpretation for every original incoming transfer. |
| `ConstructorFact` | Original invocation, receiver state, identity provenance and established or unresolved construction relationship (`Allocation`, `This`, `Super`, `Ancestor`, `OtherOwner`, unknown). |
| `InitializationUse` | Initialization states at the original use position, including failure, mixed paths and unknowns; a finite allocation comparison when a single original allocation is known. |
| `ArrayAccessView` | Original get/put/fill operation and SSA array state, with component bounds at that execution position and explicit coverage gaps. |
| `ArrayFillView` | Original fill operation and SSA array state, with a source-supported primitive element interpretation or local problem; width/count/bits remain in the execution payload fact. |
| `CheckView` | One original `check-cast`, in operation order, with its target name when it resolved and the pre-check disposition of its membership: `Redundant` with typed evidence, `Retained`, or `Unknown` with a concrete gap. |

`ValueFacts::allocations()` preserves known new-instance origins through the
existing copy/Phi rules, including partial sources. This set is not a dynamic
identity proof. `AllocationBinding` compares a captured reference with the
allocation result at the original use position using P4's existing correlated
reference solver. `Same` is published only for a dominating allocation definition
and an exact same-reference result; `Different` and `Unknown` remain distinct.
`ConstructorFact` carries the corresponding receiver binding. P5 consumes these
finite facts without access to the private alias solver or another register
source analysis.

`Owner` type origins retain the original class key and TypeId. The current-class
type is therefore comparable by identity, and comparison with the declared direct
superclass identity is constant-time when that declaration was read. A superclass
descriptor that could not be read keeps its original identity and error: it is not
evidence that an owner is equal to, or different from, that unresolved parent.

Type-name identities are method-local analysis identities, not synthetic DEX TypeIds.
Original names retain exact UTF-16 units and parameter/reference origins, including
original type identities when supplied. Component projections name their original
array source and remove one actual array dimension. Intrinsic names derive from
opcode semantics, not external declaration files. A declared reference descriptor
is not an exact runtime-class or complete-hierarchy claim.

## Constraint construction

`constraints.rs` consumes the existing operation enum and checked declarations.
Method entry/return declarations, fields, call prototypes and allocation
references supply declaration constraints. Original casts supply position-related
check events rather than value-definition seeds. Unary/binary/compare, array and terminal semantics
supply their own representation requirements. Polymorphic calls use their supplied
prototype independently of the separate target declaration.

Producer facts and consumer requirements are different constraints. A known int
producer does not become float because one use requests float. Equal width alone
does not establish compatibility. Original conversion instructions remain at their
original operation positions; a use view does not insert a replacement conversion.

### Position-related check bounds

`refinements.rs` consumes the original cast target, SSA receiver binding and
normal/exception transfers. A check-cast has no register write or SSA output.
P4 MUST apply its checked bound only after normal completion and only to a
reference established to be the checked reference. The input's original source
declaration remains unchanged; a successful array check adds a position guarantee,
not another alternative alongside an Object source declaration. The bound MUST NOT
leak to a preceding position, exception path or unproved alias.

Private queries identify a StateId, PointId and component-projection depth.
Copy relations preserve the reference; a Phi rewrites the binding on each original
incoming transfer before joining that path's knowledge. Both references in an alias
query select the same transfer. A reference-valued array load projects its source
at the original load execute point; this type dependency does not identify the
loaded element with the array object. No register environment or second SSA chain
is constructed. `references.rs` supplies the same immutable source/alias rules to
this consumer and initialization.

Alternative paths retain known bounds and independent gaps. A successful check
can discharge declaration-projection gaps for its covered normal path, but cannot
repair unavailable SSA input. Uncovered unknown siblings remain unknown. Null-only paths are
grounded but supply no element type; a successful cast of null MUST NOT contribute
a component type to a different non-null alternative. Neither null nor construction
bottom establishes an array execution or non-nullness.

### Pre-check redundancy of an original check-cast

`redundancy.rs` answers one question for each original `check-cast` without
running another CFG, SSA or dominance chain: does every reaching transfer already
establish that this exact reference is null or an instance of this exact original
target before the check executes? Only two original events establish it.

1. A same-target `check-cast` that completed normally on the same value. Its
   exceptional completion establishes nothing.
2. An `instanceof` on the outcome where its result is nonzero, resolved through
   the original result value, its P3 terminal use, the original branch outcome and
   the P4 copy/alias rules. The zero outcome and a merged boolean establish
   nothing; `instanceof` is false for null, so its true outcome also proves
   non-nullness.

Target identity is the original type identity, never a descriptor spelling,
declared type, generic argument or assignability guess. The value comparison uses
the same private alias relation as component refinement. A local proof is read at
the transfer leaving its establishing point, so the check's own success never
justifies that same check. The disposition is a must-fact: a node without reaching
transfers is unestablished, a join requires every reaching transfer, and the
least fixpoint starts from unestablished, so a loop's first failing check stays
retained. A merged alias, an undefined or unavailable reaching state, and an
unresolved type reference or operand layout publish `Retained` or `Unknown`; they
never remove a check and never fail the method. The worklist grows finite
point/state sets from the input.

The unpublished `TypePartial` carries no `CheckView`, so a stopped P4 never
publishes a disposition. P7 consumes the published disposition; P4 does not decide
display.

The sparse worklist grows finite name/gap sets. Queries beyond every input
descriptor's array rank share one no-nominal-evidence depth; this closes recursive
component dependencies without a numeric quota. Queries retain descriptor-suffix
references and materialize only components demanded by published access facts,
rather than expanding every dimension of unrelated declarations. An ungrounded cycle acquires
Unknown rather than vacuous completeness. Projection finishes before Copy/Phi
source propagation and family selection; no consumer retries an earlier stage.

### Exception declaration bounds

For each exception value, P4 uses the original incoming exception edges to collect
all catch declarations that reach that handler. `Catch` origins retain TypeId and
exact descriptor text. A shared handler keeps the alternatives rather than choosing
its first typed catch. Catch-all supplies a Throwable bound. A failed declaration
read keeps its original execution fact and a local missing-descriptor issue;
a readable non-class catch retains its classified input-use issue and an
`InvalidCatchType` problem. Neither becomes an inferred type from a consumer.

A caught object's original exception definition and its declaration alternatives
remain available independently of ordinary reference reads. P4 does not propagate
a separate exception-provenance domain merely to prove Java assignability.
A Throwable bound is not an alternative runtime-class claim alongside a more
specific typed catch. Reading a reference does not require resolving that hierarchy.

## Finite source propagation

`solve.rs` indexes Move/Phi rules by source ValueId. Position-related component
bounds are established first; moves and Phis propagate those producer facts and
literal/range/identity evidence only to affected rules. A cast retains the same
reference binding rather than introducing a value rule. Known type sources grow;
component projection strictly removes array dimensions. Original names are not
rewritten to a chosen common representation. Opcode/prototype seeds establish
producer family bounds; only complete component knowledge supplies additional
producer bounds. Copy/Phi relations propagate representation constraints. The
solver MUST NOT treat a propagated declaration subset as exhaustive source coverage.

Literal evidence retains original bits separately from representation. A move or
Phi can retain a literal only when all grounded sources agree on width and bits.
A nonliteral/unavailable or different-bit source ends that literal evidence.
Unrooted cycles supply no literal or zero evidence. Thus zero can have independent
null/integer/float use views without turning an arbitrary runtime integer into null.

Integral ranges are seeded by original integral bit interpretations, descriptors
and opcode-defined ranges. Moves copy ranges and Phis take hulls. Range endpoints
come from these fixed seeds; consumer demands and conditional tests do not invent
narrowing or zero facts. P4 does not constant-fold arbitrary arithmetic.

Identity provenance follows existing SSA aliases. Different representation types
or equal type names do not establish object identity. A nontrivial loop merge
cannot identify an earlier dynamic allocation with a newly executed allocation
site merely because the static producer is the same.

## Representation and views

Representation families distinguish integral words, long, float, double and
references. Compatible nonliteral relations use union/find and intersect allowed
families. A contradictory relation remains local rather than merging an impossible
group and erasing independently established producer facts. Ambiguous producers
with contradictory requirements retain the conflict instead of choosing by visit
order. Pure literals have separate use-site views.

`views.rs` first establishes the machine read from source availability, width,
required representation and genuinely required missing facts. Direct reads preserve
the existing value; literal reads preserve the original bits in the selected family.
An available reference-family source uses `Direct(Reference)` without proving
subtyping, Java source binding or initialized-object state. Integral/long reads do
not require a use-site declaration's numeric range. Source declarations stay in
`ValueFacts`; original use declarations stay in `Requirement::types`. Neither is
rewritten to match the other.

An optional declared integral view is selected only after the read is established.
Its range proof comes from the source's existing facts or exact literal bits, never
from the consumer's desired type. Missing or insufficient range evidence leaves
the established direct/literal read intact; it is not a read-recovery problem.
Exact literals keep their literal view when the declaration adds no narrower
value-preserving form. A selected IntegralView may also refer to a literal whose
bits remain in ValueFacts. Literals and runtime values follow the same separation.

A declaration such as Z does not by itself prove canonical boolean values or
request an implicit conversion. Downstream expression building MUST distinguish
an ordinary integral read from a range-proved declared view; it MUST NOT infer
boolean naturalization from the declaration alone. Keeping the original word
expression and original binding does not require such naturalization. P4 does not
add bitwise range rules or manufacture [0,1] bounds to make a read available.

Unavailable SSA sources, unresolved argument grouping, representation conflicts
and required missing facts retain their existing local outcomes. Literal null still
requires established one-word zero provenance; an arbitrary runtime int does not
become a reference. Initialization and array-component interpretation remain
separate consumers with their own actual fact dependencies. Original casts,
including their resolution/check/exception behavior, remain execution operations.
Original integer conversions and storage truncations likewise remain at their
original operations; their input reads do not require a prior losslessness proof.
Actual representation conflicts and original classified use issues remain distinct
from inability to select an optional value-preserving view.

Bit-preserving move and word/wide storage operands retain a copy/bits view when no
numeric interpretation is required. A complete, consistent declaration context can
select a literal's integral/floating bit view after that read is established; it
does not convert a runtime value. Literal equality predicates can use the integral
bit interpretation.
These choices do not select a global type for other uses of the same constant.
Pure-literal Phis need no common runtime storage representation: every incoming
retains its bit identity and each consumer has its own interpretation. Runtime
Phis have a common representation and per-transfer incoming views. Available
literal inputs constrain a runtime Phi's local allowed families before nonliteral
groups are joined. The shared literal rule admits reference interpretation only
for one-word zero; nonzero words remain integral/float candidates and wide bits
remain long/double candidates. It does not choose an interpretation for that literal
at another use. Pure-literal Phis retain independent use views.

Numeric Phi transfers copy their existing word/wide encoding rather than evaluate
a numeric expression. A nonselected numeric Runtime family set denotes common
bit storage; incoming views preserve CopyBits/LiteralBits and P5 consumes that
storage without choosing int/float or long/double. Candidate sets that mix
references and numeric data still require a compatible representation and retain
unresolved incoming problems. Ordinary numeric reads keep their own interpretation
requirements; a ready bit transfer does not certify such a read.

A conflicting local Phi constraint does not rewrite an independent producer's
established representation. Unavailable inputs supply no literal evidence, and an
unavailable incoming does not acquire the known sibling's value or adaptation.

### Invocation identity and source expression

P4 reads parameter/return declarations from the original invocation prototype;
it does not choose an overload using argument types. The caller-visible return
source comes from that prototype, not a source-language rebind or an expected
result type. The original invocation kind, logical DEX reference identity,
prototype, ordered bindings and completion rules remain in the retained execution
product; P4 neither clones a second call directory nor changes those facts.

[Signature-qualified calls](../PIPELINE_DESIGN.md#5731-原目标签名限定) express that
existing binding without converting an operand. They do not prove assignment
compatibility or bypass an original runtime check. A source-expression consumer
cannot repair unknown binding by adding casts, boxing, helpers or typed aliases
merely to influence Java overload selection.

## Array component knowledge

`components.rs` is the sole declaration-projection producer. Allocation projection
and the position-related solver use its results. `refinements.rs` supplies one
component knowledge record for each original access; load-result sources, optional
store context and payload interpretation consume that record. Projection uses
already delivered descriptors and SSA availability, not consumer demands,
method-name patterns or classpath facts.

Knowledge consists of known component sources K and gaps U. Joining alternatives
unions K and preserves every participating gap. A declared array removes one
dimension with its original type origin retained. A non-array declaration bound
supplies an UnprojectableDeclaration gap, not proof that the runtime object is
non-array. Unknown/unavailable alternatives remain gaps beside known arrays.
Empty knowledge is internal bottom until source closure; ungrounded components
become unknown, never vacuously complete.

`ArrayAccessView` covers each original get, put and payload fill. Its component
knowledge remains available even when ordinary reads are established. Complete
coverage means all participating non-null alternatives have projectable bounds at
that position; it proves neither an exact runtime reference component class nor
non-nullness. A covering successful check can supply a bound absent from the
original declaration. P4 MUST NOT drop known sources or uncovered gaps merely to
turn a mixed projection into a complete one. All-null knowledge remains distinct
from an established element type.

Opcode semantics establish the receiver/index/value read requirements and the
normal load-result encoding. A missing component declaration MUST NOT independently
block those reads. Word/wide stores preserve bits; reference stores read references
or established null without proving assignment compatibility. Complete component
declarations supply optional use context, not stronger storage-read requirements.
Partial declaration context stays in ArrayAccessView rather than masquerading as
a complete Requirement. Known source components do not override unknown branches.

Original null, bounds, component and covariant-store checks, storage truncation
and normal/exception effects remain in execution facts. Consumers MUST NOT add
casts, conversions or helpers to compensate for an unknown component, or omit
checks because declared projection coverage is complete.

## Array payload interpretation

P4 consumes the operation-bound `ExecutionFacts::array_payload` and the existing
SSA array source and its existing ArrayAccessView component knowledge; it does not
project a second set of array declarations. `ArrayFillView` selects Boolean, Byte,
Char, Short, Int, Long,
Float or Double only when every known array source supports the same component
interpretation, no source is unavailable/unknown, and payload width agrees.
Equal payload widths do not select a family: `[F` and `[I` keep distinct bit
interpretations. Mixed component interpretations, non-primitive components,
width mismatches and unavailable sources remain local problems with original data
retained. A payload-view failure keeps its FailureKind in the local problem and
its full error through the operation-bound execution query; it is not relabeled
as an absent payload. An unestablished component never becomes an inferred int array.

The view records the element interpretation rather than allocating one SSA value
per payload element. Consumers pair it with the borrowed original bits; integer
signedness, floating bit patterns and noncanonical values are not replaced by
numeric conversions or normalized literals. P4 does not infer array length from
payload count, pad/truncate data, introduce stores or merge the fill with allocation.
Original effects and normal/exception completions stay at their original positions.

## Position-related initialization

`initialization.rs` relates constructors to existing receiver bindings and original
allocation/constructor signatures. Matching does not require a constructor body.
This does not decide whether P7 can merge the operations into natural `new` syntax.
A constructor call whose receiver is `this` establishes `This` when its owner is
the current class, `Super` for the declared direct superclass and `Ancestor` for a
proven indirect superclass. The last case retains the original invocation owner;
it is not a Java `super()` expression. The same-DEX declaration query distinguishes
a proven unrelated owner from an unresolved chain. `OtherOwner` requires the
former; the latter keeps `UnknownOwner(SuperclassGap)` with the missing definition,
original input error or other concrete reason. It is not owner-mismatch evidence.
A new allocation still requires its exact allocation owner; ancestor acceptance
applies only to constructor-this sources. Constructor relationships follow the existing Copy/Phi source
rules rather than requiring one provenance root; check-cast retains the same
reference binding. Equal source relationships retain
that relationship even when identity is Multiple; different relationships retain
the existing ambiguous summary. The same source-relation facts drive initialization.
All possible pending sources must match `Allocation`, `This`, `Super` or `Ancestor` before
normal completion establishes their initialization. Sources whose producer
semantics carry no pending obligation do not invalidate that proof. `OtherOwner` and unresolved pending sources
are not successful-initialization evidence. Repeated construction retains its
separate conflict rules.

The initialization solver asks for an original SSA reference binding at an original
PointId. Entry/producer semantics supply initialized references, null or pending
allocations/constructor `this`; an unclassified producer supplies Unknown, not an
initialized default. Null is distinct internally from a constructed object. The
published Initialized state means no pending construction obligation, not non-null.
Only actual reference interpretations and independently required unavailable
reference inputs create use demands; a numeric value's reference candidate alone
does not create an object-initialization issue.

Sparse dependencies follow P3's transfers and the existing source rules. A Phi
selects each source on its original incoming transfer, including that transfer's
normal/exception initialization effect before joining. Copy captures a reference,
not an initialization snapshot; check-cast introduces no replacement binding.
Later constructor events still affect proven aliases. Successful and exceptional completion remain distinct even for
void calls with no register-result commit. Failure preserves its state and already
established effects; it is not heap rollback.

Paired observations retain initialization states separately for equal, distinct and
unproved reference relationships. Both bindings select the same incoming transfer;
equal sets of possible roots do not prove aliasing. A new normal allocation is
distinct from references already present at its execute point. A static allocation
site does not identify one dynamic object across iterations. Insufficient relation
facts retain uncertainty; no register environment or complete-method alias tracker
is reconstructed. When an additional constructor receiver cannot be related to
either observed binding, the solver conservatively preserves the affected gap.

Finite bit-set worklists solve the demanded dependencies. Straight-line identity
segments are collapsed without crossing observed definitions, joins or relevant
events. Bottom means no evidence; an ungrounded cycle becomes Unknown, never a
vacuously initialized result. Unavailable source alternatives remain unavailable
rather than borrowing a known sibling's state.

A direct call with an unreadable declaration can still be a constructor call.
P4 records an uncertain initialization event without manufacturing a ConstructorFact.
It retains the original reference-read error in the execution product and records
the affected call site. Both normal and exceptional successors preserve uncertainty
for potentially aliased uninitialized roots; proven-disjoint roots and already
initialized roots are not reset. Receiver provenance comes only from existing SSA
bindings, not raw-register replay. A known non-constructor call is not reclassified
as a constructor merely because another fact is unavailable.

Mixed paths retain the participating initialization states. Repeated initialization
or reuse after failed construction does not reset to a known successful state.
An unknown receiver is not presumed disjoint from a live uninitialized object;
separate fresh allocations have explicit disjointness. Reference provenance roots
from arbitrary calls/loads do not prove disjointness. Multiple identity alone does
not create an initialization gap: all initialized incoming states join to
Initialized. An actual unresolved object relationship remains analysis uncertainty,
not a missing declaration. Constructor-owner gaps retain their own diagnostics. Missing definitions outside
the current logical DEX remain unknown even when another supplied DEX contains
a definition; this query does not perform cross-DEX resolution. Successful
normal completion is still required to establish initialization. The ancestor
relationship alone changes neither the original invocation nor exception behavior.

## Inspection and evidence

The existing `dbg_shell` supports `p4`/`types` and `p4-all`. Detailed views retain
original sites, type origins, bits, Phi transfers, constructor relations,
initialization states and component sources/gaps. Unknown component counts are
separate from unreadable use views and typed recovery issues. Batch traversal completion is not a semantic correctness
badge. P3 views still observe retained SSA through the same P4 method chain.

The previous core test was removed by operator decision. No independent
verifier runs on ordinary requests. Observed runs and missing execution evidence
belong to the engine README; constructor classification or zero construction
stops do not certify every initialization path.
