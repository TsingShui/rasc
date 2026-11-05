# 00 — Product and engine boundary

Diana is a native Rust APK/DEX decompiler. The application in `apps/diana/` owns
files, APK/ZIP handling, selection and output. The engine in
`crates/diana-decomp/` borrows DEX bytes and derives facts from that input.
The GUI package is a native UI demo with authored display content, not an Engine
consumer. The web directory remains a documentation placeholder outside the workspace.

The approved architecture and complete product chain are defined in
[PIPELINE_DESIGN](../PIPELINE_DESIGN.md). The current implementation and scheduling
entry are described in [phases](../phases.md) and the
[engine README](../../../crates/diana-decomp/README.md). Design contracts do not
establish that later stages are implemented.

## Product obligations

- Diana MUST use the analyzed input as the only source of decompilation facts.
  It MUST NOT require external declarations, classpaths, `android.jar`, symbol
  databases or network facts.
- Diana MUST preserve original member identities, instruction/argument/effect
  order, numeric bits, exception selection, lock and initialization behavior.
  Missing facts remain explicit uncertainty, not invented values or declarations.
- Default retrieval MUST expose available readable code and original facts.
  A local failure MUST NOT hide independently recoverable fragments within the
  same method or other members. Java legality and compilability are not output gates.
- Text and JSON MUST present the same underlying result. No generated dispatcher,
  fabricated exception body or second analysis/retry route is permitted.
- The engine MUST remain native Rust without unsafe code or external runtime/
  process dependencies. Input bytes are borrowed; files and ZIPs remain
  application-owned.

P0, the method reader, P1, P2 and P3 currently deliver analysis facts through Engine.
The final method/class presentation and rendering interfaces are not yet active;
old `Engine::decompile`, `Report`, JavaUnit/Mixed schemas and P8 renderer signatures
are not current APIs. Application command descriptions belong to the
[CLI README](../../../apps/diana/README.md), not a duplicate engine spec.

## Failure kinds and fact states

| Kind | Meaning |
|---|---|
| `Malformed` | A known input-format or semantic constraint is violated. |
| `Unsupported` | A format or required capability is not supported; this does not prove that the input is invalid. |
| `ResourceExhausted` | Actual allocation or platform-capacity failure. |
| `InternalInvariant` | The implementation violates its own product or construction invariant. |

A failure kind is not a presentation status. `Unavailable` describes a fact or
result that could not be obtained; it MUST retain its reason. `Unknown` describes
an interpretation not established by available facts and MUST NOT replace a
recorded read error. `Absent` means confirmed nonexistence; `NotRead` means the
producer did not read that fact. Complete directory coverage does not imply that
every member declaration or method body is valid.

P1–P9 MUST publish coverage, established facts and local unavailability as defined
by [the common product model](../PIPELINE_DESIGN.md#41-成熟度与增量) and
[stage-by-stage consumption](../PIPELINE_DESIGN.md#92-p1p9-统一消费局部结果).
A complete availability description does not claim that every item was recovered.
Established facts retain their guarantees; missing items retain their reasons and
impact. A construction snapshot is not such a published product.

Local fact failure MUST NOT automatically stop a stage. Stage stops are reserved
for an inability to maintain the minimum product guarantees or retain results.
Stage stops MUST retain original identity, concrete reasons and available facts.
They MUST NOT promote an incomplete product to success. Failure kind alone MUST
NOT determine stop scope. All error handling MUST follow
[PIPELINE_DESIGN §9](../PIPELINE_DESIGN.md#9-局部失败与方法退出): preserve independent
fragments and results, and widen a stop only for an actual shared dependency or
resource failure. Product guarantees and read states remain governed by
[§4.5](../PIPELINE_DESIGN.md#45-独立读取状态阶段停止与诊断) and the outcome contracts
in [02-flow](02-flow.md) and [03-ssa](03-ssa.md).

## Safety and evidence

Shared readers MUST check ranges, counts and arithmetic before slicing or
allocating. Derived container growth MUST be fallible and preserve classified
errors. Long work MUST have a finite traversal measure. The CLI/host owns process
termination; request cancellation callbacks and tokens are not part of the engine
API.
There are no numeric work, allocation or output quotas and no successful silent
truncation. Original input facts, platform capacity and bounded queues are not
optional safety checks.

Build/lint success and finite corpus observations do not prove semantic
correctness. Independent stage-owned translation checks compile only in tests;
normal decompilation does not run them. Their results establish only their stated
obligations and semantic model. Reference use and evidence limitations are owned
by [SOURCE_MAP](../SOURCE_MAP.md); contribution and testing rules are owned by
[CONTRIBUTING](../CONTRIBUTING.md) and the root AGENTS.md.
