# diana-decomp — native engine library

Diana's independent native Rust engine borrows caller-owned DEX bytes. Files,
ZIPs, network access and output ownership belong to the application. Production
forbids unsafe code and external runtime/process dependencies; declarations come
only from the analyzed input.

The active method chain is P0 class facts → method reader → P1 disassembly → P2
execution model → P3 SSA. `Engine::analyze_method` and class requests use one method task.
P4–P9 are not yet active; the application decompilation interface is being rebuilt.
The approved architecture is [PIPELINE_DESIGN](../../docs/diana/PIPELINE_DESIGN.md).

P1 retains complete instruction/payload facts, original references, entry layout
and structural associations. P2 owns ordered operations, exact slots, explicit
normal/exception frame/heap/lock completion rules, transfers, exits and coverage.
It does not expose its complete P1 predecessor. Read-only `MethodEvidence` supports
original inspection and checks without product extraction or construction access.
P2/P3 constructors are internal and require Engine's method-task authority.
P3 privately retains P2 and delivers exact-slot states, definitions, ordered use
bindings, necessary Phis, per-transfer incoming states and original StatePoint
mappings. Borrowed `ExecutionFacts` exposes normal execution queries, not a
predecessor product. [P3's contract](../../docs/diana/spec/03-ssa.md) describes
exception-aware LT dominators, live-demand IDF placement and correspondence checks.
Undefined/invalidated sources remain explicit availability facts, not default values.

A P2 stop retains the complete P1 input, classified error and available unvalidated
construction facts. A P3 stop retains complete P2 input and an unvalidated SSA
snapshot; neither stop can be promoted to a successful product. Class requests retain delivered members and mark request-level
termination explicitly. Range/capacity checks and fallible allocation remain
mandatory. The CLI/host owns process termination; Engine has no request
cancellation callback/token or checkpoint parameter. There are no numeric quotas.

Translation checking compares ordered local symbolic behavior and completion
rules and matches every original transfer/coverage owner. It does not prove ART
admission or concrete external-call behavior. Real-input corpus runs have not
encountered custom/polymorphic invokes or method-handle/type constants. The scoped
`src/dynamic_instructions_test.rs` unit test constructs DEX 038/039 inputs and
checks public Engine products against independently specified references, ordered
slots, result categories and normal/exception completion rules. It also checks
classified stops for invalid versions, references, argument layouts and results.
The same test checks register-renaming relations, equivalent wide-copy/constant
encodings and retention of call/exception effects when a result is discarded.
This is static synthetic-input evidence, not ART execution evidence.
A [Verus trial](proofs/README.md) proves the wide-copy patch predicate called by
production replay, using the same Rust function body. The adapter and remaining
P2 checks are not machine-proved; this is not a proof of whole-method correctness.
Fault-injected allocation execution has not been performed; failure retention is
currently code-reviewed.

Use the single engine inspection entry from the repository root:

```sh
cargo run -p diana-decomp --release --example dbg_shell --locked -- \
  --dex <classes.dex> --exec '{"cmd":"p2","class":0,"method":0,"details":true}'
```

`p3` (`details:true`) observes state positions, bindings, definitions and Phi
incoming records. `p3-all` reports SSA counts, unavailable uses, upstream failures
and explicit incomplete execution on request termination. P3 is not formally
verified; allocation-failure injection and formal single-class performance
measurements remain unperformed.

`p2-all` observes retained execution facts from the same Engine chain and reports
classified outcomes and encountered opcode variants. `roundtrip-all` checks independent byte re-encoding over retained
P1 evidence. No phase-specific debug binary is required.

Applicable engine gates:

```sh
cargo fmt --all -- --check
cargo clippy -p diana-decomp --all-targets --all-features --locked -- -D warnings
cargo build -p diana-decomp --release --locked
cargo nextest run -p diana-decomp --all-features --locked \
  -E 'test(dynamic_instructions_preserve_layout_references_and_completions)'
```

Dependency, reference-isolation and unit-test policies are owned by root/package
AGENTS.md and [CONTRIBUTING](../../docs/diana/CONTRIBUTING.md). The crate remains
unpublished (`publish=false`, version `0.0.0`).
