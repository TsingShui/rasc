# Diana phases

The approved architecture is [PIPELINE_DESIGN](PIPELINE_DESIGN.md). The application
owns files/APKs and lends DEX bytes to the native engine. Engine holds indexes,
starts method tasks and delivers classified products; stage algorithms stay in
their owning modules.

| Stage | Active location | Product and responsibility |
|---|---|---|
| P0 | `crates/diana-decomp/src/p0/` | `ClassFacts`: original declarations, four-group directory, static preparation and metadata facts. |
| Method reader | `crates/diana-decomp/src/input/` | `MethodSource`: bound P0 method entry, safely located code and independent metadata state. |
| P1 | `crates/diana-decomp/src/p1/` | `DecodedMethod`: complete instruction/payload facts, entry/reference layouts and original structural associations. |
| P2 | `crates/diana-decomp/src/p2/` | `FlowMethod`: ordered operations/terminals, explicit completion rules, normal/exception transfers, exits and source coverage. |

P3–P9 retain their specified responsibilities in PIPELINE_DESIGN: SSA, types,
storage, regions, method expression, class/name assembly and pure rendering.
Their archived implementations are not active stages or alternate recovery paths.

## Current method scheduling

`engine.rs::MethodTask::run` is shared by `Engine::analyze_method` and the class
request entry `Engine::analyze`:

```text
P0 ClassFacts → method reader → P1 DecodedMethod → P2 FlowMethod
                         └─ declaration-only
                                   └─ P1 stop with decoded prefix
                                                     └─ FlowStop with P1 input,
                                                        error and partial facts
```

P2 construction requires a `MethodTask` credential issued only by Engine.
The stage owns both construction and checks; Engine cannot assemble a successful
FlowMethod. A class stop retains delivered members with an explicit termination
kind. Local unsupported methods do not erase independent siblings. The currently
implemented method scheduling loop is sequential.

`dbg_shell` is the sole engine inspection example. P1/P2 views and batch commands
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
- Only owning-stage constructors with scheduler-issued authority can publish
  mature products. Consumers MUST NOT promote partial snapshots or reuse stage
  construction to retry a stopped method.
- Stage checks validate their own conversions against original obligations.
  They MUST NOT construct SSA/types or manufacture missing predecessor facts.
- Method results and failure kinds are distinct. Unknown heap/lock outcomes are
  explicit semantics, not a successful verification badge or an input error.

The stage chapters define [input](spec/01-input.md), [P0](spec/00-class.md) and
[P2 execution/checking](spec/02-flow.md). PIPELINE_DESIGN remains authoritative
for ownership, consumers, failure preservation and the later product chain.
