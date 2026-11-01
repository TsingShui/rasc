# Shared flow and SSA extraction

Goal: make the existing branch decoder, fixed-entry SSA construction and type
closure separate production stages consumed by the current Java validators.
Contracts: DESIGN §3.3–§4; ALGORITHMS §2–§4 and §6.

The first extraction preserves the current normal-flow vocabulary and its
handler restriction. Static-call evidence is still collected by the decoder
until the call migration; the other whole-method routes remain as listed in
[the baseline](architecture-baseline.md). Array operations enter this shared
chain in the following vertical slice.

## Implemented slice

- `p2_flow/normal.rs::decode` owns the existing normal-flow decoding, block/edge
  construction and invoke/result sites. `FlowBody` has a private constructor
  marker. `Op::effects` supplies conservative call effects; the P8 diamond
  adapter consumes them instead of admitting an effectful body.
- `p3_ssa/normal.rs::build` consumes FlowBody, assigns definitions and fixed entry
  Phi, connects normal edges and independently replays bound uses/transfers.
  `verify_availability` closes bad/grounded facts and checks both ordinary and
  call operands before the sealed SsaBody reaches typing.
- `p5_types/normal.rs::solve` owns the existing kind/integral/range closure.
  TypedBody owns the verified SsaBody; it does not regenerate IDs or maintain a
  parallel register table. The P8 diamond and dispatcher validators consume it.
- `pipeline/branch.rs::build` now schedules these concrete stages. The shared
  charged `substrate::owned::push` reserves before growth. Decode, graph, SSA,
  availability, type and effect scans retain work/cancellation checkpoints.

The bound operation vocabulary still has the old source/right-operand layout and
separate call arguments. Unifying those uses for array stores remains part of
`shared-flow-ssa`; this extraction alone does not finish that milestone. Static
binding remains early, P8 still assembles storage, and the other whole-method
routes remain migration sources rather than accepted final architecture.

## Verification

Platform: macOS arm64. Existing public SSA/undefined-join, protected-method,
parallel-copy and dispatcher regressions provide the native coverage; no test
functions were added.

- `cargo fmt --all -- --check`, strict workspace/all-target/all-feature locked
  Clippy, locked release build and `cargo nextest run --workspace --all-features
  --locked` passed; nextest ran 118 tests with zero skips.
- `python3 tools/e2e.py run` produced
  `.cache/e2e/engine/run-plb4lsxn/report.json`: full draft-7 selection, 86 cases,
  177 invocations and 181 seams; 50 cases/116 seams passed. The overall run failed
  for the remaining obligations. Comparing passing seam identities with
  `run-bi7d0lc8` found no lost or additional pass.
- After adding the final P5 per-definition work charge, the related default run
  selected D001–D008, D055, D057–D059, D061, D071, D077 and D083:
  `run-v0_6qhji` passed all 16 cases/37 seams. The dispatch-only run selected
  D001–D008, D061 and D083: `run-nwdm781g` passed all 10 cases/24 seams.
- These E2E executions include the frozen static-field working-tree changes.
  An isolated checkout of `9a58b15` with only this staged Rust patch passed fmt,
  strict Clippy, release build and all 118 native tests without those changes.
  The first isolated nextest attempt failed because D065 was absent; copying the
  existing certified `container.dex` enabled the successful rerun.

Logs and the isolated patch are under `.cache/e2e/shared-flow-ssa/`. The frozen
static-field files remain separate from this contribution. Next: replace the
limited bound-use layout with shared operation operands and connect the existing
array operations through these producers and consumers.
