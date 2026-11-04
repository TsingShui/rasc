# 07 — Java reconstruction

Code: `crates/diana-decomp/src/p7_java/`; ordinary-method entry:
`branch.rs::reconstruct`. Class assembly is in `class_unit.rs`.

P7 consumes the planned method, storage, types, original input and available
declaration facts. It constructs method and class representations and handles
declarations, calls, arrays, constructors, exceptions and generated members.

The product direction is to show readable reconstruction when possible. Material
uncertainties are retained in the report and surfaced on request (`--verbose`),
never by suppressing the readable member. A local failure MUST NOT hide other members.
Neither readable Java nor a class-shaped output alone claims compilability or
semantic equivalence.

P7 produces owned `JavaMethod`, `JavaInitializer`, `JavaField` and `JavaUnit`
presentation products. Diana MUST NOT certify whole-method or whole-class Java
legality. It MUST retain available reconstruction with concrete unresolved facts;
a missing representation uses the classified Debug/Unavailable exit.

Each method runs exactly one analysis chain and selects a result by the following conditions:

1. Cancellation stops the request immediately and MUST NOT continue raw per method.
2. A proven Malformed yields Unavailable with any safe original debug excerpt; it never becomes a normal success. An abstract, native or interface member with no code yields Java with `body: declaration` when its declaration is readable; otherwise its available declaration facts remain visible as Debug or Unavailable.
3. P7 MUST retain a complete renderable reconstruction without requiring Java-legality qualification. It MUST NOT fabricate a missing operation or suppress other completed members. Exact operation, value, copy and control correspondence remain checked against the shared products; an InternalInvariant remains a BUG.
4. When a declaration or source fact is unknown, P7 MUST retain an available readable reconstruction as Java with explicit uncertainty reasons and original identity. It MUST distinguish unknown facts from established facts and MUST NOT claim verified semantics or compilation. If no readable reconstruction is available, Unsupported enters the debug exit once. ResourceExhausted retains its classified failure and enters the debug exit only if that read remains possible. Read only facts whose original ranges/widths are checked, retain their coverage and mark unavailable regions. A confirmed Malformed MUST NOT be revoked by re-reading.
5. When a decoded debug view cannot be produced safely, the result is Unavailable with any safe excerpt. When raw
   newly discovers definite damage, keep the Malformed fact; InternalInvariant keeps its BUG
   status even with raw attached. The implementation MUST NOT rerun normal analysis.

When the normal reconstruction cannot be produced and the original read remains possible, the
result MAY be a decoded debug view; when that read itself fails, the result is Unavailable.
This is a single branch from a failure result to the read-only raw exit, not a recovery
scheduler.

## Class, initialization and compiler-generated members

### Unit assembly order

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
members early. Only hard terminations such as cancellation, global damage or allocation failure
stop per the request rules, and an unprocessed prefix MUST NOT be passed off as a complete result.
Class assembly checks original field and member coverage, preparation availability
and ownership of Generated syntax. It MUST NOT require complete external hierarchy,
Java override compatibility, final definite assignment or non-colliding source
signatures. A readable class without an original constructor remains constructor-free;
Diana MUST NOT fabricate a constructor for compilation.
A missing shell or representation keeps Mixed without hiding independent members.
An original member name MUST NOT be renamed globally; only synthetic locals MAY
be named safely.

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

P7 fixes a source-name plan before sealing Java; P8 only emits it. The plan emits
no imports or static imports. Primitive descriptors map to primitive keywords,
arrays to the component spelling plus `[]`, and known reference types to their
qualified source names. A named nested type uses its proven source owner chain;
an unrelated top-level name containing `$` keeps `$`.

P7 retains the original descriptors and references separately from source spelling.
A spelling that can be printed does not prove Java name resolution, accessibility
or overload selection. Missing facts MAY produce concrete diagnostics; they MUST
NOT deny an otherwise available method. P8 only prints the selected spelling and
MUST NOT repair semantics or rename original members.

### Static/final fields and the constructor chain

- encoded_values are bound in the original static_fields order, and missing values follow the DEX
  type; the implementation MUST NOT sort by field name.
- Source fields are declared first. P7 emits original non-default encoded values
  in static preparation blocks, in original field order, before the original clinit.
  This rule applies equally to final and non-final fields. Original default
  preparation stays implicit; P7 MUST NOT invent writes to satisfy Java definite assignment.
- P7 MUST NOT move original initializer writes into constant-expression field
  declarations or delete reads to satisfy javac. Original clinit may overwrite an
  encoded preparation value; both positions MUST remain represented.
- Constructor chaining preserves target identity, acyclicity and initialization
  order. It MUST NOT require a final-field path-count certificate for display or
  copy delegated field writes into the caller.
