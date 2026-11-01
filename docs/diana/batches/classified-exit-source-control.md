# Classified exits and explicit source control

Goal: remove the post-analysis declaration scan and move remaining lexical
control/monitor lowering from P9 into P8. Baseline: b55e9d4. Contracts: DESIGN §4;
ALGORITHMS §6.3–§6.5/§10/§11; TASKS §1/T13/T18/T19. Preparation reuses accepted
B01–B04 and the unchanged draft-7 inputs on macOS arm64. The operator authorized
the remaining A1/A3 architecture corrections.

P2/P4 own decoded operations and declaration evidence. A classified ordinary
analysis failure proceeds directly to the read-only original-output exit without
another whole-method decoder or declaration pass. P7 still owns regions over P6
storage and CopyPlans. P8 owns explicit lexical transfers and synchronized lock
operands; P9 renders those checked source products.

Existing public branch/binding/raw-integrity and natural/protected/monitor cases
provide independent source-JVM and classified-failure expectations. Changes to
classification, member availability, source or budgets require explicit review;
a new passing total alone does not establish preservation.

## Execution

`pipeline::diagnostics::unsupported_method` and its external-field rescan helper
are removed. `p4_calls::normal::collect` retains missing external signature
identity/PC in `ResolvedBody`; `pipeline::branch::Analysis` carries that evidence
through a later declined analysis. Earlier phase failures keep their own reason
without inspecting later instructions. Existing P4 field evidence still owns
constantness and missing-field failures.

`p8_java::{natural_control,protected_control}` lower checked P7 regions and the
replayed dispatcher operations into a sealed `source_control::Plan`. The plan
records lexical transfers, condition/statement/CopyPlan references, catch source
names and synchronized operands. Lock selection checks the original acquisition
operation against the P7 monitor scope. The builder checks lexical scopes and
rejects duplicate statement/edge ownership. `dispatcher_verify::edge` also checks
the original source block. P9 consumes the plan without selecting control targets
or scanning monitor statements.

The builders charge work and fallible vector/string allocation through the method
budget. Source plans coexist with the checked operations until rendering; explicit
limits can therefore reject additional intermediate storage. Cancellation and the
single classified raw exit retain their existing contracts.

Existing coverage: `crates/diana-decomp/tests/pipeline/{ssa,conditions,reservations}.rs`,
`src/p7_regions/monitor_tests.rs`, `src/p8_java/protection_tests.rs` and the tracked
fixtures under `crates/diana-decomp/tests/fixtures/`. The call-definition regression
now also checks complete raw output and the original missing-binding reference/PC.
Core E2E inputs and independent Drivers remain those of draft-7, including
D007/D013/D029–D040/D080–D082.

Commands and observed results on macOS arm64:

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`:
  passed.
- `cargo build --workspace --release --locked`: passed.
- `cargo nextest run --workspace --all-features --locked`: 155 passed, none skipped.
- `python3 tools/e2e.py run`: 70/86 cases and 143/181 seams passed; exit 1 for
  outstanding required cases. The final run follows the sealing/source-edge check.
- Compare per-invocation public reports with baseline b55e9d4: all 2,379 original
  member observations and their complete public reports are unchanged, including
  identities, status, source and diagnostics. Reproduce by running the fixed
  draft-7 E2E set at each revision and comparing matching invocation `public.json`
  reports. Execution used the pinned Temurin 21.0.8+9 Java8 target evidence.
- `git diff --check` and
  `python3 tools/audit/desensitize_check.py --range HEAD`: passed.

A first full E2E attempt was interrupted by the command timeout; it is not counted
as acceptance. Subsequent full runs completed with the results above. T20 final
assignment obligations and overall EngineCoreRC acceptance remain open.
