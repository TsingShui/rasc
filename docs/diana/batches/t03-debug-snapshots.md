# T03 original-information snapshots

Goal: render original method facts as readable JSON using the existing fallback
budget, including explicit partial coverage and exact UTF16/word/payload facts.
Producers: T02 catalogs/range views and T03 layouts/original operands. Contracts:
ALGORITHMS §7–§8 and DESIGN §2.3/§3.7. Fixed obligations: F02/F03 and
D055/D056/D059/D061/D062/D063/D086; reuse independent expected evidence.

Not in scope: Java recovery and T05 Engine integration.

Input reads in the fallback path use the same checked parsers and existing
request/method ledgers, not another context or retry. Output capacity and escaped
UTF8 bytes are checked before reservation/copy. Partial data stays explicitly
partial; cancellation and resource errors never return truncated success.

Files/functions: `p9_emit/debug.rs` owns `ProgramIndex::debug_method`, typed
coverage/issues and original fact formatting; `json.rs` owns fallible UTF8
buffering/escaping; `error.rs` owns classified errors. The input reader shares
its checked grammar through the two actual `InputControl` implementations:
request admission and fallback. No additional ledger or context is introduced.
`catalog.rs` retains independent hierarchy/interface/value-pointer failures and
can select one method without decoding other method bodies. `control.rs`
enforces the existing `fallback_output_bytes` contract. Input and output
`mod.rs` files contain declarations/re-exports, following the operator's rule.

Red: public snapshot tests failed compilation before their producer existed.
The discovered-malformation/output-exhaustion case then failed with the wrong
BudgetExceeded classification; a sticky, allocation-free primary issue now
preserves Malformed alongside the concrete secondary resource failure.
Cancellation and BUG are not masked. The Unicode recipe also exposed a wrong
new-test selector: the independent builder sorts method IDs, so the case now
uses its recorded original primary ID rather than assuming 0.

Green: 81 public-interface core tests pass with no skips, including 13 snapshot
cases and all 68 previous cases. Coverage includes ownership after input drop,
UTF16 surrogates/NUL, supplementary names, exact words and operands, ordered
handlers, local corruption, unknown-width prefixes, selected-member isolation,
normal/raw work separation, explicit code/output limits and cancellation.

Source review: scalar/character JSON serialization uses fixed stack scratch
with bounds derived from the scalar formats; long text copies checkpoint in
UTF8-aligned chunks. The owned String avoids a second uninterruptible full-output
UTF8 validation. Every capacity increment is checked/charged before fallible
exact reservation. No source is published on a resource failure. A malformed
primary is remembered before allocating an issue-list entry. The decoder reads
original facts only; no failed Java AST, guessed width or renderer retry is used.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t03_debug)'
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t03_debug_output_exhaustion)'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
python3 tools/audit/desensitize_check.py --range HEAD
```

Artifacts: `target/debug/`, `target/release/`. The audit uses a temporary Git index
for new files without staging the real index. T04 still supplies encoded values,
original debug programs and native 041 facts; unavailable metadata remains
explicit. T03 safe-excerpt/Report spans and T05 public Engine integration retain
their original acceptance obligations. No fixed Engine case or T/H task is accepted.
