# T01 method budgets and diagnostics

Goal: implement method normal/raw quota isolation, finite fallback reservations
and classified resource diagnostics. Producer: T01; contracts: DESIGN §2.3,
§3.7 and §10.3, ALGORITHMS §13. Preparation evidence is reused from
`b02-b04-preparation.md` and `debug-fallback.md`.

Fixed obligations: schema 2, draft-7 D062/D063/D064/D086. Input identities and
expected values remain in `e2e_cases.json`; no fixture rebuild is required.

Not in scope: DEX parsing and Java reconstruction.

## Implementation and verification

Work starts in `crates/diana-decomp/src/pipeline/control.rs`. Request and method
charges are checked separately before committing either ledger. Only finite
fallback values reserve parent work/allocation; dropping a method returns only
remaining reservations. Cancellation is checked before reservation, charging and
the consuming normal-to-fallback transition.

Changed functions: `RunContext::method_budget`, `MethodLedger::charge_work`,
`MethodLedger::charge_allocation`, `MethodLedger::drop`, `Quota` checked operations,
`MethodBudget::into_fallback`, and `ControlError::{kind,code}`. The consuming
normal-to-fallback transition keeps the same counters and parent borrow.

Source review checked failed reservations/charges for atomicity, protection of
held resources, normal/raw method-allocation totals, and drop after cancellation.
Resource errors retain inline numeric data; classification adds no heap allocation.

Platform: macOS arm64, rustc 1.98.0. Each command ran once and passed:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check --workspace --all-targets --locked
cargo build --workspace --release --all-features --locked
```

Artifacts: `target/release/`. Domain tests: 0 executed. F00 resource/cancellation
execution awaits a real public input request under T01/T05; no private-ledger or
mechanical forwarding tests substitute for that execution. T01 remains unaccepted.

Remaining producer dependency: a useful `Engine::open` requires the checked DEX
index owned by T02. Decision requested: implement the T02 input slice next, then
complete T01 Engine/diagnostic-location wiring against real input and execute the
applicable core tests with nextest. Full request/output diagnostics and the fixed
D062/D063/D064/D086 invocations retain their integration obligations.
