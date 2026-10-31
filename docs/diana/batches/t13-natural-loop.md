# T13 natural loop adapter

Goal: recover an explicit natural while AST for the admitted reducible loop
schema, preserving every original block, edge and copy. Contracts: TASKS T13 and
ALGORITHMS normal-graph/representation rules. StructuredThenDispatch and
DispatchOnly execute the same certified inputs and independent expectations;
unsupported schemas keep the verified ordinary dispatcher.

P7 builds the normal storage graph and iterative RPO/immediate-dominator trees
in forward and reverse directions. Scratch is proportional to nodes/edges, not
an N-by-N dominance matrix. The admitted schema is a jump sequence, one
single-entry header/linear-body loop and one returning exit sequence. Header
and latch dominance and exit post-dominance are discharged before reducing the
header/body into a NaturalLoop. No graph, SSA, CopyPlan, result order or effects
are rewritten or sorted.

A separate validator seals RegionCoverage only after checking disjoint complete
leaf coverage, entry, each sequence transfer and original enter/leave/latch
EdgeIds. A failed certificate returns InternalInvariant rather than falling back.
An unmatched schema alone returns the existing checked dispatcher. P9 consumes
the same owned storage statements and copies: header operations stay inside
while, chosen exit copies precede break and latch copies precede continue. A
simple loop therefore has an explicit natural AST, not pc/switch text disguised
as a loop.

The additive Engine::with_representation API selects the adapter without changing
the existing Options construction contract or Java/recovery eligibility. The
public development example transports the choice; the existing thin E2E runner
can choose it for actual core execution without changing case inputs or expected
values. Existing diamond recognition remains the natural diamond producer.

```sh
python3 tools/e2e.py run --case D002 --case D003 --case D004 --case D008 --case D001
python3 tools/e2e.py run --representation dispatch_only --case D001 --case D002 --case D003 --case D004 --case D008
python3 tools/e2e.py run
```

On macOS arm64, `run-kc4ucap7` and final `run-16gwvhoa` each pass all five
selected cases and 13 invocations. D002/D003 default actual sources contain a
natural while with no dispatcher; DispatchOnly actual sources contain the
original pc/switch form, including D001. Both modes pass real compilation and
the independent Drivers' zero/several iteration and parallel-copy observations.
D004/D008 currently retain the dispatcher for their more complex shape. The
existing public core two-entry-cycle test also still verifies that an
irreducible graph keeps the dispatcher and never observes DA defaults.

Supplemental authorized input is under `.cache/e2e/t13-natural-loop/`. Locked
javac/D8 produce a loop whose predicate and body log every evaluation, with a
body failure option. Structured and dispatch actual methods both compile and
match the independent reference JVM for zero iterations, three iterations and
escaping fixtures.Failure at the second body call. Header/body effect order and
the failing call's absent backedge copies are preserved. Original/actual sources,
compiler logs and the three comparisons remain separate in observations.json;
these are not extra fixed-case passes or auxiliary-tool tests.

```sh
target/release/examples/inspect_class .cache/e2e/t13-natural-loop/dex/classes.dex 0 'Lfixtures/Case;' f '(I)I' --catalog .cache/e2e/t13-natural-loop/catalog.json --output .cache/e2e/t13-natural-loop/structured.json
target/release/examples/inspect_class .cache/e2e/t13-natural-loop/dex/classes.dex 0 'Lfixtures/Case;' f '(I)I' --catalog .cache/e2e/t13-natural-loop/catalog.json --control .cache/e2e/t13-natural-loop/dispatch.controls.json --output .cache/e2e/t13-natural-loop/dispatch.json
```

The complete final `.cache/e2e/engine/run-9kw1nuw6/report.json` retains 86
mandatory cases, 177 invocations and 181 seams: 42 whole cases and 96 seams
passed, with no previously passing seam lost. The full command exits 1 for the
remaining real failures. Workspace fmt, all-target/all-feature Clippy with
warnings denied, locked release build, all 117 cargo-nextest core tests with zero
skips, diff and sensitive-input checks passed. Existing core coverage was reused;
no new tests, auxiliary-tool tests, fixed expectations, fingerprints or TASKS.md
changes were introduced.

T13 remains pending for the required multi-exit/conditional and nested natural
regions and remaining coverage obligations. Next action: extend the sealed
region profile while preserving exact edge-specific exit/continue copies.
