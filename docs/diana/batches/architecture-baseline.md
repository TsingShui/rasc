# Shared pipeline architecture baseline

Goal: identify the current whole-method routes, their duplicated responsibilities
and their migration destinations under DESIGN §3.3–§5 and ALGORITHMS §2–§6, §9.
This batch inspects the existing implementation and stored evidence.

## Working tree and evidence

The inspected HEAD is `01b8123`. The existing static-field changes remain frozen
in `p4_calls/{fields,mod}.rs`, `pipeline/heap_access.rs`,
`p8_java/heap_access.rs` and `p9_emit/heap_access.rs`, relative to
`crates/diana-decomp/src/`. The untracked `t15-static-fields.md` remains a draft.
The diff adds static/instance field distinctions, sget/sput handling, source
qualifiers and corresponding replay checks; it also removes the heap producer's
array/nominal-parameter admission condition. These changes remain uncommitted.

Stored `.cache/e2e/t15-static-fields/observations.json` contains ten matching
actual/reference JVM observations, including initialization failure and final
nonconstant reads. Its `nextest.log` records 118 passed and zero skipped tests.
These are stored observations, not a new execution or completed T15 acceptance.
The interrupted `full-e2e.log` ends during D072 and records no completed report.

The last confirmed complete report is
`.cache/e2e/engine/run-bi7d0lc8/report.json`: draft-7, 86 selected cases,
177 invocations and 181 seams, with 50 cases and 116 seams passed. Its overall
execution is `failed`. This remains behavioral evidence at its recorded revision;
it does not establish architecture compliance for the frozen working tree.

## Phase ownership and current gaps

Paths below are relative to `crates/diana-decomp/src/`. The owning contracts remain
DESIGN §4 and the cited ALGORITHMS sections; this table records implementation gaps.

| Contract | Observed implementation | Migration destination |
|---|---|---|
| P2 FlowBody, ALGORITHMS §2 | Each producer decodes its own closed vocabulary. `branch::flow` already resolves static contracts during decoding. There is no common operation/effect product for arrays, calls and numeric operations. | One decoded operation representation with original sites/references, read/write slots and explicit effects; declaration constraints follow SSA. |
| P3 SsaBody, §3 | `branch::build` has fixed entry Phi, EdgeId and bound uses. Scalar, heap and call-chain producers have separate ValueId/register tables; casts, receiver and fill producers track aliases separately. | Reuse branch CFG/SSA construction and replay, with shared definitions and uses. Preserve width/state distinctions rather than importing a second tracker. |
| P4 ResolvedBody, §9 | P4 helpers provide useful sealed contracts, but whole-method producers select and consume them before a shared type stage. Engine preflight separately checks calls, static reads and fill shape. | Collect original declaration evidence and unresolved obligations over the common SSA; preserve hard-error precedence without treating missing evidence as a guessed binding. |
| P5 TypedBody, §4 | Scalar has a dedicated solver; branch's `VerifiedSsa` carries kinds, integral bits and ranges; heap/call/cast producers maintain their own type vectors. | One typed product with final per-use conversions before storage; final source binding remains a P8 check. |
| P6 StorageBody, §5 | Parallel-copy helpers are reusable, but `p8_java/dispatcher.rs` assembles dispatcher locals/copies. FreshArray and allocation have dedicated proof code. | Common typed storage and CopyPlan; retain dedicated allocation/fill proofs without method-level entry paths. |
| P7 RegionPlan, §6 | Natural loops already retain the dispatcher storage in `StructuredLoop`; this sharing is useful but limited to the branch route. | Natural structure and dispatcher adapt the same common storage, without changing CFG/effects. |
| P8 verification, §6/§9 | `engine.rs` tries nine whole-method producer/verifier pairs and emits immediately after a match. Each route has a separate replay/Java body. | One phase chain and method validator; reuse independent original-fact checks as operation/storage obligations. |
| Classified raw exit, §7.2 | Routes eventually return `NormalMember::Raw` or a classified error to the engine. Java eligibility and preflight checks are distributed before the branch route. | Retain the engine's read-only raw handling; remove method-route selection after behavioral parity. |

## Temporary route boundaries

These are necessary admission restrictions observed in code, not exhaustive support
claims. All routes remain temporary migration sources. Their existing rejection
behavior remains evidence to preserve, not a reason to widen support by guessing.

| `pipeline/` route | Observed boundary | Migration ownership |
|---|---|---|
| `scalar.rs` | Static, no handlers; closed straight-line numeric/String vocabulary and separate word binding/type solving. | Common P2/P3 operations; existing numeric semantics in P5/P8. |
| `branch.rs` | Static, no handlers; limited declaration types and opcodes. Array descriptors/operations are absent. Straight-line admission also depends on calls/narrow types. | Common CFG/SSA foundation; remove shape-dependent admission as operations migrate. |
| `heap_access.rs` | Static, no handlers; int/void result, int/Object/String/int-array/Object-array parameters plus a single nominal descriptor. Branch opcodes are absent. | Arrays first through shared P2–P8; field evidence through P4/P5/P8. |
| `receiver.rs` | Int result; one nominal static parameter or parameterless initialized instance receiver; no handlers, separate aliases. | Shared receiver uses, binding obligations and initialization availability. |
| `call_chain.rs` | Static, no parameters/handlers; closed construction/call/value chain with separate slots and types. | Common invoke/result atoms and SSA; P4/P8 binding, P6 allocation storage. |
| `constructor.rs` | Real constructor, int parameters, no handlers; dedicated UninitThis slots and prologue/body recognition. | Shared analysis plus P4 initialization constraints, P6 deferred storage and P8 first-statement validation. |
| `casts.rs` | Static void, no handlers; Object/String parameter aliases and explicit casts. | Shared normal-result definitions, P5 refinement and P8 original-site checks. |
| `zero_fill.rs` | Static void, one array parameter, no handlers; separate alias/fill scan. | P5 component/width evidence and P8 LengthOnly at the original effect site. |
| `fresh_array.rs` | Static, no parameters/handlers, primitive-array result; whole-method stage/slot machine. | Common allocation/fill operations; retain P6 FreshArray proof and P8 exact expansion. |

The branch-plus-array and loop-plus-array gaps follow from these disjoint
vocabularies. This batch has not executed a new composition counterexample.
Historical T06–T15 behavior evidence therefore remains scoped to executed inputs;
affected architectural acceptance requires revalidation after migration. In
particular, historical T13/T14 completion does not establish a common pipeline.

## Verification and next slice

Commands: inspect `git status --short`, `git diff --stat`, the five frozen-file
diffs, the report fields and the scoped static-field logs; read the contracts and
producer/consumer interfaces cited above. No production functions changed.
`git diff --check`, `cargo fmt --all -- --check` and
`python3 tools/audit/desensitize_check.py --range HEAD` passed on macOS arm64.
Clippy, release build, nextest and core E2E were not rerun for this inspection-only
batch; the stored results above retain their original scope.

Next slice: extract the existing branch flow/SSA responsibilities into real shared
products and consumers, then connect array load/store/length to that chain. Keep
EdgeId, Phi and original effect placement; verify branch/loop composition against
independent core observations and retain existing Java results during migration.
