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
the class's direct superclass identity are consumed as delivered facts. P4 does not
receive a separate context parameter and does not query the class index itself.
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
| `ConstructorFact` | Original invocation, receiver state, identity provenance and established or unresolved construction relationship (`Allocation`, `This`, `Super`, `OtherOwner`, unknown). |
| `InitializationUse` | Initialization states at the original use position, including failure, mixed paths and unknowns. |
| `ArrayFillView` | Original fill operation and SSA array state, with a source-supported primitive element interpretation or local problem; width/count/bits remain in the execution payload fact. |

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
Method entry/return declarations, fields, call prototypes, allocation and cast
references supply type seeds. Unary/binary/compare, array and terminal semantics
supply their own representation requirements. Polymorphic calls use their supplied
prototype independently of the separate target declaration.

Producer facts and consumer requirements are different constraints. A known int
producer does not become float because one use requests float. Equal width alone
does not establish compatibility. Original conversion instructions remain at their
original operation positions; a use view does not insert a replacement conversion.

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

`solve.rs` indexes dependent rules by source ValueId. Move, Phi, cast identity and
array-component facts propagate only to affected rules. Known type sources grow;
component projection strictly removes array dimensions. Original names are not
rewritten to a chosen common representation.

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

`views.rs` establishes direct reads, literal bit interpretations, null views and
range-supported integral views. An available reference-family source is read as
`Direct(Reference)` once its required reading facts are established. This means
passing the existing reference unchanged, not proven subtyping, Java source
binding or initialized-object state. Source declarations stay in `ValueFacts`;
original use-site declarations stay in `Requirement::types`. They are not rewritten
to match one another. Unknown hierarchy alone is not a missing read interpretation
and does not produce a recovery issue.

Unavailable SSA sources, unresolved argument grouping, representation conflicts
and required missing facts retain their existing local outcomes. Literal null still
requires established one-word zero provenance; an arbitrary runtime int does not
become a reference. Initialization and array-component interpretation remain
separate consumers with their own actual fact dependencies. Original casts,
including their resolution/check/exception behavior, remain execution operations.

Bit-preserving move operands retain a copy/bits view when no numeric interpretation
is required; literal equality predicates can use the integral bit interpretation.
These choices do not select a global type for other uses of the same constant.
Pure-literal Phis need no common runtime storage representation: every incoming
retains its bit identity and each consumer has its own interpretation. Runtime
Phis have a common representation and per-transfer incoming views. Available
literal inputs constrain a runtime Phi's local allowed families before nonliteral
groups are joined. The shared literal rule admits reference interpretation only
for one-word zero; nonzero words remain integral/float candidates and wide bits
remain long/double candidates. It does not choose an interpretation for that literal
at another use. Pure-literal Phis retain independent use views.

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

## Array payload interpretation

P4 consumes the operation-bound `ExecutionFacts::array_payload` and the existing
SSA array source. `ArrayFillView` selects Boolean, Byte, Char, Short, Int, Long,
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
the current class and `Super` when its owner is the declared direct superclass; a
different readable owner keeps the original invocation and is reported as
`OtherOwner`. Constructor relationships follow the existing Copy/Cast/Phi source
rules rather than requiring one provenance root. Equal source relationships retain
that relationship even when identity is Multiple; different relationships retain
the existing ambiguous summary. The same source-relation facts drive initialization.
All possible pending sources must match `Allocation`, `This` or `Super` before
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
normal/exception initialization effect before joining. Copy/Cast captures a
reference, not an initialization snapshot: later constructor events still affect
proven aliases. Successful and exceptional completion remain distinct even for
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
not a missing declaration. Constructor-owner gaps retain their own diagnostics.

## Inspection and evidence

The existing `dbg_shell` supports `p4`/`types` and `p4-all`. Detailed views retain
original sites, type origins, bits, Phi transfers, constructor relations and
initialization states. Batch traversal completion is not a semantic correctness
badge. P3 views still observe retained SSA through the same P4 method chain.

The existing sole core test uses public Engine products with independently encoded
DEX and expected interpretations. No independent verifier runs on ordinary requests.
Observed runs and missing execution evidence belong to the engine README; constructor
classification or zero construction stops do not certify every initialization path.
