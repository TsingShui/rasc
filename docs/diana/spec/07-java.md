# P7 owned method expressions

[PIPELINE_DESIGN §5.7](../PIPELINE_DESIGN.md#57-p7methodresult) defines the
method-expression contract. The implementation is `crates/diana-decomp/src/p7/`.
Capability and execution observations belong in the
[engine README](../../../crates/diana-decomp/README.md).

## Boundary and ownership

Engine's sole method task calls `build_method(task, planned)` after P6.
P7 consumes mature execution, type, storage and control plans. It MUST NOT decode
instructions, resolve missing declarations, solve types, identify regions or
schedule copies again. `build_declaration` consumes the reader's established
no-code state and P0 declaration without manufacturing an executable graph.

`MethodResult` owns its header and optional `MethodCode`. The header retains the
original MethodKey, owner descriptor, flags, exact declaration, method kind and
method-scoped annotation records. Metadata sharing retains owned immutable P0
records, not a DEX lease. Names retain UTF-16 units and an identifier/original-name
form; constructors and class initializers have distinct kinds.

The original flags distinguish `ACC_SYNCHRONIZED` (0x20, VM locking on native
methods) from `ACC_DECLARED_SYNCHRONIZED` (0x20000, the declared Java modifier,
with no execution effect beyond reflection), as specified by the
[DEX access-flags table](https://source.android.com/docs/core/runtime/dex-format#access-flags)
and pinned ART [modifier definitions](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/libdexfile/dex/modifiers.h).
P7 MUST NOT synthesize or suppress body monitor operations from the declared
modifier. P9's [declaration spelling](09-rendering.md#declarations-metadata-and-literal-identity)
uses `synchronized` for either bit without changing P7's execution plan or raw
flags.

`MethodCode` contains flat expression, statement, reference, type-use, local,
block, transfer and control arenas. It also owns original coverage, operation
provenance, copy schedules, construction publications, exception chains/sites,
handlers, array payloads/component gaps and upstream issues. Original stage IDs
are typed provenance coordinates, not permission to query a live predecessor.
Type uses retain descriptors and original sources; P8 owns class-wide display
names and imports.

`ExpressedMethod` pairs the result with privately retained P6 analysis for the
existing inspection consumers. `inspection()` lends restricted read-only facts
from that same execution. It exposes no owned predecessor or solver.
`into_result()` releases retained analysis and returns Arc<MethodResult>. P8 and
the same-execution inspection response share this immutable owned expression,
without duplicating arenas or borrowing DEX. Partial expressions likewise retain
a shared unpublished prefix; its type-use and local-declaration views do not
promote it to a completed MethodResult. The production method chain has no parallel P6 success path.

## Unified operation lowering

P7 uses exhaustive P2 operation/terminal matches for both natural and explicit
control. Lowering starts from established locals, snapshots and exact literal bits.
The final layout either retains P5's original materialization or composes a
definition at an admitted adjacent use. P7 MUST NOT inline an effect merely
because its result has one use. Unused calls, loads, checks and arithmetic effects
retain their original execution and exception sites.

The operation rules cover moves, constants, unary conversions, binary/compare
operations, field/array reads and writes, array length/fill/allocation,
instance-of, check-cast, invocation, object allocation/initialization, monitors,
exception capture and unresolved invocation/result layouts. Reverse subtraction
retains operand orientation. Literal interpretation is P4's view, not a guessed
Java type. `check-cast` is a statement with no invented SSA write.

`Arguments::Unavailable` retains the original register words and storage reason;
it is not an empty parameter list. An unavailable expression retains its concrete
gap and provenance. P7 MUST NOT substitute zero, null, NOP or a fabricated local.
Reference-read errors and opcode-use errors remain distinct.

A constructor or delegated `this`/`super` call also uses the natural surface: its
name is never a Java identifier and the receiver or created type already names the
target, so only an unresolved competing delivered overload qualifies it.

Ordinary calls use a natural surface unless the delivered references leave the
binding ambiguous: an original-name form, a missing signature or argument layout,
dynamic invocation, or another delivered reference with the same owner, name and
arity but a different parameter list or return type. An argument whose displayed
type only widens into the formal parameter is not by itself such an ambiguity;
P7 inserts no conversion for it. Unknown external declarations do not block
ordinary calls.

For competing overloads only, P7 omits the binding disclosure when the recovered
types of the printed arguments select exactly the original target by equal
original type units.
Every argument MUST be a leaf value or literal read with available original read
provenance and exactly one recovered type whose units are known. Snapshot reads,
missing or conflicting types and unavailable type spellings keep the disclosure.
P7 compares only equal original type units at each parameter position among
references with the same owner, name and arity, deduplicated by original reference
identity. Exactly one distinct candidate MUST match, and it MUST be the original
invocation target. A unique match to another target keeps the disclosure.
Custom/polymorphic calls and original-name forms other than constructors retain
their existing qualification rules. This comparison is not Java overload
resolution, assignability, inheritance, widening or a claim about declarations
outside the input.

The binding suffix discloses the original target; it is not a Java-legality
device. Both forms retain the complete original reference, dispatch kind,
receiver, argument order and invocation prototype. P7 MUST NOT insert casts,
typed aliases, boxing or helpers to influence binding. `LocalSpelling` uses a
nominal name only for complete, equal delivered reference type sources; other
locals retain their machine storage representation. Header parameter names are
fixed by P7, including no-code declarations. P9 prints the selected suffix as
`[binding ...]`; a natural call prints only the receiver, member and arguments.

P1 supplies checked call-site IDs, bytes/range, argument count and flat bootstrap
events. A separate flat leaf-declaration pool retains referenced strings, types,
fields, methods, prototypes and handles. P7 copies these facts into owned reference
records, preserving container order and literal bits. It does not reopen encoded
values or replace custom/polymorphic calls with ordinary calls.

## Materialization and ordered phases

A statement retains ordered operand reads, P5 definitions and distinct
before-operation, before-throw snapshot and after-normal-commit copy references.
Normal transfer copies execute only after selection of that original transfer.
Handler receives execute only on exception arrival, after the original capture
when present. `HandlerCapture::Body` and `Catch` distinguish the unique capture
owner; a handler label MUST NOT bind it a second time.

Each copy bundle retains P5's requirements separately from executable Save/Assign
steps. P7 maps storage reads and destinations without rebuilding a parallel-copy
schedule. Unavailable assignments remain gaps; a plan ID alone does not authorize
an assignment. Pure literal definitions can omit an emitted statement only when
all publication plans are literal and no before/snapshot/after copy is attached.

A store prints as an ordinary assignment unless its array, index or receiver slot
is an inlined expression or an unavailable operand. Those slots are never
composed, so reading them before the value cannot be observed and the store's
null/bounds check still follows the value. A narrow element store keeps its
original width marker.

A void `return` that is the last printed action of a method body is the same as
reaching the closing brace, so P7 marks it display-omitted when nothing is attached
to it: no copies, phases, site or unavailable definition, and no statement after it.

A break or continue whose target is the innermost enclosing scope needs no label,
and a continue that ends the loop body is the same as reaching the body end, so the
final plan prints neither. Its attached copies remain ordered statements.

A tail-test repetition-edge copy set may move into the body end when the delivered
facts prove both schedules equal: the predicate has one normal arrival from inside
the body; the condition reads no destination the copies write; the exiting-edge
copies neither read nor write those destinations; and no read outside the loop
subtree, handler receive or construction observes them. Copies are plain
assignments, so moving them introduces no effect or exception site. A head-test
predicate is reached on entry with different state, so those copies stay on their
edge.

A loop predicate becomes an inline condition only when its block holds no label,
statement, continuation or terminal phase. Edge copies may remain: they are printed
inside the condition by their original repeating or exiting role, so a break still
does not execute exiting-edge copies.

A control's planned continuation contributes no second jump when its last printed
block already jumps to the same label; two adjacent jumps carry the same effect
and only one is printed.

A jump, including a planned continuation, whose target prints next is ordinary
sequencing, so P7 MUST omit its displayed `goto`. The search follows the final
emission layout through Sequence, Labelled and Graph containers and the last
printed If branch. It stops at a parent's continuation and at Loop, Try, Switch
and Synchronized boundaries, whose trailing text or actions prevent fallthrough.

P7 MUST omit an `else` emptied by this collapse only when its false-edge copies
are empty and its control is an empty Block or a label-free Sequence of empty
children. Each such control has no required label, continuation or deferred
control; each block has no actions or displayed label. An empty block end is
None, a list of copy-free Fallthrough transfers, or a Goto terminal with no
terminal phases and a copy-free Fallthrough transfer. Other terminals retain the
branch. For an explicit If terminal, P7 MUST omit the false arm only when its
transfer is copy-free Fallthrough and all terminal phases are empty. P9 consumes
this separate terminal display flag without recomputing emptiness. These display
choices preserve the original blocks, controls, labels, transfers, sites, copies
and exception routes.

Each exception site also records whether its route still needs an inline marker:
a natural single route targeting an enclosing printed catch does not, because the
chain line and the catch scope already state it. Other sites keep a compact chain
reference.

P5-selected construction groups become `New` expressions placed at the original
constructor call. Corridor operations that prepare arguments print as their own
statements at their original positions, so only the allocation moves. The group retains
allocation and initialization operations, member/edge provenance, arguments,
normal publication point and all throwing sites. Its selected common snapshot
prelude executes once. Alias publications occur only after successful
initialization; failed construction does not invent external assignment or undo
callee effects. Unselected construction relations retain separate allocation and
initialization statements. `this`/`super` syntax requires the existing pending
initialization fact and an original first operation without preceding copy/save
obligations; otherwise P7 retains explicit initialization at its original position.

## Control, protection and synchronized outlets

P7 retains P6's selected Block/Sequence/If/Loop/Switch/LabelledBlock/Try/Synchronized/
Graph kinds, child ownership and original continuation evidence. Labels have fixed
method-local display names. Block completion distinguishes ordinary terminals from
syntax-owned predicates/exits; retained evidence is not a second emission.
All original switch roles, duplicate-target links and selected-edge copies survive.
Explicit graph transfers use the same statements and storage as natural regions.

P7 MUST copy only P6's published reader-facing diagnostics into `Issue::Control`.
Under the [P6 publication contract](06-regions.md#planning-state-and-published-diagnostics),
these are `Storage`, `Lock` and `Protection` records, not `Graph` planning records.
P7 retains the selected graph reason in its control expression; P9 prints it once
in `graph<reason>`, not as a duplicate method-body `diagnostic` statement.
Rejected-candidate reasons at non-graph entries are not method statements or graph
annotations. P7/P9 MUST NOT re-derive or display P6's private candidate history.
The retained non-graph diagnostics keep their existing `diagnostic` spelling,
content, order and method-head placement; no other text states those issues.

Exception sites retain original completion, ordered chain, edge delivery and
snapshot provenance. Natural catch delivery, explicit protection, propagation and
P6's method-local ImpossibleRelease finding stay distinct. P7 MUST NOT discard an
original exceptional edge or invent a handler from syntax adjacency.

Each synchronized outlet identifies its release statement, entry label,
original transfer and exact resume intent. Before-operand copies precede the
release read; snapshots follow that read. `completion` orders after-commit copies,
selected transfers with their copies, and routing-terminal phases after release.
The retained `after_release` list describes obligations, not a second schedule.
`Resume` describes the established destination/abrupt intent, not a second terminal
event. Rethrow retains its exact original expression; capture is provenance only.
P7 MUST NOT move unlocked work into the held body or infer outlets from a common
follow. These outlet records retain source ownership; they are not another
execution route alongside the final emission plan.

`EmissionPlan` is P9's sole execution layout. It binds the supplied P5 copy
locations to operation, terminal and selected-edge phases without rescheduling
steps. It fixes child roles, condition polarity, method-local control names and
scope continuations. Syntax-owned header/predicate continuations are deferred to
their owner, so a header cannot jump past its own test or monitor acquisition.
Loop-condition edge copies stay inside that test's selected-edge clauses; they
are not appended after a loop. `LoopPlacement` distinguishes infinite, head-test
and tail-test forms and records when an empty predicate can use a compact form.

Matched releases print at their original nodes. Their after-release copies and
routing terminals also stay at their original nodes and therefore keep original
lexical try/catch ownership. The synchronized container has explicit planned
exits, not another implicit release at its closing brace. A selected compound
construction prints at the original constructor call; correlated members that
only alias the object or define literals remain suppressed. These are
placements of existing plans, not region identification or a new dispatcher.

## Composition before publication

`p7/compose` consumes the lowered P7 operations, P5 materializations and the
completed P6-derived emission layout. It does not decode input, rebuild SSA,
identify regions or reschedule copies. Composition modifies the one final
`EmissionPlan` before `MethodResult` is published. P9 prints that plan.

The expression pass composes an integral/floating pure unary or binary definition
only when its original result is a single available numeric local, has one
compatible use, and has no associated effect, exception or copy/snapshot phase.
An adjacent operation use must be its first captured operand and have no
before-operation copies. A return use must be the next original node in the same
block, with one incoming fallthrough, no intervening work or terminal phases,
and adjacent selected sequence children. All operands remain in original order.
Locals participating in copies or construction publication are not hidden. An
unavailable copy dependency prevents this optional composition for that method.

An original `instance-of` can compose with an immediately following equality or
inequality branch only when its checked type reference is available, its original
exception propagates without a local handler or snapshot, and the same adjacency
and storage requirements hold. Its check and possible exception still occur once
before branch selection. No cast or new SSA definition is invented.

The current expression pass does not nest already composed operands. This keeps
expression rendering stack usage independent of input expression depth. Original
operations, PCs, statements, transfer records and gaps remain owned; composed
operations map to the expression range, and a cross-node source control spans
through its composed use.

The layout pass keeps labels referenced by explicit transfers, graph entries,
exception handlers and construction continuations. Unreferenced labels retain
zero-length source anchors. An unreferenced Sequence or If wrapper can be
transparent. A labeled or graph scope keeps its braces, graph header and
children, but prints its control name only when a printed transfer names that
control. A labeled scope that prints no name, has no continuation or deferred
composition, and has one unnamed printed child prints that child without its own
braces, unless a bare `break` or `continue` targets the scope. A loop condition
becomes ordinary `while`/`do-while` only when the predicate block, terminal
phases and both edge copy schedules contain no work. Otherwise the existing
explicit test retains its edge obligations. Pure propagation chains can be
implicit only when no displayed site or handler requires the chain.

Integral literals encoded in integer arithmetic instructions retain their original
bits and an integral view. Other bit-pattern literals keep their supplied views.

## Failure and evidence

`ExpressionError` preserves concrete request/resource errors and necessary
construction invariants. `ExpressionStop` retains mature P6 facts and an
unpublished `ExpressionPartial`; partial arenas are not a usable MethodResult.
`fragments()` exposes operation-associated statements with closed dependencies.
Unassociated arena entries remain construction evidence and can still reference
pending records. Copy/storage prefixes do not promise a complete control layout.
Local input/type/storage/control uncertainty remains in the owned mature result
and affects only its actual consumers. No error kind automatically truncates
siblings or promotes a construction prefix to success.

The previous core test was removed by operator decision. Ordinary Engine
requests do not run independent replay, correspondence passes or a verification
switch. P7 JSON serializes the inspection product; final text and UTF-8 spans are
owned by [P9](09-rendering.md), without a Java-legality gate. P8/P9 MUST consume
the existing expression and naming products rather than repairing or re-analyzing
execution. Corpus completion is not an arbitrary VM, heap/callee or lock proof.
