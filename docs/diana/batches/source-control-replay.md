# Final source-control replay

Goal: check the final P8 source-control plan against checked operations and P7
regions before sealing. Baseline: 60066ab. Contracts: DESIGN §4 and ALGORITHMS
§6.3–§6.5/§10/§11. Preparation reuses accepted B01–B04 and unchanged draft-7
inputs on macOS arm64.

P7 produces regions from P6 storage. P8 lowers them and independently checks the
resulting lexical transfers, original block/edge coverage and synchronized
operands. P9 consumes the sealed result. Existing natural-loop, protected and
monitor fixtures and their independent Drivers cover the unchanged behavior;
fixed mutation counterexamples exercise the final replay obligation.

## Execution

`p8_java::source_replay::verify` derives lexical successors from the final source
steps independently of CFG target annotations. `Builder::finish` calls it before
constructing `Plan`. The replay compares statements, branch arms, CopyPlans,
normal successors and completions with the already replayed dispatcher operations.
It checks required block coverage against P7 and rejects orphan source events.
Synchronized acquisition operands and enclosed block membership are compared with
the P7 monitor region. P7's existing `protection::blocks` traversal is shared as a
read-only query. Try steps retain the original P7 node identity; P9 prints the
already selected `try` syntax without interpreting that identity.

The replay also checks final try/catch block membership, catch order, handler
entry and source type alternatives against P7 and the P8 source-name plan. The
existing P7 and dispatcher verifiers remain responsible for original exceptional
edges, catch-type evidence, cleanup ownership and effects.

`src/p8_java/source_control_tests.rs` crosses the existing input, ordinary pipeline,
region and Java verification modules. It reuses the T13 fixed loop instructions
and `tests/fixtures/monitor-scopes/d036.dex` plus the ordered catches in
`tests/fixtures/throw-state/classes.dex`, with their catalogs. D036's independent
Driver checks that the body observes the held monitor. Fixed mutations cover
wrong break/continue, lost statements/copies, wrong/extra completion, a wrong lock
operand, premature monitor closure, changed catches/try coverage, empty loops and
duplicate nested loop labels. Disabling only the new replay call
reproduced acceptance of the wrong break; the regression failed as intended.
The replay call was restored before final validation.

The new replay charges each traversal and uses fallible, budgeted allocations for
lexical maps, coverage and membership. Its bounded lexical walk rejects cycles
without a semantic event. The same counterexamples check zero-work,
zero-allocation and cancellation through the new producer path. No numeric default
limits were added.

Observed on macOS arm64 with unchanged draft-7 preparation:

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`:
  passed.
- `cargo build --workspace --release --locked`: passed.
- `cargo nextest run --workspace --all-features --locked`: final integration ran
  157 tests, all passed with none skipped.
- `python3 tools/e2e.py run`: 70/86 cases and 143/181 seams passed; exit 1 for
  outstanding required cases. All 2,379 member observations and complete public
  reports matched the preceding accepted implementation run. Reproduce by comparing
  matching draft-7 invocation `public.json` reports at baseline 60066ab and this
  change.
- After strengthening monitor-body membership, run
  `python3 tools/e2e.py run --case D036 --case D037 --case D038 --case D039 --case D040 --case D081 --case D082`:
  all 11 selected invocations passed, including the required classified D081
  outcome. All 15 member observations matched the baseline public reports.
- `git diff --check` and
  `python3 tools/audit/desensitize_check.py --range HEAD`: passed.

The companion `phase-products-hardening.md` records numeric legalization,
visibility/classified-outcome changes and the blocked constructor replacement.
Overall EngineCoreRC acceptance remains open.
