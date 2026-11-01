# Arrays in the shared pipeline

Goal: migrate existing int/Object array load, store and length operations through
shared flow, SSA, types, storage and Java verification, including branch/loop
composition and original null/bounds/effect order.
Contracts: DESIGN §3.3–§5; ALGORITHMS §2, §3, §4 and §5.

Reuse the fixed D024/D026 obligations and existing related inputs. Supplemental
self-made branch/loop observations remain separate from fixed-case acceptance.
The temporary routes retain the boundaries in
[the baseline](architecture-baseline.md) until behavioral parity permits removal.

## Shared producers and consumers

P2 now represents array load/store/length as operations with original sites and
heap/may-throw effects. Their operands use the common ordered SSA inputs, including
three inputs for store; normal results remain OpResult definitions. The former
branch/call-shape admission condition is removed.

P5 closes array kinds with the existing value facts and creates sealed per-operation
array evidence and lossless index/store conversions. A known int/Object element
mismatch or invalid index/value family is Malformed/InvalidArrayAccessType.
The normal profile currently admits int[] and Object[] descriptors; other declared
array types and nominal parameter types retain their previous support boundaries.

`p6_variables/normal.rs` owns StorageBody, independent locals, temporary identities,
EntrySeed copies and normal-edge CopyPlans. It uses the existing independently
checked parallel-copy planner. P8 now receives this sealed storage and only copies
its declarations/transfers into Java nodes; it does not allocate source storage or
replan Phi copies. Natural structure and dispatcher use this same storage.
P7 still analyzes the existing dispatcher projection; final representation-boundary
consolidation remains a later obligation.

P8 array checks independently compare raw opcode/register correspondence, bound
inputs/results, component types and use conversions. P9 prints materialized reads,
length assignments and stores, including unused operations. No additional runtime
check, helper, call or compound assignment is introduced. The shared route now
precedes the temporary heap route, so the fixed supported array cases use it.
Legacy heap array handling remains for its existing nominal-parameter boundary and
unsupported-opcode classification until member/type migration permits deletion.

Every added operation/read, type/evidence, storage/copy and output loop uses the
shared MethodBudget. Growth reserves fallibly after allocation charging; cancellation
and classified failures propagate to the existing engine boundary.

## Core evidence

Existing coverage was inspected in `tests/pipeline/ssa.rs`, the P6 copy/dispatcher
regressions and D024/D026 before adding coverage. One public-Engine table regression
adds branch reads, loop reads/length, branch stores and malformed element/index
inputs under both representation policies. The existing fixture builder supplies
a concrete array-parameter recipe. The native suite now runs 119 tests, zero skips.
The existing pure P6 interpretation explicitly rejects the new effectful statement
variant; it is not extended into an array execution oracle.

Self-made sources and commands are under `.cache/e2e/shared-arrays/`. Before this
slice, selected `choose` and `sum` returned Debug/JavaAnalysisUnavailable; the
preserved input is `baseline.dex`. After migration, five selected methods produce
Java through the shared route. Default and dispatch-only output each compile with
the locked JDK and match 31 independent reference observations for values,
exception classes and array state. These include overflow, null/empty arrays,
runtime ArrayStoreException, and writes visible before a later bounds failure.
These are method-in-known-context observations, not JavaUnit or extra fixed passes.

## Gates and fixed cases

Platform: macOS arm64/JVM. Fmt, strict locked workspace/all-target/all-feature Clippy,
locked release build, cargo-nextest, diff and sensitive-input checks passed.
An isolated `07f64ca` checkout with only this staged contribution also passed these
Rust gates and all 119 tests using the existing certified D065 container. The frozen
static-field changes remain uncommitted and are not dependencies of this slice.

- Full `python3 tools/e2e.py run`: `run-35_lx6_3`, 86 cases/177 invocations/181 seams,
  51 cases/117 seams passed; overall failed for remaining obligations. No previous
  passing seam was lost. D086 unlimited-default also passes after removal of the
  branch-shape admission condition; the independent expected values are unchanged.
- Dispatch-only D001–D008, D024, D026, D061, D083 and D086: `run-1hv49i43`, all
  13 cases/31 seams passed.
- After removing a redundant copy verification already performed by the planner,
  default D024, D026, D055, D057–D059, D077 and D086: `run-uv8oucm7`, all
  8 cases/13 seams passed.

Logs, reference observations and the isolated patch remain in the batch cache.
Next: move call/member evidence and nominal types into their owning shared stages,
then remove the corresponding temporary trackers and routes after parity.
