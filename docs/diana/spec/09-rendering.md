# P9 rendering and source spans

[PIPELINE_DESIGN §8.2](../PIPELINE_DESIGN.md#82-p7p8p9-的显示产品交付)
defines the display boundary. Code lives in `crates/diana-decomp/src/p9/`;
execution observations belong in the [engine README](../../../crates/diana-decomp/README.md).

## Input, authority and lifetime

Engine calls P9 with its private `RenderTask` after the same request's P8
assembly. P9 consumes `ClassResult`, standalone `MethodDisplay`, or a classified
`AssemblyStop`, together with their owned original facts and expression products.
P8 shares these immutable results through Arc. Neither P9's input nor its output
borrows DEX, Engine or method-analysis scratch.

P9 MUST consume P7's final `EmissionPlan` and P8's chosen type spellings. It MUST
NOT decode instructions, reconstruct a graph, choose a handler, solve types,
resolve overloads, plan imports or retry an earlier stage. Indexing existing
`TypeSource` mappings does not choose another spelling. Explicit graph syntax
uses the same statements and copies as structured syntax.

`Rendered` owns UTF-8 text, source spans, concrete rendering issues and the same
immutable source product. `write_text` writes that text; `write_json` serializes
that publication, including identical text and source facts. JSON does not run
another renderer or select another recovery mode. The retained source section
also contains inspection-formatted diagnostics; P9 does not parse Debug strings
as an execution representation. The notation identifier is
`diana-java-like-v1`, not a compilability or verification badge.

## Expression and control notation

The output is Java-like code with explicit forms for existing DEX operations and
phase boundaries. These forms do not introduce runtime helpers or a dispatcher.
Ordinary values use P7's locals, literal views and materializations. P9 renders
nested composed expressions with a fallibly allocated work stack, preserving each
producer's statement, operation, site, result-commit and control spans. A delivered
`ComposedResult::Store` prints `(local = expression)`: the original local is
assigned only after the expression completes normally, before its value is used
by the surrounding predicate. P9 does not infer store retention from read counts.
A delivered `ComposedPredicate::LongComparison` prints the original left and
right operands with P7's selected relation. Predicate negation complements that
relation; P9 does not infer a comparison from numeric syntax or fold floating
NaN behavior. Both the comparison expression and its consumed read retain spans,
as do its original statement, operation and source control.
Calls retain original target IDs, dispatch and separate invocation prototypes.
P9 prints P7's selected signature directly after the method name, using P8's
existing type spellings. The qualification is part of the Java-like call notation,
not a conversion, generic invocation or comment.

The following forms have fixed execution meanings:

| Form | Meaning |
|---|---|
| `receiver.field = value` / `array[index] = value` | Ordinary assignment. It is used whenever the array, index or receiver slot is a plain local read or literal, because reading them before the value cannot be observed and the null/bounds check still follows the value. |
| `store value -> receiver.field` or `store value -> array[index]` | Fallback when a non-value slot is an inlined expression or an unavailable operand, so Java's left-to-right order would not match the delivered store. |
| `[after_operands_before_effect { copies }]` | Capture the operation's original operands, run its supplied snapshot steps, then execute the original effect. A result assignment commits only on normal completion. |
| `new T(arguments)` / `target = new T(arguments)` | Execute the selected P5 construction. A single publication that is the construction's own deferred carrier prints as an ordinary assignment, because Java assigns only after `new` completes. |
| `T local` | The original `new-instance` published to that local. The object exists at this position and its constructor has not run, so the constructor call keeps its own statement and position. This declaration stands where the allocation runs, and the method header omits that local. |
| `new T(arguments) [on_success publish (locals)]` | Fallback when more than one alias publishes, or the publication is not that carrier. Aliases become visible only after successful initialization. |
| `test { ... yield condition [repeat {...}] [exit {...}]; }` | A loop condition whose predicate node also carries statements or phases keeps this block form. Copies belong to the original repeating or exiting edge; an empty edge prints no clause. Outer negation changes repetition polarity, not which original edge owns copies. |
| `while (condition [repeat {...}])` | When the predicate node holds nothing but the condition, P7 selects this compact form and the copies stay attached to their own original edge. |
| `synchronized R(lock) with_explicit_exits` | The header owns the original acquire. This lexical container does not implicitly release at its closing brace or on an abrupt exit. |
| `release synchronized R(lock)` | Execute that outlet's original release read/effect at its original node. Only the named lock scope ends; enclosing try scopes remain unchanged. |
| `on_exception_entry { copies }` | Receive saved values only on exceptional arrival, after the original capture when present. |
| `try { ... } catch (Type)` / `catch (all)` | A catch header prints the caught type and nothing else: the handler block follows it, so the clause's route target is not repeated. `all` is a clause with no delivered type (a catch-all), which the DEX format and `exception_chain` routes spell the same way. |
| `exception_chain` and `[exn cN]` site markers | Preserve original selection priority, handler targets and delivered natural/explicit protection. A marker is printed only when the enclosing printed catch does not already express that route; it names the chain whose line carries the clauses. A chain line is printed only when a printed `[exn cN]` marker names it, because each catch route is otherwise already printed as the clause of its `try`. Method-local impossible-release findings do not delete original exception facts. |
| `graph ... entry label` / `no_implicit_entry` | Preserve the supplied graph entry or its explicit absence. The first printed child is not an invented entry. |

A `break` or `continue` whose target is the innermost enclosing scope prints
without a label, and the label is then omitted. A `continue` that ends its loop
body prints nothing, because reaching the body end already starts the predicate;
its copies still print in order.

A void `return` that is the last printed action of a method body prints nothing:
the closing brace performs the same exit and nothing is attached to that return.
A return with copies, phases, a site or an unavailable definition keeps its text,
and so does any return that other statements follow.

A method body that emits no text prints `{ }` on its declaration line. A body
with any printed content keeps its own lines.

A labeled or graph scope prints its `R<id>:` control name only when a printed
transfer still names that control; the control, its braces, its graph header and
its children remain in the product. A named scope whose only child is another
unnamed printed control prints that child directly; P7 marks it transparent and
P9 omits the scope's own brace pair.

A natural call prints `receiver.method(arguments)`. A selected `Parameters` call
prints `receiver.method<(ParameterTypes)>(arguments)`. A selected `FullSignature`
call prints `receiver.method<(ParameterTypes) -> ReturnType>(arguments)`.
The parameter parentheses distinguish a signature from generic type arguments;
an empty parameter list prints `<()>` or `<() -> ReturnType>`.

When P7 selects owner disclosure, an instance receiver prints as
`(receiver via Owner)`. For example,
`(v1 via StringBuilder).append<(Object)>(value)` names the original reference's
owner and parameter list. `via` introduces no conversion, check, extra evaluation
or choice of runtime implementation. The original dispatch remains unchanged.
A qualified super call retains `super`, including `(super via Owner)` when P7
selected owner disclosure. Static calls already name the owner before the member.
The owned reference still carries the original target identity, dispatch kind
and complete prototype, and the printed member keeps its source span.

A jump whose target prints next is sequencing, not an emitted `goto`, including planned
continuations and the last printed If branch. P7 selects this omission and marks
an emptied, label-free, copy-free `else` as omitted; P9 MUST print neither its
`else` header nor its control. The omitted false edge retains a zero-width
Transfer anchor at the predicate. For an explicit If terminal, P9 consumes P7's
separate empty-false-arm flag and omits that arm's text, retaining the original
false transfer and any nonprinting copy bundles as zero-width anchors. P7 also selects
an exactly negated single-arm If when only the original false arm prints, an
empty-If display when neither arm nor the predicate has work, and `else if` for a
single nested printable If. P9 consumes those shapes without recomputing
adjacency or emptiness. Inversion uses the same predicate-negation renderer as
loops; the original false edge retains its copies at the displayed body's start.
An empty If retains its header and original terminal facts, with a zero-length
terminal anchor for a suppressed explicit test. A chained If owns its own braces
and arm emission.

An ordered condition prints one `if (test0 || test1 ...)`. Each test keeps its
own terminal span around its predicate text and both original test-edge transfer
spans at the same evaluation position except for the final other edge when an
else is present. `ConditionElse::Copies` prints an else containing only that
edge's original bundles, in their delivered order. Its Copy and Transfer spans
occur there once; P9 does not emit those copies while printing the tests or again
after the condition. `ConditionElse::Body` prints the edge's copies before the
delivered body. The shared predicate renderer establishes
and restores that terminal's check carrier for ordinary If, Loop and ordered
Condition tests, including on rendering failure. The final `else` route keeps the
selected `other` link. Later test runs print no text, so P7 delivers those run controls
and P9 anchors their controls, block labels, transfers and empty copy bundles as
zero-length spans, without recovering control from the graph. A terminal whose
route prints no words keeps both its Transfer and Terminal anchors, including
zero-step copy bundles, and leaves no indentation in the text.

P7 fixes condition/header ownership, loop repetition, child roles and necessary
continuations. P9 prints those choices. A delivered control completion prints
its selected goto/break/continue after that control's normal body; an original
abrupt terminal leaves before it. A private switch default can have no body
operations while retaining its selected-edge Copy/Transfer anchors followed by
the delivered break. Successful case returns retain their own terminal/operand
origins, and the shared continuation prints once outside the switch. P9 does not
infer scope completion or shared targets from adjacency. An explicit Condition uses the selected
P6-derived order and child completions in the emission controls, not the retained
source control's tests/shared/other order or source continuation evidence. In particular, predicate-exit copies do
not appear in a loop's common continuation, where an independent break could
execute them. A predicate with no separate work or edge copies can use the
ordinary loop-condition spelling selected by P7. A compact evaluation
Sequence retains its supplied block, link and control anchors; a composed producer's
control range closes at its condition use, with the original operation, statement,
result PC and exception site origins. P9 traverses only that delivered layout. An
exactly negated composed boolean Eqz prints its operand without double negation;
original arm identities and selected-edge phases do not change. Synchronized routing terminals
and post-release copies remain at their original nodes rather than moving past
surrounding catch scopes. Uncaught propagation is not an unavailable handler.

Array allocation prints as `new Element[length]`, followed by one empty `[]`
per remaining original dimension; the delivered element spelling and rank are
unchanged. An element that is itself a combined expression keeps the explicit
form.

Unary conversions correspond to original conversion operations. A check-cast
remains a check with no invented register write. It prints as `cast v to T` at its
original position, or as `((T) v)` at the first printed read of the checked local;
the check then keeps its own statement, operation, type and route origins at the
use. That read is either the first evaluated value of the following statement or
the first executed step of the single unconditional transfer copy set that ends the
same block. P7 records the exact statement and read-expression occurrence, and only
that occurrence prints the check: another read of the same local, including one
later in the same statement, keeps its own plain text. A composed first value that
would print under another statement's carrier, a later copy step, a selected
branch, a terminal, a copy phase or a printed route keeps the cast a statement, and
that statement carries one short trailing comment naming the original `check-cast`
and the fact that the value is not reassigned. Composition moves the check's text
only: the original operation, statement, exception site and every copy step still
execute once in their original order, and no assignment, local or value is created.

A check P4 published as `Redundant` prints no text at all. Its plan action stays at
its original position and carries zero-length `Statement`, `Operation`,
`Expression`, `Reference`, `Type` and `Site` anchors there, so the printed body has
no redundant cast while the original event, its resolved type name and its routes
keep a source position. P9 reads that plan decision; it does not prove or search for
redundancy, and an ordinary check that P4 retained prints exactly as before.

The same short comment marks the other statements printed in Diana syntax rather
than Java: `capture_exception` (move-exception, the caught throwable) and an
allocated object whose constructor has not run. That allocation prints as the
allocated type and its local, and its constructor call prints as
`receiver.<init>(arguments)`; the comment says the object exists before its
constructor. Bracket annotations and block headers that are already visibly not
Java keep no comment. Array fill preserves its one
original operation, element width/count and bits. Explicit allocation and
initialization stay separate when P5 did not select a compound construction.
Unavailable reads and argument layouts print their supplied gaps and original
words, not zero, null or an empty argument list.

## Declarations, metadata and literal identity

Static preparation prints one `name = value` line per original static ordinal
before the class initializer, when the class result reports that the list states an
encoded value. Each line uses the field declaration name and the original index
when that declaration is unavailable. An entry the encoded array does not cover
prints `implicit_default`; a decoded entry prints its expression, with an
explicit prefix marker when the traversal stopped early. These are encoded values,
not executed assignments, so they are not folded into the field declaration.

P9 prints the supplied member list, including no-code, synthetic/bridge and
stopped members. It does not infer nesting from `$` or create declarations for an
unlocated directory suffix. Static preparation follows P8's original ordinal
plan before the original class initializer. Implicit defaults are not encoded
assignments or guessed zero values.

A metadata record prints at its own declaration, in delivered order and above the
declaration it belongs to: a class record above the class header, a field record
above that field, a method record above that method's header, and a parameter
record inline before that parameter's type. A record whose owner declaration is
not printed keeps the `metadata { ... }` block with its original owner prefix
(`class`, `field <name>`, `method <name>`, `method <name> parameter <n>`) and the
original index. The record's visibility prints as `build`, `runtime` or `system`,
and a prefix traversal keeps an explicit marker. Flat array and nested-annotation events print as `{ ... }` and
`@Type(...)` from their delivered child counts and closing events. This also
preserves records whose member directory entry was not located, and no record is
printed twice or dropped when it moves to its declaration. Array/annotation
values use P8's flat events; call-site bootstrap values use P7's owned events and
leaf declarations. Counts, reference identities and local gaps remain visible.
Neither path parses encoded input again; generic display syntax comes only
from P8's interpreted signature plan.

A `@Signature` record whose declaration P8 already interpreted prints that
interpreted signature instead of its raw descriptor fragments: one type for a
field or parameter, optional type parameters followed by `(parameters) -> return`
for a method, and optional type parameters followed by `extends`/`implements`
clauses for a class. A record P8 did not
interpret keeps its original value tokens, so an uninterpreted signature never
silently changes shape. The delivered annotation text remains in the product.

A constructor prints as `OwnerName(parameters)` with no return type, using the
class declaration's name rather than a reference spelling that an unrelated field
or local could qualify. A class initializer prints as `static { ... }`. An
ordinary `new T(...)` omits the initializer suffix when the delivered constructor
is that same type's `<init>`; a different delivered initializer keeps it. The
original `<init>`/`<clinit>` identities remain in the owned method declaration.

A constructor delegation whose receiver is still before its constructor prints as
`this(...)` or `super(...)` exactly when it is the first statement the body prints,
the target has P7's natural qualification and the earlier operations printed
nothing (argument definitions that become their uses' values do not count). The
original code offset is not the test. Otherwise the delegation prints its original
relation, `super(...)` or `this(...)`, or `receiver.<init>(...)` when the receiver
is an allocated object rather than the class under construction. An ambiguous
target prints its selected signature immediately after `this` or `super`.
An explicit initialization of an allocated object keeps `receiver.<init>` and
prints its selected signature after `<init>`, with `via` on the receiver when
selected.

A constructor reference prints the type it constructs, never a wrapper around the
`<init>` name: `new ArrayList()` names the object and its constructor, and a
qualified constructor reference carries the same spelling followed by its
original prototype. An
annotation record for a constructor names that type too. The delivered `<init>`
identity remains in the owned reference and declaration facts.

An original name segment that is not a Java identifier prints inside backticks,
exactly as delivered: `` class `0000` ``, `X.`00BN``, `Result.`constructor-impl``.
Only that segment is quoted; valid package and name segments keep their normal
spelling, and `$` stays inside its own simple-name segment. Backticks and
backslashes are escaped, and controls, bidirectional controls and isolated
surrogates keep `\uXXXX`. Nothing is renamed, no import is added because of the
quoting, and the delivered units, descriptors and identities stay unchanged.

A declared field type, method parameter, return type and superclass/interface
print their delivered generic arguments when the plan carries them, for example
`Map<String, Face[]>`; otherwise the erased type prints. Class and method headers
print the delivered type parameters in declaration order. A single non-Object
bound prints as `T extends Bound`; an unbounded or plain Object-bound parameter
prints only its name. Variable uses print that declared name, including the same
backtick segment notation for non-identifiers. Variables do not create imports;
ordinary named types in arguments and bounds retain the existing naming rules.

P9 prints the supplied class, field and method access flags as Java modifier
words where the kind has one. A DEX-only bit that the declaration form does not
already show keeps an explicit `dex_flags<...>` marker; the class kind word and an
`<init>`/`<clinit>` name already convey the interface/annotation/enum and
constructor bits. Bits with no defined meaning for that kind remain visible as
`unknown_flags<0x...>` and are never folded into the whole original word. The raw
word itself remains in the source product. A field's `enum` marker identifies an
enumerated value, not merely a field whose enclosing type is an enum; ordinary
field syntax does not convey that fact. `synthetic`, `bridge` and `varargs` retain
their marker spellings, not Java modifier words.

For methods, either `ACC_SYNCHRONIZED` (0x20) or `ACC_DECLARED_SYNCHRONIZED`
(0x20000) prints one `synchronized` declaration word, including when both are set.
The [DEX access-flags table](https://source.android.com/docs/core/runtime/dex-format#access-flags)
defines 0x20000 as declared `synchronized`, with no execution effect beyond
reflection; 0x20 requests automatic locking and is valid only with `ACC_NATIVE`.
The pinned ART [modifier definitions](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/libdexfile/dex/modifiers.h)
keep these bits distinct. Here the word displays the declaration, not an extra
implicit acquire/release: P9 MUST NOT add, remove or fold delivered body monitors
because of it. The original flag word still distinguishes VM locking from the
declared modifier.

An interface, annotation or enum prints no superclass clause: Java has none there
and the delivered Object/Enum entry is implicit. An ordinary class whose delivered
superclass is exactly `java.lang.Object` prints no clause either, because Java
source never states that relationship; the delivered type use and its planned
import stay unchanged. A missing, unread or unavailable superclass fact still
prints, so the gap stays visible, and every other superclass keeps its clause. P8
selects that clause and P9 prints the selected form.

A class initializer prints as `static { ... }` without access modifiers, because
Java static initializers cannot carry them; those bits remain in the method's
original flags. An initializer whose printed body states nothing prints no block
at all. P8 omits the block when the initializer prints no local declaration, no
chain clause, no call-site definition, no copy, no diagnostic, no action and no
block end that still states text, and it keeps the block when a record of that
member or that member's metadata gap would print at the same position. The
member, its header, its original identities and its code remain in the class
product and coverage; only its text is absent.

P9 MUST preserve P8's base spelling and array rank. Original-name forms and raw
descriptors remain explicit. Literal strings do not pass through name planning.
UTF-16 escaping preserves NUL, quotes, backslashes, controls and isolated
surrogates; valid pairs become UTF-8 scalars. Bidi controls are escaped to avoid
visual source injection. Floating-point non-finite values retain exact bit
notation rather than losing a NaN payload.

## Spans

`TextOffset` is a byte coordinate in the final UTF-8 text. `SourceSpan` ranges are
half-open and refer to original members or typed P7/P8 identities. Operations
include original PCs; result capture, terminals, expressions, references,
transfers, copies, labels, controls, metadata and preparation have distinct
origins. A compound expression can map multiple original operations to one range.
Nested spans overlap intentionally. Empty schedules and implicit fallthrough can
have zero-length anchors; these are not invented executed instructions.

Suppressed literal statements and contracted interior transfers remain in the
owned source products and coverage. A composed operation's range encloses its
printed expression; its original statement and optional exception site retain
separate origins. The original source-control range closes at the composed use
for a cross-node composition. P9 does not manufacture textual operations
merely to give every original instruction a nonempty range. Offsets are measured
while writing; they are never reused from an earlier qualified-name rendering.

## Failure locality and output errors

Existing input, type, storage and control gaps remain content of the result, not
reasons to reject independent members. Stopped members retain their classified
cause, operation-associated closed P7 fragments, original decoded items and
checked retained bytes when available. Other unpublished arena entries remain
construction evidence. Storage/copy facts of a prefix are labeled as facts, not
promoted to an executable method schedule. P9 MUST NOT recover missing control
from those facts or reopen a DEX range.

A method's display-reference invariant or private traversal/index scratch failure
produces a method-local rendering issue; other members continue. Failure to retain
that diagnostic preserves its cause in `DiagnosticRetention`. Shared output
buffers, class-wide display context and output-layout failures stop that rendering
publication with `RenderStop`, retaining its source product, completed text prefix,
spans and existing issues. A prefix is not relabeled `Rendered`.

The writer methods propagate concrete text/JSON sink errors. They MUST NOT report
a truncated successful write, retry analysis or substitute an exception stub.
The caller owns the sink and backpressure. Output growth and traversal work stacks
use checked arithmetic and fallible reservations; region traversal is iterative.

## Evidence boundary

The previous core test was removed by operator decision. Text/JSON use the same
owned publication, but construction completion does not establish semantic
equivalence. Corpus observations do not prove arbitrary VM, heap/callee,
bootstrap, protection or allocation-failure behavior.
