# T01 request resource control

Goal: implement cumulative request work/allocation accounting and cancellation
through `RunContext`. Contract: DESIGN §2.2, §3.7 and §10.3; ALGORITHMS §13;
producer: T01. Preparation evidence: `b02-b04-preparation.md` and
`debug-fallback.md` for the accepted draft-7 profile.

Fixed acceptance obligations: D062, D063, D064 and D086 in schema 2, draft-7.
Their recipes, invocations and expected values remain in `e2e_cases.json`.
Existing certification evidence is reused; this batch does not rebuild fixtures.

Not in scope: method/raw reservations, input parsing, Engine scheduling and rendering.

## Implementation

Files: `crates/diana-decomp/src/pipeline/control.rs`, `src/pipeline/mod.rs`
and `src/lib.rs`. Functions: `RunContext::new`, `checkpoint`, `charge_work`,
`charge_allocation`, `usage` and `checked_charge`.

Both implemented request limits default to None. Charges check cancellation,
checked arithmetic and the applicable limit before changing the cumulative ledger.
An observed cancellation is latched. Allocation charging records caller-reported
capacity increments before the caller attempts allocation; it does not allocate
or measure RSS.

## Verification

Platform: macOS arm64, rustc 1.98.0. Each command ran once and passed:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check --workspace --all-targets --locked
cargo build --workspace --release --locked
```

The default release artifacts are under `target/release/`; compilation is not
semantic evidence. No fixture or oracle changed. Domain tests: 0 executed;
`cargo nextest run` awaits a real public input/decompilation request, as required
by T01. Remaining obligations include method/raw quota isolation and reservation
release, allocation-site wiring, open/decompile/render integration, and the fixed
resource/cancellation cases through the public engine adapter in T05.

`git diff --check` and the sensitive-input audit passed. A temporary Git index
included the new source and batch files for the final diff/audit check without
changing the real index. No E2E invocations were executed in this slice.

Next: extend the ledger with method normal/raw sub-quotas and finite fallback
reservations under DESIGN §3.7 before wiring real input work.
