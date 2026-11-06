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

Ordinary calls use a natural surface unless the delivered references leave the
binding ambiguous: an original-name form, a missing signature or argument layout,
dynamic invocation, or another delivered reference with the same owner, name and
arity but a different parameter list or return type. An argument whose displayed
type only widens into the formal parameter is not such an ambiguity, so the call
keeps its natural surface; P7 inserts no conversion for it. This comparison is
not overload resolution, subtype checking or a claim about declarations outside
the input. Unknown external declarations do not block ordinary calls. Both forms
retain the complete original
reference, dispatch kind, receiver, argument order and invocation prototype.
P7 MUST NOT insert casts, typed aliases, boxing or helpers to influence binding.
`LocalSpelling` uses a nominal name only for complete, equal delivered reference
type sources; other locals retain their machine storage representation. Calls
using such reference carriers select an explicit signature rather than pretending
that the display supplies a nominal argument type. Header parameter names are
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

A jump whose destination is the statement printed immediately after it is
ordinary sequencing, so the final plan prints no `goto`. The search may pass
upward only through containers that print their children in order and add nothing
after them; a loop body never falls through its own test. Each exception site also
records whether its route still needs an inline marker: a natural single route
targeting an enclosing printed catch does not, because the chain line and the catch
scope already state it. Other sites keep a compact chain reference.

P5-selected construction groups become `New` expressions. The group retains
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
construction resumes at the original initializer node when it crosses nodes;
absorbed statements remain suppressed even for backward-PC channels. These are
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
transparent; graph and labeled scopes remain explicit. A loop condition becomes
ordinary `while`/`do-while` only when the predicate block, terminal phases and
both edge copy schedules contain no work. Otherwise the existing explicit test
retains its edge obligations. Pure propagation chains can be implicit only when
no displayed site or handler requires the chain.

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
