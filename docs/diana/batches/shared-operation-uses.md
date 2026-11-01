# Shared operation identities and operands

Goal: replace the branch-specific bound source/right operand and separate call
arguments with one operation identity and ordered definition-use representation.
Contracts: DESIGN §3.3 and §5; ALGORITHMS §2–§4.

The normal-flow profile and temporary routes retain the boundaries recorded in
[the baseline](architecture-baseline.md). This batch extends the production chain
from [the extraction](shared-flow-ssa.md), with existing Java validators as real
consumers before array migration.

## Production changes

`p2_flow/normal.rs` assigns an OpId and original PC to each semantic operation.
Its read iterator preserves operand order, including the complete borrowed call
word list. P3 binds every input before committing the normal result.

`p3_ssa/normal.rs` now owns one operation arena with ordered ValueId inputs.
Definitions are Parameter, OpResult(OpId) or Phi. Blocks reference operations by
identity; the call table references those same operations. The separate
BoundOperand, source/right fields, BoundCall and copied call arguments are removed.
The SSA checker replays input bindings and edge environments, checks operation
identity/site/kind/result correspondence, and retains the independent original
invoke/reference/result-adjacency checks. Bad/grounded checks include all inputs,
including calls with unused results.

P5 reads producer facts and move/arithmetic inputs from this arena. P8 assignment,
call and dispatcher checks use the same operations; null literal evidence follows
the original constant operation rather than a duplicated definition expression.
Existing Phi, EdgeId and parallel-copy consumers retain their identities.
Arena growth uses charged fallible allocation; read, bind, graph, availability and
type loops charge work and check cancellation through the shared MethodBudget.

## Evidence

Platform: macOS arm64. No test functions were added; the existing public SSA,
undefined join, protected-method, copy and raw/resource regressions remain active.

- Fmt, strict locked workspace/all-target/all-feature Clippy, locked release build
  and cargo-nextest passed. All 118 native tests ran with zero skips.
- Full `python3 tools/e2e.py run` produced
  `.cache/e2e/engine/run-aent22bd/report.json`: 86 cases, 177 invocations and
  181 seams selected, with 50 cases/116 seams passed. The overall result remains
  failed. Comparing passing identities against `run-bi7d0lc8` found no lost or
  additional passing seam.
- Dispatch-only D001–D008, D061 and D083 passed all 10 cases/24 seams in
  `run-77q757c9`. Both adapters consume the shared products.
- The E2E runs include the frozen static-field worktree changes. An isolated
  checkout of `89a3b01` with only this staged Rust patch passed fmt, strict Clippy,
  all 118 native tests and release build. It reused the existing certified D065
  container; the frozen field changes are not dependencies of this contribution.

Commands, logs and the isolation patch are under
`.cache/e2e/shared-operation-uses/`; sensitive-input and staged diff checks passed.

The shared producer/consumer milestone is ready for the first array vertical
slice. Current admission still has the extracted normal-profile restrictions;
static binding is still early, storage assembly remains in P8, and the temporary
whole-method routes remain listed in the baseline. These remain later migration
obligations, not restored architecture claims.
