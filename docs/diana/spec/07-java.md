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

P7 produces an owned `JavaMethod` or `JavaInitializer`. Availability is separate
from `JavaVerification`: `NativeChecksComplete` records completion of applicable
native checks; `Incomplete` retains material unknowns or failed source checks.
Neither state establishes compilation or semantic equivalence. The engine MUST
retain a renderable reconstruction when source evidence is incomplete. A missing
representation still uses the classified Debug/Unavailable exit.

Each method runs exactly one analysis chain and selects a result by the following conditions:

1. Cancellation stops the request immediately and MUST NOT continue raw per method.
2. A proven Malformed yields Unavailable with any safe original debug excerpt; it never becomes a normal success. An abstract, native or interface member with no code yields Java with `body: declaration` when its declaration is readable; otherwise its available declaration facts remain visible as Debug or Unavailable.
3. P7 MUST retain a complete renderable reconstruction independently of native source-check completion. It MUST NOT fabricate a missing operation or suppress other completed members. Exact operation, value, copy and control correspondence remain checked against the shared products; an InternalInvariant remains a BUG.
4. When verification declines, P7 MAY retain an available readable reconstruction as Java with explicit uncertainty reasons and original identity. It MUST distinguish unknown facts from established facts and MUST NOT claim verified semantics or compilation. If no readable reconstruction is available, Unsupported enters the debug exit once. BudgetExceeded retains its classified failure and enters the debug exit only if resources remain. Read only facts whose original ranges/widths are checked, retain their coverage and mark unavailable regions. Any further read uses the same request resources and explicit fallback quota. A confirmed Malformed MUST NOT be revoked by re-reading.
5. When a decoded debug view cannot be produced safely within available resources, the result is Unavailable with any safe excerpt. When raw
   newly discovers definite damage, keep the Malformed fact; InternalInvariant keeps its BUG
   status even with raw attached. The implementation MUST NOT rerun normal or change the engine
   or limits.

When the caller explicitly sets normal work to 0 and raw is still available, the result MAY be
a decoded debug view; when the explicit fallback quota is also 0, the result is Unavailable.
None is not 0, and an unlimited default quota MUST NOT be treated as an exhausted budget.
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

P7 fixes a source-name plan before sealing Java; P8 only emits it. The plan emits
no imports or static imports. Primitive descriptors map to primitive keywords,
arrays to the component spelling plus `[]`, and known reference types to their
qualified source names. A named nested type uses its proven source owner chain;
an unrelated top-level name containing `$` keeps `$`.

For each type/static qualifier, P7 resolves the spelling in the actual package,
lexical owner, inherited member/type and expression scope under §9.4. A fully
qualified spelling is not automatically safe: its leading package identifier can
be shadowed in an expression. P7 MAY use an in-scope owner/simple spelling only
when resolution proves the same original type/member and dispatch. Otherwise it
rejects the affected Java rather than renaming the package/member or inserting a
helper. An unnamed-package type referenced from a named package is not guessed
accessible. Implicit `java.lang` lookup does not override an original type with
the same simple name. Synthetic locals avoid the identifiers the sealed plan
needs; P8 MUST NOT change their names in a way that invalidates that binding evidence.

### Static/final fields and the constructor chain

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
acquires an implicit default. P7 selects a source expression only for a matching
supported descriptor/value form; noncanonical NaN preparation remains Unsupported.
Finite FP and signed zero use exact hexadecimal or zero literals; infinity uses
constant division. String preparation checks modified UTF-8 target limits before
escaping original UTF-16 units.

P4 records own constructor/clinit final writes as initialization obligations,
not ordinary field-write permission. P4 WriteThis supplies constructor receiver
provenance. P7 consumes shared checked operations, storage and normal edges to
check the finite path-count set. The supported source graph is an unprotected
single-block Sequence or exclusive acyclic Diamond; affected explicit field reads,
cycles, switches and unsupported completion shapes reject this profile. P6 builds
these regions from the same P5 products. P7 replays final lexical control and
original field operations before publishing the method certificate.

A delegating constructor with no own final writes retains its checked original
target in the P7 certificate. Unit verification follows only certificates from
already-recovered original constructors, rejecting missing/non-constructor targets,
cycles and an own write combined with delegation. It does not analyze a callee
again. Missing assignments leave the unit Mixed without withdrawing independently
available methods.

P7 unit assembly emits preparation blocks in original field order after all
field declarations. A static final with checked original writes requires default
preparation and exactly one normal-path write; without such a write, unit assembly
emits the supported encoded or implicit default once. Non-default preparation
combined with a later final write rejects the unit. Static final assignments use
an already checked unqualified own-field form; fields colliding with synthetic
local spellings are rejected rather than rebound. P8 prints the sealed unit
preparation and original initializer bodies without decoding or choosing values.

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
consume ordinary member/unit verification. The instance profile accepts a public
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
issue while other members are retrieved; cancellation/resource/invariant errors
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
that owner's verified source span. A failed owner unit displays the retained
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
returns and one Object-bounded identity variable. Static identity calls use exact
Object source formals and result views; other applicable generic competitors reject
binding. Raw parent/interface profiles require a complete Object-bound class
Signature and variable-independent member contracts. P7 retains original method
Signature alongside the erased declaration; it does not infer arbitrary generic
parent substitutions from the descriptor.

Generated is a checked core emission rule, not “delete whatever looks synthetic”. A failure of any
rule MUST NOT produce an exception stub for an arbitrary method; the corresponding required
full-unit case remains failing.

### Java target checks

The Java target is Java8/classfile 52, and the development compile contract is
`javac --release 8 -proc:none -g:none` from Temurin21.0.8+9. Production does not invoke javac and
MUST NOT predict the compiler's code size.
P7 checks only the original semantics, the source rules, the declaration facts and the format items
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

### Source size is not an estimated rejection threshold

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

### Compiler-owned members and declaration completeness

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

#### 3.6 Java results, Generated and printing

P7 produces Java member and class representations from the shared analysis
products. It retains original identities, material uncertainties and available
original facts. Where structure, bindings, initialization or declaration details
remain uncertain, P7 may still present a readable reconstruction and MUST mark
the uncertainty rather than claim it is proven. P7 MUST NOT delete a member based
on a missing synthetic marker alone or fabricate a replacement body. Generated
syntax retains the original member identities and MUST NOT duplicate members.

P8 is a pure printer and MUST NOT solve types or structure; its input is already sealed, but it MUST still check budget/cancellation per segment. Java expressions use the
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
resource controls and cancellation remain applicable.
