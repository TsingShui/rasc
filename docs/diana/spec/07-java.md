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
Java type. `check-cast` is a check with no invented SSA write or replacement
binding.

P7 composes a check-cast's display into the first printed read of its checked
local only when that read is the first evaluated value of the following statement
or the first executed step of this block's single unconditional transfer copy set.
The carrier names that exact statement and read-expression occurrence, so a second
read of the same storage in the printed form never carries the check. The check's
own statement, operation, exception site, reference, resolved type name and
original copy steps MUST stay in the product with their source origins; composition
removes one display action, not the check event, and P7 MUST refuse the composition
when a phase, route, later operand, second transfer, selected branch or earlier copy
step would move the check past an event it precedes.

P7 omits a check-cast's own text only from P4's published `Redundant` disposition,
which is a must-fact over every reaching transfer. The omitted statement keeps its
statement, operation, pc, reference, resolved type name, original route and a
zero-length source anchor at its original position (`OmittedCheck`), and the check
never composes: the surviving carrier prints its original text, so no redundant
`((T) v)` is invented.
A check that carries a before/snapshot/after copy phase, a printed route or an
output definition keeps its statement even when P4 established redundancy, because
its removal could not keep those obligations. `Retained` and `Unknown`
dispositions always keep the statement. P7 performs no alias, reachability or
dominance reasoning of its own.

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

The signature qualification discloses the original reference; it is not a Java-legality
device. Both forms retain the complete original reference, dispatch kind,
receiver, argument order and invocation prototype. P7 MUST NOT insert casts,
typed aliases, boxing or helpers to influence binding. `LocalSpelling` uses a
nominal name only for complete, equal delivered reference type sources; other
locals retain their machine storage representation. Header parameter names are
fixed by P7, including no-code declarations.