- clinit emits `static { initExit: { ... } }`, the original return-void becomes `break initExit`, and
  the printer MUST NOT print a return inside a Java initializer. If the original body is
  necessarily abrupt, wrap it in `if (true) { ... }` to satisfy the normal-completion check of a
  Java initializer; the condition is constant with no dynamic false path, and this is not an
  exception stub or a skipped original body.
  A known clinit without code needs no method; initialization exceptions are not wrapped and are
  handled by the target VM's class-initialization rules.

#### Original preparation and initialization

P1 retains decoded scalar bits or String UTF-16 units alongside original encoded
ranges/tags and bytes. A failed suffix remains partial and never acquires an
implicit default. P7 selects a source expression only for a matching supported
value form; noncanonical NaN preparation remains Unsupported because the existing
expression cannot preserve its payload. Finite FP, signed zero and String units
retain their original values.

P7 prints original preparation before initializer bodies. It does not compute
Java final definite-assignment certificates or aggregate constructor write counts.
P2–P5 initialization and P7 operation/control replay still preserve uninitialized
objects, receiver provenance, original effect order and exception paths.
P8 prints these owned products without decoding or choosing values.

### A generated member always has code

Bridges, the values/valueOf of enums, hidden constructor parameters and inner outer-capture are
language-generated shapes.
P7 uses `MemberEmission::Explicit | Generated(owner-node, rule)`; Generated is still Java member
evidence with code and is never a body-less declaration. The unit printer emits
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
resolution. P7 MUST preserve the original binary identity of the source owner/name chain;
metadata that would require external renaming is not source-admitted. P7 rejects
an inner simple name equal to an enclosing type name. Static public nested classes
consume ordinary method reconstruction and class assembly. The instance profile accepts a public
inner with one synthetic final outer field and a constructor whose sole explicit
DEX parameter is that outer. Its original prefix is the outer assignment followed
by Object initialization; later writes or capture-field references in that
constructor reject the certificate. Other instance field/constructor layouts keep
Mixed and original facts rather than receiving an inferred capture scheme.

P4 records the exact capture declaration. P4 proves receiver/parameter SSA origins;
P5 and P7 independently replay the omitted entry operation. Capture-dependent
source requires certificates for the original constructors before lowering.
A capture read becomes `Outer.this` only for a proved entry receiver; reading
another object's capture field is not interchangeable with that expression.
The generated field product retains its original FieldKey and constructor
MethodKey. Non-public static calls to the top-level enclosing owner require
complete same-nest declarations and matching native access. Existing synthetic
static accessors remain Explicit with their original names and ordinary verified
bodies; no new accessor is synthesized and no cross-class private DEX access is
granted by lexical ownership alone.

P7 supplies a flat owner preorder with checked depths and sealed nested emission
plans. P8 MUST render those plans without inferring hidden fields from flags or
splitting binary names. Original class and method spans use final UTF-8 byte
coordinates, including nested descendants inside their owner spans. Closure work,
depth-stack allocation and output remain checked and cancellable; no numeric default cap
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
contracts. P7 consumes validated original code and checks the constructor entry,
constant allocation/name/ordinal/store sequence, helper array order, values
clone/cast and valueOf class/argument/cast. Exact references, registers, normal
returns and absence of extra operations/protection are required. Declaration
order comes from checked initializer stores, not field-table order. Executable
references to backing storage outside these methods reject the owner. An extra
constructor effect remains original Debug/Mixed rather than being silently
consumed by enum syntax.

Generated method evidence distinguishes bridge method owners from enum class
owners; generated field evidence records constant ordinals or backing storage.
P7 checks unique and complete original field/method coverage before sealing the
enum unit binding. P8 renders the checked enum constants and maps generated method
identities to the final class source span. Mixed output and standalone retrieval
retain original snapshots. A local proof parse error remains a classified class
issue while other members are retrieved; cancellation/allocation/invariant errors
terminate without a partial successful unit.

The native covariant profile accepts a no-argument Object-returning bridge to
the same owner's String-returning instance override. P4 requires the original
public nonfinal override contract and complete nongeneric ancestor evidence.
P7 checks bridge/synthetic flags, the original entry receiver, exact virtual
target, adjacent object result and return, with no extra instruction or protection.
Class requests determine this emission plan before ordinary Explicit analysis;
a passing template retains its original method identity and raw snapshot.
Standalone requests do not use class-owned bridge generation. P7 unit admission
requires the explicit target's Java result and maps the Generated identity to
that owner's source span. A failed owner unit displays the retained
original bridge snapshot rather than a synthetic successful Java method.

A bounded parameter bridge additionally supports one evidenced
interface<String> specialization. P1 supplies original class Signature fragments;
P4 checks their equality to the caller facts, the interface's Object-bound class
parameter and its (TT;)TT; method contract. P7 checks the original String cast,
entry receiver/argument, exact String target and return. The emitted interface
specialization is required to make javac generate that same bridge. Missing or
contradicted specialization never permits deleting the bridge.

