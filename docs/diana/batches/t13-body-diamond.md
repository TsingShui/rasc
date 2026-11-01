# T13 body diamond and shared latch

Goal: reduce D004's conditional-continuation diamond without deleting the shared
increment/latch or its original copies. Reuse the verified normal storage AST,
RPO/dominance and disjoint leaf certificate. Compare both native adapters through
the existing core cases and independent JVM behavior.

Body branches now have mutually exclusive Exit or Diamond plans. A diamond owns
zero/one original jump block in each arm and records both entry EdgeIds, each
present arm's outgoing EdgeId and the common join. The producer checks header
dominance, join post-dominance and distinct leaves before collapsing the branch
and arms. The shared join stays in the enclosing sequence. Its increment and
latch copies execute after either arm, rather than disappearing through visitation.

The independent coverage validator checks every arm target and outgoing transfer
against the owned normal storage, along with disjoint complete leaves and the
loop boundary. P9 prints original branch copies before arm statements, arm copies
before the join and latch copies before continue. CFG, SSA and CopyPlan stay
unchanged. A failed certificate remains InternalInvariant; an unmatched graph
keeps the existing legal dispatcher.

```sh
python3 tools/e2e.py run --case D004 --case D008 --case D001 --case D002 --case D003 --case D005
python3 tools/e2e.py run --representation dispatch_only --case D004 --case D008 --case D001 --case D002 --case D003 --case D005
```

On macOS arm64, `run-_8ees_3z` and `run-0so77gnr` each pass the six selected
fixed cases and 14 invocations, including real javac/Driver comparisons. D004 now
has a natural while/body diamond without pc/switch. Actual D002/D003/D008 sources
also retain their natural loops. Fixed inputs and independent expectations are
unchanged.

Supplemental authorized core sources in `.cache/e2e/t13-body-diamond/` log a
predicate each iteration, skip even indices through conditional continue and
perform an int-returning effect at odd indices. A helper throws at index 3 when
requested. Locked javac/D8 inputs reuse unchanged declaration facts. Original and
both actual sources compile and match independent expectations and reference JVM:

- zero iterations: value=0; H:0;
- normal: value=2; H:4,H:3,B:1,H:2,H:1,B:3,H:0;
- failure: fixtures.Failure; H:4,H:3,B:1,H:2,H:1,B:3.

The repeated predicates show that continue retains the shared increment, while
the absent final predicate on failure preserves abrupt completion. Compiler logs,
separate original/actual sources and observations.json retain these supplemental
comparisons; they do not add fixed-case passes or test auxiliary tools.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff and sensitive-input
checks passed. This batch ran the affected E2E set. The last complete mandatory
report remains `run-9kw1nuw6` from the natural-loop batch. TASKS.md, fixed
expectations and required denominators are unchanged.

T13 remains pending for nested-loop evidence and remaining fixed coverage
obligations. Next action: inspect the required nested profile and extend the
bounded region producer without duplicating effects.
