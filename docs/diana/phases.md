# Diana phases

The approved architecture is [PIPELINE_DESIGN](PIPELINE_DESIGN.md). The application
owns files/APKs and lends DEX bytes to the native engine. Engine holds indexes,
starts method tasks and delivers classified products; stage algorithms stay in
their owning modules.

| Stage | Active location | Product and responsibility |
|---|---|---|
| P0 | `crates/diana-decomp/src/p0/` | `ClassFacts`: original declarations, four-group directory, static preparation and metadata facts; the class declaration is shared as an immutable `OwnerFacts` handle. |
| Method reader | `crates/diana-decomp/src/input/` | `MethodSource`: bound P0 method entry, safely located code and independent metadata state. |
| P1 | `crates/diana-decomp/src/p1/` | `DecodedMethod`: complete instruction/payload facts, entry/reference layouts and original structural associations. |
| P2 | `crates/diana-decomp/src/p2/` | `FlowMethod`: ordered operations/terminals, explicit completion rules, normal/exception transfers, exits and source coverage. |
| P3 | `crates/diana-decomp/src/p3/` | `SsaMethod`: exact-slot states, LT dominators, live-pruned IDF Phis, definitions/uses, availability and original state-position mappings. |
| P4 | `crates/diana-decomp/src/p4/` | `TypedMethod`: value facts, per-use interpretations, Phi incoming adaptations and position-related initialization. |
| P5 | `crates/diana-decomp/src/p5/` | `StorageMethod`: value bindings, original-position materialization, per-transfer copies, exception snapshots and local storage gaps. |

P6–P9 retain their specified responsibilities in PIPELINE_DESIGN: regions,
method expression, class/name assembly and pure rendering.
Their archived implementations are not active stages or alternate recovery paths.
The [P1–P9 local-result contract](PIPELINE_DESIGN.md#92-p1p9-统一消费局部结果) applies
to every stage: mature products account for coverage, established facts and local
unavailability, and downstream processing preserves independent results. This
contract is broader than the currently supported local data/binding continuation;
control/protection publication limits and inactive stages remain recorded in the
engine README.

## Current method scheduling

`engine.rs::MethodTask::run` is shared by `Engine::analyze_method` and the class
request entry `Engine::analyze`:

```text
P0 ClassFacts → method reader → P1 DecodedMethod → P2 FlowMethod → P3 SsaMethod → P4 TypedMethod → P5 StorageMethod
                   └─ declaration-only
                              └─ P1 stop with decoded prefix
                                                └─ FlowStop with P1 input
                                                                 └─ SsaStop with P2 input

Stage stops retain their error and unpublished construction facts. P4 TypeStop
retains SSA and TypePartial, not a relabeled TypedMethod. P5 StorageStop retains
P4 and StoragePartial, not a relabeled StorageMethod.
```

P2–P5 construction requires a `MethodTask` credential issued only by Engine.
Each stage owns construction; independent checks are test-only. Engine cannot assemble successful
FlowMethod, SsaMethod, TypedMethod or StorageMethod products. Local reference, operand and binding issues can
flow through to SSA with independent facts preserved. Class requests reserve result
storage before tasks and retain every selected known member result, including
method-task construction failures; error kind does not truncate siblings.
`Engine::with_method_jobs(n)` selects bounded method concurrency. The default and
values zero/one run sequentially; multi-member requests with `n > 1` use the
Engine's lazily created Rayon pool. Both branches call the same MethodTask.
Single-method requests do not initialize a pool. `ClassAnalysis.terminated` is
removed rather than retained as an always-empty class-abort mechanism.

P0 selection and fallible result reservation finish before method tasks start.
An indexed parallel map writes one result per selected entry directly into that
reserved destination vector. There is no fail-fast result iterator, sorting pass
or separate completion-reordering buffer. Scoped `install` completes every task
before returning borrowed products; method scratch stays private.

Pool creation caches only a successful pool. Initialization is serialized, but
method execution does not hold that mutex. A startup failure returns the original
Rayon error as `RequestError::MethodScheduling`/`ResourceExhausted` before any
member task starts; P0 remains separately queryable. The error is not cached as a
missing pool, so later requests can retry startup. A method-private error after
startup remains that member's result, without cancelling siblings.

`dbg_shell` is the sole engine inspection example. P1–P5 views and batch commands
observe the Engine products; they do not start a second stage chain. Byte
roundtrip is an independent check over retained original evidence, not production
DEX reconstruction.

## Product access and construction authority

- A stage MUST privately hold its predecessor and expose immutable queries for
  actual downstream facts. It MUST NOT provide a public predecessor getter or
  Deref chain that lets later analysis substitute raw decoding for missing facts.
- `FlowMethod` queries provide operations, ordered slots, completion rules,
  transfers, entry and original declarations. `MethodEvidence` is a separate
  borrowed debug/check view; it cannot extract a DecodedMethod or invoke a stage.
- `SsaMethod` privately owns P2. `SsaFacts` exposes ordered bindings, definitions,
  source availability and Phi incoming/state-position mappings. `ExecutionFacts`
  is an immutable query view without predecessor extraction; raw inspection stays
  in the separate `MethodEvidence` view.
- Each method product retains P0's immutable `OwnerFacts` handle rather than
  copying the class declaration. Sharing that read-only declaration does not add
  inter-method state: method products remain independent. Engine can analyze class
  members with bounded Rayon concurrency. Batch drivers that also run independent
  DEX processes need to coordinate these two concurrency settings.
- `TypedMethod` privately owns P3. TypeFacts delivers value/use/Phi and initialization
  states; SSA and execution remain immutable fact views. P4 does not receive the
  whole ProgramIndex or synthesize missing producer facts.
- `StorageMethod` privately owns P4. StorageFacts supplies bindings, use accesses,
  original commit/materialization sites, incoming routes, copy steps and exception
  snapshots. P4 interpretations and execution queries remain immutable. Retained
  SSA inspection is isolated in borrowed `AnalysisEvidence`, not predecessor extraction.
- Only owning-stage constructors with scheduler-issued authority can publish
  mature products. Consumers MUST NOT promote partial snapshots or reuse stage
  construction to retry a stopped method.
- Independent stage correspondence checks compile only under `cfg(test)`.
  Normal Engine requests run the algorithms without verification passes or a
  verification switch. Input safety and construction error handling remain.
  Test verifiers MUST NOT repair bindings or invoke another stage's analysis.
- Method results and failure kinds are distinct. Unknown heap/lock outcomes are
  explicit semantics, not a successful verification badge or an input error.

The stage chapters define [input](spec/01-input.md), [P0](spec/00-class.md) and
[P2 execution/tests](spec/02-flow.md), [P3 SSA/tests](spec/03-ssa.md) and
[P4 interpretations](spec/04-types.md) and [P5 storage](spec/05-variables.md).
PIPELINE_DESIGN remains authoritative
for ownership, consumers, failure preservation and the later product chain.
