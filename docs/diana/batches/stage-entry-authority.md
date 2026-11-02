# Stage entry authority

Goal: preserve public decompilation results while preventing production phase
implementations from starting another phase's analysis through existing entries.
Baseline: c7fa1e1. Contracts: DESIGN §4.1 and the root architecture guardrails;
ALGORITHMS §3, §9.2 and §12 govern the unchanged SSA, initialization and Java proofs.
Preparation reuses accepted B01–B04 and unchanged draft-7 inputs/tools.

The engine and pipeline modules schedule the ordinary P2–P8 chain and the scoped
constructor dependency queries. P2–P8 own their algorithms and immutable products.
Their inputs retain existing checkedness, original identity and lifetime; entry
initialization facts remain partial and never certify a callee body. The selected
method/request control retains existing classified errors, raw fallback resources
and cancellation propagation. A scheduling credential grants entry authority,
not validation or recovery capability.

This batch restricts existing construction entries and removes scheduler aliases
for phase-owned vocabulary. The credential is privately constructed by the engine;
scheduler coordinators pass it only to thin stage wrappers. Private algorithms do
not receive it. Read-only queries and independent replay retain their inputs and
checks. Public library interfaces, independent expected values and effective task
dependencies remain unchanged.

Existing evidence includes all core contracts and the D042 definition/reference/
encoding/return-fault, missing binding, unrelated-owner and cycle counterexamples
in `crates/diana-decomp/tests/pipeline/initialization.rs`. Mutation/replay cases
remain in their existing test modules. Compile-refusal probes will be temporary
edits to core Rust modules, removed before acceptance; no auxiliary test tooling
or new private-IR golden is introduced.

## Execution

The engine privately constructs `SchedulePermit`. It has no production factory,
Default, Clone or serialization implementation. The ordinary pipeline and
constructor coordinator borrow it; each stage wrapper consumes only the borrowed
authority and calls a private algorithm without passing the credential onward.
The credential neither creates a RunContext nor resets a budget.

The guarded entries are ordinary P2 decode, P3 SSA construction, P4 collection,
P5 solving, P6 allocation, P7 planning and P8 method verification; the P2/P3/P4/P5
entry-fact producers and P8 constructor-graph creation; and P8 no-code declaration,
field and class-unit admission. The class-unit wrapper takes EnvironmentId from
the same catalog used by its only production caller, preserving the original
argument value without an extra broad input aggregate.

P2–P6 `normal` modules and P8 method/class/field implementation modules are private.
Their phase interfaces explicitly export the products, bounded read-only queries
and guarded entries used by existing consumers. The pipeline no longer re-exports
phase types or scalar vocabulary. Its consumers name the actual owning phase.
P7 monitor/protection construction and P4/P5 internal constructor helpers are
restricted to their owning phase. Native replay tests retain test-only access to
required setup products; these helpers do not exist in production builds.

Temporary probes in `crates/diana-decomp/src/p9_emit/source_control.rs`, checked
individually with `cargo check -p diana-decomp --lib --locked`, established:

| Attempt | Observed refusal |
|---|---|
| Invoke ordinary P2 decode without a credential | E0061 |
| Call the hidden P2 implementation directly | E0603 |
| Construct a credential outside the engine | E0451 |
| Invoke `SchedulePermit::for_test` in production | E0599 |

All probes were removed before final verification. Existing native tests use the
credential factory only under cfg(test); their corruption, resource and replay
obligations remain unchanged. No new test functions or expected values were added.

Observed on macOS arm64 using unchanged draft-7 inputs and pinned Java8-target tools:

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`:
  passed.
- `cargo build --workspace --release --locked`: passed.
- `cargo nextest run --workspace --all-features --locked`: 159 passed, none skipped.
- `python3 tools/e2e.py run`: 70/86 cases and 143/181 seams passed; exit 1 for
  outstanding required cases. Matching draft-7 invocation `public.json` reports
  at c7fa1e1 and this change contain identical complete public reports and all
  2,379 original member observations, including identity, Java availability,
  source and classified diagnostics. Reproduce the comparison by running that
  command at both revisions with the same certified inputs and environment.
- `python3 tools/e2e.py run --representation dispatch_only --case D001 --case D002 --case D003 --case D004 --case D005 --case D006 --case D007 --case D008 --case D009 --case D010 --case D011 --case D012 --case D013 --case D041 --case D042 --case D044 --case D046 --case D047 --case D082`:
  19/19 selected cases and 41/41 invocations/seams passed.
- `git diff --check` and
  `python3 tools/audit/desensitize_check.py --range HEAD`: passed.

The entry restrictions are compiler-enforced in production. They do not prove
that a newly written algorithm belongs in its phase or replace independent
semantic replay. Contribution review still checks algorithm ownership and any
proposed expansion of interface authority under the root AGENTS rules.
