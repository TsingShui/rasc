# 04 — Calls and declaration facts

Code: `crates/diana-decomp/src/p4_calls/`; entry: `normal.rs::collect`.
Declaration evidence is supplied by P0 `DeclarationCatalog`; original method
and field references come from P1.

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

#### 3.4 DeclarationCatalog and minimal call binding

The concrete immutable data tables are TypeDecl, MethodDecl, FieldDecl and HierarchyEdge. FieldDecl additionally records
`JavaConstantKind=Constant|NonConstant|Unknown`, which prevents sget from being constant-inlined by javac; a raw encoded
value does not by itself prove the constant attribute of the external source. Each owner/candidate
group has Complete/Partial/Unknown plus its source. An empty catalog is legal, and missing evidence affects only the corresponding Java capability; the catalog MUST NOT read files,
query the network, spawn or scrape an all-platform database. EnvironmentId identifies only this request's facts snapshot, and Diana MUST NOT cache content across requests.

```text
lookup_type(TypeRef) -> Fact<TypeDecl>
lookup_field(FieldRef) -> Fact<FieldDecl>
lookup_method(MethodRef) -> Fact<MethodDecl>
overloads(owner, name, invoke_kind) -> {candidates, completeness}
subtype(a,b) -> ProvenYes|ProvenNo|Unknown
```

subtype traverses the known parent/interface graph with a visited set and an explicit queue, and charges loops/depth/candidate count; it MUST return ProvenNo only when the complete ancestor set was exhausted without a hit, and it MUST report a missing node as Unknown. overloads includes the facts required for inheritance/hiding and access; P4 MUST NOT let a candidate traversal replace the Java selection with the first arriving item.

P8 uses fixed-arity calls with exact formal static types, and it MUST allow only known reference/
primitive widening as candidate applicability, without introducing boxing or varargs expansion. P8 MUST require the applicable set to contain a unique most-specific erased
target that agrees with the original invoke resolution. P8 MUST NOT guess generic inference, return-only collision or unknown parent contracts
in this profile. See [04-calls](04-calls.md) for the complete steps.
