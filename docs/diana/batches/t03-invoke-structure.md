# T03 invoke structure

Goal: validate original invoke word counts, adjacent wide argument words and
move-result kinds through the public method-layout operation. Producers: T02
checked method/prototype references and T03 original register operands.
Contracts: ALGORITHMS §1.2–§1.3 and DESIGN §5.1/§3.7. Reuse unchanged draft-7
B01–B04 input certification and comparator evidence recorded in TASKS §3.
Fixed obligations: F02 and D055/D056/D059/D062/D063. Expected values remain in
`e2e_cases.json`; independent core recipes exercise the structural boundary.

Not in scope: call dispatch/type analysis and Java recovery.

The normal method ledger charges prototype reads and argument traversal; the
existing fallback ledger retains the original facts after structural failure.
The implementation uses borrowed shorty/register iterators without allocation.

Source review: AOSP [Dalvik bytecode](https://source.android.com/docs/core/runtime/dalvik-bytecode)
calling conventions and [constraints](https://source.android.com/docs/core/runtime/constraints),
especially invoke word counts and result kinds. The implementation is derived
from the contract and self-made recipes; no reference implementation was copied.

Changed functions: `binding::check_producer` checks the original prototype and
word iterator; `ResultKind::accepts` checks move-result kind;
`Checker::check_instructions` binds the adjacent raw sites. `InputControl` for
`MethodBudget` sends parser work through the existing normal ledger and retains
request metadata/string limits. `check_method_layout` still returns only the
structural seal; register type/definition and dispatch analysis remain separate.

Red: four public core tests ran and failed because the old validator accepted
invalid invoke counts, nonadjacent wide words, mismatched results and damaged
callee metadata. An initial new-test closure arity error was corrected before
that behavioral run. Green: all 94 core tests pass with no skips, including nine
new invoke cases. The tests cover the list/range families, receiver counting,
wide words spanning the fifth list register, void/scalar/wide/object results,
dead-code checks, adjacency, local failures, fallback, work limits and cancellation.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t03_invoke)'
cargo nextest run -p diana-decomp --test contracts --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
python3 tools/audit/desensitize_check.py --range HEAD
```

Artifacts: `target/debug/` and `target/release/`. The unchanged preparation,
certification and comparator reports under `.cache/e2e/` remain available; their
fingerprint fields are informational and were not verified. T03 Report spans,
T04 metadata/041 and T05 Engine integration retain their acceptance requirements.
The all-features workspace run passes the same 94 core tests without skips;
format, Clippy, release build and diff checks pass. The audit includes new files
through a temporary Git index and preserves the real index.
