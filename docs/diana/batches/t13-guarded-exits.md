# T13 guarded loop exits

Goal: recover D008's conditional break with exact original exit/latch copies.
Contracts: TASKS T13 and ALGORITHMS normal-region rules. The common exit sequence
is emitted once; a guard does not delete its other continuation or duplicate
effects. Both representation adapters retain independent core behavior checks.

The planner now admits conditional body exits to the common tail and optionally
one original exit-only jump block per guard. Dominance and disjoint coverage
include that block; the coverage validator rechecks its entry EdgeId, target and
outgoing transfer. The writer emits branch copies, exit-block statements and its
outgoing copies before break. The other arm continues along the original body
and executes latch copies before continue. An invalid certificate remains a hard
InternalInvariant; only an unmatched schema retains the checked dispatcher.

The first supplemental observation caught an exit-stub mismatch: the source was
still a dispatcher, so the natural-representation assertion failed. That result
was not counted as structured evidence. The corrected profile preserves the
separate stub instead of dropping it or claiming the fallback as natural Java.

```sh
python3 tools/e2e.py run --case D008 --case D002 --case D003 --case D004
python3 tools/e2e.py run --representation dispatch_only --case D008 --case D002 --case D003 --case D004
```

On macOS arm64, final `run-bk5jr7g4` and `run-gx8z23wd` each pass all four
selected fixed cases and 11 invocations through actual compilation and independent
JVM Drivers. D008 now contains a natural while with two conditional breaks and
no pc/switch. D002/D003 keep their natural loops. D004's conditional-continuation
diamond still uses the legal dispatcher.

Supplemental core sources are in `.cache/e2e/t13-guarded-exits/`. The locked
javac/D8 recipe reuses the previous effect-loop helpers and unchanged declaration
facts, with an original guarded break at n=1. Both actual adapter sources compile
and match independently authored expectations and the reference JVM for:

- zero iterations: value=0 and H:0;
- guard exit after two iterations: value=1 and H:3,B:3,H:2,B:2,H:1;
- body failure: fixtures.Failure and H:3,B:3,H:2,B:2.

The guard prevents B:1 from executing; the escaping failure prevents later header
work. Separate original/actual sources, compilation logs and observations.json
retain the comparisons. These are supplemental core observations, not extra
fixed-case passes or auxiliary-tool tests.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff and sensitive-input
checks passed. This batch ran the affected E2E set; the last complete mandatory
report remains `run-9kw1nuw6` from the preceding batch. TASKS, fixed expectations
and input denominators are unchanged.

T13 remains pending for conditional-continuation diamonds, nested natural regions
and remaining fixed coverage obligations. Next action: preserve the shared latch
when reducing D004's body diamond.
