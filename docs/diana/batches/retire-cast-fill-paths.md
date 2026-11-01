# Retire cast and fill method paths

This batch removes the migrated cast/empty-fill/FreshArray method builders,
parallel trackers and AST variants. Contracts: [DESIGN](../DESIGN.md),
[ALGORITHMS](../ALGORITHMS.md), and the shared
[cast](shared-casts.md), [empty-fill](shared-empty-fill.md) and
[FreshArray](shared-fresh-array.md) evidence.

The engine no longer attempts these three method builders after shared flow.
Their pipeline parsers/register trackers, P6 legacy FreshArray replay, P8 method
verifiers and P9 method writers are deleted. The shared per-operation producers,
LengthOnly/FreshArray proofs and storage consumers remain. The exact element
formatter is extracted unchanged into array_literals.rs for its shared consumer;
it is not a new recovery path.

No tests, fixtures, Drivers or expected values were authored or changed. Related
fixed `run-zdfl__k3` passes D025/D027/D073/D074: 4 cases, 11 invocations and
11 seams. The prior independent method-context observations remain scoped to
the unchanged shared producers and formatter.

Format, strict Clippy, release and cargo-nextest passed: 122 tests, zero skips.
The staged patch also passed these Rust gates in a detached worktree without the
frozen static-field changes, reusing the certified D065 fixture. Diff and
sensitive-input checks passed before submission. Artifacts are under
`.cache/e2e/retire-cast-fill-paths/`.

Not run: full fixed E2E after this deletion slice; final reacceptance retains that
obligation. Scalar vocabulary parity, other temporary routes and the unified raw
exit remain in the current deletion task. The existing fill-shape fallback
classifier also remains pending that consolidation. The frozen static-field
files remain uncommitted and unchanged by this deletion.
