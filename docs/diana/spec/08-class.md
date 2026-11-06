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
original class-initializer body. Implicit defaults remain distinct from explicitly
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
A field, method parameter list and return type use the declaration's own
`@Signature` when that text parses and its erased form equals the delivered
descriptor. The reader accepts primitives, classes, arrays, wildcards and nested
arguments; a type variable, an inner class with outer arguments, a malformed
encoding, an unsupported nesting depth or a descriptor mismatch keeps the erased
type. A signature that produced a display type no longer blocks shortening; one
that did not still does. Class-level type parameters and superclass/interface
arguments are not interpreted yet.

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
value interpretation. Invalid ordinary identifier spelling retains original-name
notation and original UTF-16 units.

P8 MUST NOT rewrite an already rendered string. Literal strings, raw descriptors,
member names, call signatures and P7 execution products remain unchanged. The
plan does not certify Java name binding, access legality or compilability.
P9 consumes the chosen spelling; it does not decide imports or shorten names.

## Evidence boundary

The previous core test was removed by operator decision. Normal Engine requests
have no independent verifier or verification switch. Corpus observations and
successful assembly do not prove arbitrary metadata, scope, resource or VM
execution behavior.