Raw generic method declarations require original/catalog Signature equality and
checked descriptor erasure. The supported profiles are parameterized reference
returns and one Object-bounded identity variable. Static identity calls use original
Object source formals and result views. Unsupported generic presentation stays
erased without requiring a Java overload-resolution proof. Class headers retain
original parent/interface descriptors and supported original Signature information. P7 retains original method
Signature alongside the erased declaration; it does not infer arbitrary generic
parent substitutions from the descriptor.

Generated is a checked core emission rule, not “delete whatever looks synthetic”. A failure of any
rule MUST NOT produce an exception stub for an arbitrary method; the corresponding required
full-unit case remains failing.

### Representation and output size

Java syntax is the readable notation, not a promise to emit classfile 52.
Diana MUST NOT reject readable source for JVM parameter, constant-pool, method-size,
annotation-count or MUTF8-item limits. It MUST NOT predict compiled sizes or build
a second JVM backend. DEX format checks, platform capacity, caller-configured
checked reads and cancellation remain applicable.

Operation/type/evaluation correspondence still applies to cmp-with-bias,
BoolToInt, ProvenNarrow and FillExpansion. A syntax form without a semantic
lowering rule does not become supported merely because source is not compiled.
Generated syntax still requires original member/template correspondence;
Diana MUST NOT hide a synthetic member on flags or presumed compiler behavior alone.
Independent compilation is a development observation, not production qualification.

## Single-use integer expression inlining

When inline_pure is false, return the validated JavaBody directly; when it is true, run one bounded
scan:

1. Match only the adjacent `t = expr; return t;` inside the same original block, where t is an
   ordinary OpResult local with exactly this one use.
   Do not match Phis, shadows, parameters, DeferredAllocation or any other control/protection
   domain.
2. expr contains only int/long literals, local reads, integer unary operations, explicit integer
   conversions, add/sub/mul, bitwise operations and shifts; it contains no div/rem, FP, heap, ref
   cast, call, allocation or synchronization.
3. Check the capacity of the body copy, the replaced nodes and the re-validation with checked
   arithmetic and fallible reservation; an allocation failure means Declined. Copy one JavaBody,
   scan once in the original AST order and replace disjoint pairs; do not run repeat,
   do not try modifying the original body in place, and build no plugin/option registry.
4. Move the expr tree unchanged into the return, preserving the source type and the Origin of both
   sites; a pure local declaration left without a use MAY be kept. Re-validate
   type/binding/control/effects and the complete emit.
5. If it passes, the result is Applied; an ordinary mismatch or an allocation failure is Declined;
   Cancelled stops; an internal invariant failure is a BUG and MUST NOT be wrapped as Declined.
   Acceptance compares the on/off results of the same fixed behavior, not the text or the local
   count.

The expr tree order is unchanged and nothing executes between the two statements, so the operand
reads the same lambda, the integer bits are unchanged and the original return value is unchanged.
FP is excluded to avoid store elimination involving different floating-point execution
environments; throwing/effectful expressions are excluded to avoid adding evaluation/protection
domain obligations. Inlining does not change the CFG/SSA/CopyPlan.

#### 3.6 Java results, Generated and printing

P7 produces Java member and class representations from the shared analysis
products. It retains original identities, material uncertainties and available
original facts. Where structure, bindings, initialization or declaration details
remain uncertain, P7 may still present a readable reconstruction and MUST mark
the uncertainty rather than claim it is proven. P7 MUST NOT delete a member based
on a missing synthetic marker alone or fabricate a replacement body. Generated
syntax retains the original member identities and MUST NOT duplicate members.

P8 is a pure printer and MUST NOT solve types or structure; its input is already sealed, but it MUST still check cancellation per segment. Java expressions use the
precedence table: primary/postfix→unary/cast→mul→add→shift→relational→equality→bitwise
and/xor/or→logical and/or→conditional→assignment. The printer MUST parenthesize an operand below the parent precedence; at equal precedence the
right operand keeps parentheses for sub/div/rem/shift and for non-associative operations, and the printer MUST NOT reassociate FP. A simple implementation MAY add more parentheses.
The printer MUST limit statement-expression to call/new/update/assignment and MUST emit a bare array/field/arithmetic read as a typed assignment.

An OriginSpan is a half-open interval in the final UTF8 text; the printer MUST record the byte cursor before each write and the origin after the node completes.
The printer MUST relocate class/unit prefixes uniformly when it assembles them; Generated maps to its owner syntax without losing the original key. A local name
only needs a per-request unique v-prefix, and the printer MUST NOT do name recovery or stable textual sorting.

## 8. Java representation constraints

Diana SHOULD preserve value, edge, copy, binding, exception and lock behavior in
readable output. When the available facts do not establish a behavior, Diana MUST
mark the uncertainty; it MUST NOT treat structural coverage as proof of semantics.
Compilation and behavioral comparisons are independent evidence for their tested
inputs and environments, not a prerequisite for displaying source. The engine
MUST preserve original UTF-16 and floating-point facts and MUST NOT silently
substitute fabricated method bodies or instruction widths. Real format limits,
checked reads and cancellation remain applicable.
