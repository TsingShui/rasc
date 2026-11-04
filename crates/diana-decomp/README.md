# diana-decomp — native engine library

Diana's independent native Rust engine borrows caller-owned DEX bytes. Files,
ZIPs, network access and output ownership belong to the application. Production
forbids unsafe code and external runtime/process dependencies; declarations come
only from the analyzed input.

The active method chain is P0 class facts → method reader → P1 disassembly → P2
execution model. `Engine::analyze_method` and class requests use one method task.
P3–P9 are not yet active; the application decompilation interface is being rebuilt.
The approved architecture is [PIPELINE_DESIGN](../../docs/diana/PIPELINE_DESIGN.md).

P1 retains complete instruction/payload facts, original references, entry layout
and structural associations. P2 owns ordered operations, exact slots, explicit
normal/exception frame/heap/lock completion rules, transfers, exits and coverage.
It does not expose its complete P1 predecessor. Read-only `MethodEvidence` supports
original inspection and checks without product extraction or construction access.
P2 constructors are internal and require Engine's method-task authority.

A P2 stop retains the complete P1 input, classified error and available unvalidated
construction facts. Class requests retain delivered members and mark request-level
termination explicitly. Cancellation is latched in `RunContext`; range/capacity
checks and fallible allocation remain mandatory. There are no numeric quotas.

Translation checking compares ordered local symbolic behavior and completion
rules and matches every original transfer/coverage owner. It does not prove ART
admission or concrete external-call behavior. The two local corpora exercise
ordinary operations but do not provide execution evidence for custom/polymorphic
invokes or method-handle/type constants. Fault-injected cancellation/allocation
execution has not been performed; failure retention is currently code-reviewed.

Use the single engine inspection entry from the repository root:

```sh
cargo run -p diana-decomp --release --example dbg_shell --locked -- \
  --dex <classes.dex> --exec '{"cmd":"p2","class":0,"method":0,"details":true}'
```

`p2-all` observes the Engine chain and reports classified outcomes and encountered
opcode variants. `roundtrip-all` checks independent byte re-encoding over retained
P1 evidence. No phase-specific debug binary or unit-test fixture is required.

Applicable engine gates:

```sh
cargo fmt --all -- --check
cargo clippy -p diana-decomp --all-targets --all-features --locked -- -D warnings
cargo build -p diana-decomp --release --locked
```

Dependency, reference-isolation and unit-test policies are owned by root/package
AGENTS.md and [CONTRIBUTING](../../docs/diana/CONTRIBUTING.md). The crate remains
unpublished (`publish=false`, version `0.0.0`).