P7 selects `Parameters` when the parameter list distinguishes the original
reference from its competing delivered overloads. A competing reference with the
same parameters and a different return type selects `FullSignature`; other
mandatory disclosures also retain that full form. The selection scans all
competing references, so their delivery order cannot hide a return-type collision.
Both qualified forms carry an owner-disclosure flag. For an instance call, P7
omits that disclosure only when the final receiver remains a local read whose
nominal `LocalSpelling` has exactly the original owner's type units, with no
composed producer or inline check at that read. Owner selection runs after
expression and check composition. This is display
selection, not hierarchy resolution; inheritance or multiple implementations
alone do not select qualification. Static calls already print the owner.
P9 prints the selected signature and owner according to
[the call notation](09-rendering.md#expression-and-control-notation), without
recomputing these decisions. Unread references keep their concrete unavailable
form rather than a guessed signature.

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

A loop predicate becomes an inline condition only when its delivered evaluation
subtree consists of label-free, continuation-free Sequences and Blocks with no
remaining statement or terminal phase. Suppressed composed producers and copy-free
original fallthrough links retain their origins in that subtree. Edge copies may
remain: they are printed inside the condition by their original repeating or exiting role, so a break still
does not execute exiting-edge copies.

A control's planned continuation contributes no second jump when its last printed
block already jumps to the same label; two adjacent jumps carry the same effect
and only one is printed.

A jump, including a planned continuation, whose target prints next is ordinary
sequencing, so P7 MUST omit its displayed `goto`. The search follows the final
emission layout through Sequence, Labelled and Graph containers and the last
printed If branch. It stops at a parent's continuation and at Loop, Try, Switch
and Synchronized boundaries, whose trailing text or actions prevent fallthrough.
For the successor, P7 follows the established entry through a Sequence's first
child or an If, Condition or Switch header only when every traversed container
has the target as its entry. It does not enter a loop, protected scope or graph to find
a target. This same entry rule applies to transfers and planned continuations.

P7 MUST omit an `else` emptied by this collapse only when its false-edge copies
are empty and its control is an empty Block or a label-free Sequence of empty
children. Each such control has no required label, continuation or deferred
control; each block has no actions or displayed label. An empty block end is
None, a list of copy-free Fallthrough transfers, or a Goto terminal with no
terminal phases and a copy-free Fallthrough transfer. Other terminals retain the
branch. For an explicit If terminal, P7 MUST omit the false arm only when its
transfer is copy-free Fallthrough and all terminal phases are empty. P9 consumes
this separate terminal display flag without recomputing emptiness.

When a syntax-owned If's true arm is empty under the same rules and its false
arm prints, P7 selects an inverted single-arm display using `PredicateEmission`'s
exact negation. Original true/false edge identities and child ownership stay
unchanged; the false-edge copies begin the displayed body. A true-edge copy
bundle with no executable steps or unavailable assignments is nonprinting; P9
retains its zero-length copy and transfer origins without moving any step.
A displayed predicate exception site prevents inversion. If both arms are empty,
P7 suppresses the test only when it has no terminal phases, displayed site or
composed operand evaluation, and its control has no continuation or deferred
control; its header still prints at the original position. This omission requires
both arms' copy bundles to have no executable steps or unavailable assignments.
An explicit If terminal
with two such Fallthrough arms uses a separate empty-If display flag under those
same predicate and control restrictions. P9 retains both edges' zero-length copy
and transfer origins.

P7 selects `else if` only for a copy-free false edge whose body is one printable
If, possibly inside label-free Sequences of otherwise empty children. Those
containers and the inner If have no required label, continuation or deferred
control; the inner header is empty and has no before-terminal copies. P9 prints
the selected chain without another scope or duplicate child. These display
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
P7 consumes P6's `private_cases` child layout without finding regions again. Its
switch emission lists the supplied bodies in arm order. Each original arm prints
its case/default label and selected-edge copies before its unique body, without
a dispatch goto. An empty default body still executes its original selected-edge
copies and the delivered switch completion. Case-body normal completion can be a
selected Break; a successful original Return/Throw bypasses it. P7 lowers P6's
scope completion without discovering another target, and chooses an unlabelled
break/continue only for the supplied innermost selected scope. The selector
terminal and arm transfer identities remain owned.
Other switches retain the single-body dispatch layout. P9 follows these placements;
it does not split graphs or infer private membership from printed text.
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

A `new-instance` statement that publishes its own storage keeps the original
allocation point: P7 marks it so P9 prints the allocated type and local where the
allocation runs, and the header omits that local's declaration. The constructor
call stays at its own statement, so neither the allocation nor its publication
moves.

`EmissionPlan` is P9's sole execution layout. It binds the supplied P5 copy
locations to operation, terminal and selected-edge phases without rescheduling
steps. It fixes child roles, condition polarity, method-local control names and
scope continuations. Control completion distinguishes goto, break and continue
without inventing another edge copy. Syntax-owned header/predicate continuations are deferred to
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

Ordinary If, ordered Condition and Loop predicates share one producer,
exact-use matcher, branch-role gate and composition installer. Generic layout
consumers use selected emission children and continuations; retained source
children continue to describe lexical ancestry, not printed succession.

The expression pass composes a move, a resolved string, an integral/floating unary
or binary definition, or a numeric/reference field or call definition only when its
original result is a single available value or reference carrier, has one
compatible use, and carries no copy/snapshot phase. A move's source must be a
delivered direct value read with a compatible view. A reference source and use
must each have an established initialized state; allocation carriers and other
position-related initialization states remain separate.

An adjacent operation use ordinarily must be both the first original capture and
the first operand in its selected spelling, and have no before-operation copies.
A resolved string definition with one compatible read can instead occupy a later
call argument when all preceding printed operands are uncomposed local reads or
literals. The argument must retain its exact original operation/use identity.
The string resolution still precedes every observable consumer event; a receiver
expression, earlier call or other non-leaf operand refuses this composition.
String definitions retain their propagation-only exception site, original string
reference and UTF-16 units; they are not treated as effect-free literals.
Construction arguments do not use this later-call-argument rule. Stores
retain the existing pure-producer rule because their selected spelling can print
the receiver before the value. A return or predicate use must be the next original
node in the same block, with one incoming fallthrough, no intervening work or
terminal phases, and adjacent selected Sequence or Graph children. An effectful
producer must have one original exception site that propagates without a snapshot.
A predicate uses the producer as its first captured operand. When both original
branch transfers have the same destination and no copies, the producer remains a
statement instead of being moved into an otherwise empty If. All operands remain
in original order. Locals participating in copies or construction publication are
not hidden. An unavailable copy dependency prevents this optional composition
for that method.

For later test runs of an ordered Condition, P7 also supports a store-preserving
composition when the result has additional reads. `ComposedResult::Store` names
the original local; P9 prints `(local = expression)` at the exact matched
terminal read, and the local's declaration and other reads remain visible.
The original value/state/use identity, initialization, commit, exception and
phase requirements still apply. A local read by original normal-edge copies
requires store retention, even if its expression arena has only one read.
Copy destinations, snapshots, non-edge copy phases, relocated copies and
construction publications pin the definition to its separate statement.
The first test run keeps a multi-read definition
in its printed prefix; ordinary operation, If and Loop composition still require
one read. P7 MUST NOT substitute a second evaluation for the retained store.

A single-use `cmp-long` can compose with an integral-zero branch when both source
operands have established long views. P7 delivers `ComposedPredicate::LongComparison`
with those original operands and the branch operator; P9 prints that relation,
not a subtraction that could overflow. The comparison's original operation,
expression and consumed read retain origins at the predicate. Pair comparisons,
other uses of the numeric comparison result, store-retaining compositions and
floating `cmpl`/`cmpg` remain outside this rule. The shared adjacency, phase and
exact-use requirements still apply.

An original `instance-of` can compose with an immediately following equality or
inequality branch only when its checked type reference is available, its original
exception propagates without a local handler or snapshot, and the same adjacency
and storage requirements hold. Its check and possible exception still occur once
before branch selection. No cast or new SSA definition is invented.

A call can compose at an adjacent use when its receiver and arguments are leaf
reads or literals and its exception route propagates without snapshots. P7 gives
the call boolean shorthand only when its delivered storage family is integral and
its complete, nonempty recovered type sources all name primitive boolean (`Z`).
An integral call whose complete, nonempty sources uniformly name primitive int
(`I`) keeps its numeric comparison and uses the ordered Condition producer path;
mixed or unknown sources refuse that shorthand. Other call results keep their
numeric or reference interpretation. A call with boolean shorthand composes only
into a terminal, not an operation. For a terminal, the call is the last original
operation before the empty, single-entry header; no copies, phases or intervening
evaluation move across it. The same eligibility and installation path accepts a
P6-owned loop predicate when its producer and terminal are adjacent children of
the delivered evaluation Sequence. P7 does not find another loop or exit corridor.
The producer still executes initially and on every original head continue, or
stays at its original tail position. Call results retain their `AfterResult`
commit and move-result PC origin; other candidates still require `AtOperation`.
Calls with printed exception routes remain statements. Additional reads require
the store-preserving Condition form described above; value substitution retains
the single-read requirement.

A delivered ordered condition composes a test's prelude only as an ordered
suffix of that test run's statements. Each composed definition must name a
distinct terminal operand and the matched operand positions must strictly
increase, so P9's left-to-right operand evaluation reproduces the original field
read, call, exception and commit order. Earlier statements stay printed in the
header when that run prints; a later run must be composed completely because it
prints no header of its own. A boolean definition composes only into the same
`Eqz`/`Nez` integral-zero branch role as the ordinary boolean path; a pair or a
different comparison keeps its numeric materialization. When any run cannot meet
these order, copy, phase or branch-role conditions, the condition keeps the
complete explicit schedule from its delivered P6 plan, including when local copy
access is unavailable. P7 restores terminals at the delivered predicate portions
and lowers the delivered child order, goto links and per-child normal completions
into `EmissionPlan` before selecting labels or display omissions. P7 MUST NOT
collect descendant blocks to discover that order or infer completion from a
printed successor. It MUST retain the Condition's tests/shared/other source roles
and original continuation evidence. Each referenced child keeps its existing
operations, terminal phases, copies, handlers and abrupt-exit scopes; its emission
continuation comes from the selected P6 alternative.
The ordinary display pass can omit a goto to the next printed target, as for
other completed layouts; that does not reconstruct a condition schedule.
The tests' selected edges and all intermediate other edges require no copy
bundles. The final other edge can retain copies at an explicit else position:
every bundle must retain original emission, name that same original normal edge,
and have no unavailable assignment. With no other-arm body, P7 selects
`ConditionElse::Copies`; with a delivered body, it retains
`ConditionElse::Body`. Copies remain on the final other edge, never in the common
continuation or on an earlier selected edge. P7 adds no region, local or copy
schedule to represent this else.

P7 installs compositions only after every test run admits its schedule; a
refused later run leaves no hidden local, relocated store or copy-only else. A resolved
statement's operations, statements and result PC remain owned and print at the
composed operand; no call is deduplicated.

With `trace-passes`, the condition pass reports one final layout decision per
Condition, identified by method and control. An explicit decision reports the
first blocking test and, when identified, statement and admission reason.
Installed store compositions additionally report their operation, statement,
operand and storage identities. A copy-only else reports its original transfer,
bundle and executable-step count. These are decisions, not counts of removable
gotos or independent semantic verification. An ordinary build emits no such
diagnostics.

The expression pass selects terminal uses before adjacent operations and visits
operation pairs from the end. An earlier producer may compose into an already
composed consumer when the same first-operand and original-order conditions hold.
A producer composed into a store is a nesting boundary, because the store can
print a receiver before its value. P9 renders composed expressions with a
fallibly allocated work stack rather than host recursion. Original operations,
PCs, statements, transfer records and gaps remain owned; composed operations map
to the expression range, and a cross-node source control spans through its
composed use.

The layout pass keeps labels referenced by explicit transfers, graph entries,
exception handlers and construction continuations. Unreferenced labels retain
zero-length source anchors. An unreferenced Sequence or If wrapper can be
transparent. A graph scope keeps its braces, graph header and children. A
labeled scope prints its control name only when a printed transfer names that
control. P7 marks a labeled scope transparent when it prints no name, has no
continuation or deferred composition, and has one unnamed printed child; P9
then prints that child without the scope's own braces. The original control,
labels and transfers remain in the product, including a bare `break` whose
stored target is the transparent scope. A loop condition becomes ordinary
`while`/`do-while` when its delivered evaluation subtree has no remaining prelude
work after shared adjacent-expression composition. Terminal phases still constrain
compactness; selected-edge copies retain their original repeat/exit clauses.
Otherwise the existing explicit test retains all prelude and edge obligations.
Pure propagation chains can be implicit only when no displayed site or handler requires the chain.

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
