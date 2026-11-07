# P8 class assembly and type-name plans

[PIPELINE_DESIGN §8](../PIPELINE_DESIGN.md#8-p8-类级准备与组装p9-渲染)
defines the product contract. The implementation is
`crates/diana-decomp/src/p8/`; execution observations belong in the
[engine README](../../../crates/diana-decomp/README.md).

## Input, scheduling and ownership

Engine obtains P0 facts and runs the existing method tasks once. After joining
those tasks, it calls P8 with the original class directory and one delivery per
selected method. P8 MUST NOT invoke a method-stage constructor, decode input,
retry a failed method or derive missing execution facts.

A complete class request produces `ClassOutcome::Ready(Arc<ClassResult>)` or a
classified `AssemblyStop`. A selected-method request uses the same collector and
name planner with standalone scope, producing `MethodDisplayOutcome`. Standalone
plans have no package or imports and keep qualified names. They do not analyze
other methods. Earlier-stage inspection remains attached to the same Engine
response, not to a second execution or an owned predecessor getter.

`ClassResult` owns its header, field expressions, static-preparation order,
methods, metadata expressions and `TypeNamePlan`. It shares immutable owned P0
facts and P7 results through Arc; it does not borrow DEX bytes or an Engine.
Sharing a result does not copy its expression arena or expose mutable analysis.
P7's `into_result()` returns the owned shared result and releases retained
analysis. A class result can outlive both the input bytes and Engine. P9 and same-execution
inspection share immutable owned P8 results, including assembly stops, rather than
copying expression arenas. P9's renderer consumes the established products as
described in [the rendering chapter](09-rendering.md).

The class header states the printed form of the original header: kind, flags, name
and the superclass clause. `SuperclassClause::Implicit` states that Java has no
clause there, which covers an interface, annotation or enum, an absent original
entry, and an ordinary class whose delivered superclass is exactly
`java.lang.Object`. `SuperclassClause::Printed` keeps the delivered spelling,
including a missing, unread or unavailable fact, so a gap stays visible. The
original superclass type use remains in the plan. An implicit superclass does
not by itself require an import.

`ClassResult` also records the class initializers whose printed body states
nothing, because such a block prints no text. The delivered member stays in the
result; P9 omits only the `static { ... }` block of a recorded initializer.

## Directory and failure locality

Assembly associates methods by original MethodId under the original ClassKey.
Foreign owners, duplicate deliveries and a known directory member without a
result are construction invariants. Scratch indexes do not reorder output members.
Every located field and method remains present, including native/abstract,
synthetic/bridge, constructors, initializers and stopped members. P8 does not
invent generated-member or nested-ownership evidence.

The original declared counts, delivered counts, termination position and concrete
P0 errors remain in the result. An incomplete directory does not become an empty
class or proof that later members are absent. A broken record with no established
next boundary stops the shared directory walk; already delivered members remain
usable. The walker does not scan for another record. Field and method traversal
coverage are distinguished from independent preparation and metadata errors.

Class-type and field declarations preserve Present, Absent, NotRead and
Unavailable states. Unreadable interfaces retain their list or entry state and
original identity where established. A missing type spelling does not remove
its field or other members.

`MethodDelivery` distinguishes expressed methods, no-code declarations and
classified stops. The handoff retains the original failure kind independently of
fallible diagnostic text, checked raw-code copying and decoded-item copying.
Those copy results are independent: one retention failure MUST NOT erase another
available result or a shared P7 expression prefix. P1 supplies its original
source view even when only a decoded prefix was established. The input reader
copies checked code ranges; P8 does not decode them. An unpublished P7 prefix
remains `ExpressionPartial`, never a successful MethodResult.

`AssemblyStop` retains its P0 input, all delivered methods and explicitly partial
name/value views. It does not publish those views as a complete class. Resource
failure remains classified; neither a member error nor its failure kind cancels
siblings. Actual inability to allocate output does not promise recovery by
further allocation or re-analysis.

## Fields, preparation and annotation expressions

Fields retain their original storage group, ordinal, flags, name and declared
type identity. P0 supplies scalar and flat encoded-value facts; P8 selects the
corresponding literal, reference, array, annotation and container-end expressions.
Reference expressions retain checked leaf declarations and their type-use IDs.
Their values, names and ordered container events are not recovered from bytes.

The static-preparation list follows original static ordinals and precedes every
original class-initializer body. P8 also records whether that list states any
encoded value: a list made only of implicit defaults describes no value, because
the class initializer assigns those fields. The per-field `Preparation` states
remain in the product either way. Implicit defaults remain distinct from explicitly
encoded defaults. P8 MUST NOT add an assignment for an implicit default, move a
constructor or `<clinit>` write into a field declaration, or suppress a later
write because preparation was already displayed.

P0 retains the decoded prefix of a damaged composite preparation. The checked root
count independently establishes omitted static entries, even when an earlier
encoded suffix cannot be located. Such omitted entries remain implicit defaults;
intervening unlocated encoded entries remain NotRead, not fabricated zero/null.

Metadata references use a flat owned declaration pool shared by the original
annotation facts. The pool retains original reference IDs, descriptors, field or
method declarations, prototypes and handle members. P7's method-scoped metadata
view exposes only references used by that method's records. Nested annotation
descriptors and field/interface TypeIds are delivered by their original readers.

P8 preserves annotation visibility, owner location, physical range, token order
and prefix extent. Record identity is separate from byte range: two members may
share the same annotation bytes but require different naming scopes. System
metadata remains original metadata; uninterpreted Signature data conservatively
prevents unproved type-name shortening rather than silently disappearing.

## TypeNamePlan

A named use records its original source coordinate, optional original TypeId,
lexical scope, target, spelling choice and concrete reason when qualified or
original notation is required. Sources include class/field/method declarations,
P7 type-use IDs, annotation record/token/part and preparation field/token/part.
Equal descriptors may share display storage, but original uses and IDs remain
distinct. Array rank is explicit, and matching element names share one import.

The package comes only from the selected class descriptor. Its original spelling
form is explicit. A declaration name is separate from a reference's qualified
spelling; a field shadow MUST NOT turn the class declaration into a qualified
identifier. `$` remains an ordinary binary-name unit without established nesting
evidence. P8 never splits it into an inferred enclosing type.

The planner groups delivered simple names to detect conflicts and examines
class-field names, method-local names and delivered field-reference names for
shadowing. The same target can be simple in one method and qualified in another.
For complete P7 methods, P8 also consumes the owned expressions, statements,
local type uses and references to identify a natural `this` or `super` constructor
delegation whose owner is not printed. An unavailable or incomplete method keeps
its potential imports. Other uses of the same type continue to require their
own planned imports. A nonprinting use keeps its original identity and a qualified
fallback spelling; it does not itself add an import.
A field, method parameter list and return type use the declaration's own
`@Signature` when that text parses and its erased form equals the delivered
descriptor. A class signature establishes its parameter scope only when its
superclass and every interface erase to the delivered descriptors in original
order. P8 delivers parameter names, optional bound roots and superclass/interface
roots in the signature plan. An omitted or plain Object bound has no display root;
the original annotation retains that distinction.

For an interpreted method signature, P8 identifies the original record used by
the interpretation. It marks that record fully represented only when its system
annotation is a complete `value` array of strings and its signature contains no
throws entries or explicit Object bounds, which the method declaration does not
currently display. Other records, visibilities and incomplete or additional
information remain visible.

The reader accepts primitives, classes, arrays, wildcards, nested arguments and
variables declared in the explicit class/method scope. A variable erases to its
single established bound, or Object when unbounded. Method parameters shadow
class parameters; a bound sees only earlier parameters and cannot refer to itself
or a later parameter. Unknown variables, recursive or forward bounds, multiple
intersection bounds, inner classes with outer arguments, malformed encodings,
nesting beyond the reader's `MAX_DEPTH` or descriptor mismatches keep the erased
declaration. A rejected class signature leaves class parameters unknown without
blocking independently interpretable member signatures.

A signature that produced a display type no longer blocks shortening; one that
did not still does. Variable nodes print their declared names without becoming
import candidates. Parameter names also participate in lexical shadowing for
ordinary type uses.

Incomplete lexical evidence or uninterpreted signature scope keeps qualified
spelling. A reference into `java.lang` is not assumed to be unaffected by an
unestablished same-package declaration: the planner adds an explicit single-type
import for it, because Java resolves a single-type import before the package and
before `java.lang.*`. The usage then shortens without asserting anything the
input did not establish, and no external classpath query is involved.

Unambiguous same-package uses shorten without import; other established simple
uses receive explicit single-type imports. Imports are deduplicated by full binary
spelling, not by original reference identity. Conflicting short names retain
qualified spelling. No wildcard/static import, member renaming, cast or typed alias
is introduced. Primitive spelling and array rank are presentation facts, not new
value interpretation. A segment whose original text is not a Java identifier prints
backtick-quoted in place, per path segment (`X.`00BN``, `Result.`constructor-impl``,
```0000``), keeping original UTF-16 units, `$` and literal dots inside their own
segment. A type with no delivered display name keeps
`type<"..."; display unavailable>`. P9 escapes backticks, backslashes and controls
inside the quoted segment; it does not choose a different name.

P8 MUST NOT rewrite an already rendered string. Literal strings, raw descriptors,
member names, call signatures and P7 execution products remain unchanged. The
plan does not certify Java name binding, access legality or compilability.
P9 consumes the chosen spelling; it does not decide imports or shorten names.

## Evidence boundary

The previous core test was removed by operator decision. Normal Engine requests
have no independent verifier or verification switch. Corpus observations and
successful assembly do not prove arbitrary metadata, scope, resource or VM
execution behavior.
