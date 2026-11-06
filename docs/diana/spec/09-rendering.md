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
Ordinary values use P7's locals, literal views and materializations. Calls retain
original target IDs, dispatch and separate invocation prototypes. A selected
full signature is printed in the call's binding suffix; this is part of the
Java-like call notation, not a conversion, generic invocation or mere comment.

The following forms have fixed execution meanings:

| Form | Meaning |
|---|---|
| `store value -> receiver.field` or `store value -> array[index]` | Capture the value before receiver/array/index, then perform the original store. This avoids silently adopting Java left-hand-side operand order. |
| `[after_operands_before_effect { copies }]` | Capture the operation's original operands, run its supplied snapshot steps, then execute the original effect. A result assignment commits only on normal completion. |
| `new T(arguments) [on_success publish (locals)]` | Execute the selected P5 construction and publish its aliases only after successful initialization. Its common snapshot prelude precedes allocation. |
| `resume construction g at label` | Continue the selected compound expression at the original initializer's node. Absorbed members are not executed again; PC order is not an implicit continuation. |
| `test { ... yield condition [repeat {...}] [exit {...}]; }` | A loop condition whose predicate node also carries statements or phases keeps this block form. Copies belong to the original repeating or exiting edge; an empty edge prints no clause. Outer negation changes repetition polarity, not which original edge owns copies. |
| `while (condition [repeat {...}])` | When the predicate node holds nothing but the condition, P7 selects this compact form and the copies stay attached to their own original edge. |
| `synchronized R(lock) with_explicit_exits` | The header owns the original acquire. This lexical container does not implicitly release at its closing brace or on an abrupt exit. |
| `release synchronized R(lock)` | Execute that outlet's original release read/effect at its original node. Only the named lock scope ends; enclosing try scopes remain unchanged. |
| `on_exception_entry { copies }` | Receive saved values only on exceptional arrival, after the original capture when present. |
| `exception_chain` and `[exn cN]` site markers | Preserve original selection priority, handler targets and delivered natural/explicit protection. A marker is printed only when the enclosing printed catch does not already express that route; it names the chain whose line carries the clauses. Method-local impossible-release findings do not delete original exception facts. |
| `graph ... entry label` / `no_implicit_entry` | Preserve the supplied graph entry or its explicit absence. The first printed child is not an invented entry. |

A `break` or `continue` whose target is the innermost enclosing scope prints
without a label, and the label is then omitted. A `continue` that ends its loop
body prints nothing, because reaching the body end already starts the predicate;
its copies still print in order.

A selected `FullSignature` call prints its original target as a trailing
`[binding ...]` suffix; a natural call prints only the receiver, member and
arguments. The owned reference still carries the original target identity,
dispatch kind and prototype, and the printed member keeps its source span. A jump
to the statement printed immediately after it is sequencing, not an emitted
`goto`.

P7 fixes condition/header ownership, loop repetition, child roles and necessary
continuations. P9 prints those choices. In particular, predicate-exit copies do
not appear in a loop's common continuation, where an independent break could
execute them. A predicate with no separate work or edge copies can use the
ordinary loop-condition spelling selected by P7. Synchronized routing terminals
and post-release copies remain at their original nodes rather than moving past
surrounding catch scopes. Uncaught propagation is not an unavailable handler.

Unary conversions correspond to original conversion operations. A check-cast
remains a check with no invented register write. Array fill preserves its one
original operation, element width/count and bits. Explicit allocation and
initialization stay separate when P5 did not select a compound construction.
Unavailable reads and argument layouts print their supplied gaps and original
words, not zero, null or an empty argument list.

## Declarations, metadata and literal identity

P9 prints the supplied member list, including no-code, synthetic/bridge and
stopped members. It does not infer nesting from `$` or create declarations for an
unlocated directory suffix. Static preparation follows P8's original ordinal
plan before the original class initializer. Implicit defaults are not encoded
assignments or guessed zero values.

Metadata records print once with their explicit original owner scopes. This also
preserves records whose member directory entry was not located. Array/annotation
values use P8's flat events; call-site bootstrap values use P7's owned events and
leaf declarations. Counts, reference identities and local gaps remain visible.
Neither path parses encoded input again or interprets Signature as recovered
generic syntax.

P9 prints the supplied class, field and method access flags as Java modifier
words where the kind has one. A DEX-only bit that the declaration form does not
already show keeps an explicit `dex_flags<...>` marker; the class kind word and an
`<init>`/`<clinit>` name already convey the interface/annotation/enum and
constructor bits. Bits with no defined meaning for that kind remain visible as
`unknown_flags<0x...>` and are never folded into the whole original word. The raw
word itself remains in the source product. A `declared_synchronized` marker is a
declaration fact: the VM does not lock such a method automatically, so P9 does not
invent a lock and prints only the monitor operations delivered for its body.

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
