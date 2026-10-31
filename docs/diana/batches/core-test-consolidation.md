# Core test consolidation

Goal: consolidate the T03 invoke and T04 debug-program tests by public semantic
obligation and remove duplicate executions. Contracts: TASKS §1, DESIGN §2/§3.7
and ALGORITHMS §1.2–§1.5/§7. Reuse unchanged draft-7 B01–B04 evidence; the fixed
F02/F03 and D055/D056/D059/D062/D063 obligations retain their acceptance values.

| Coverage | Consolidation |
|---|---|
| Invoke counts | Keep exact, missing and extra words for every list/range family; remove repeated mismatch counts and retain the dead-code counterexample. |
| Wide arguments and results | Merge the fifth-word counterexample into wide-word checks and all four result categories into the result test; retain the intervening-nop rejection. |
| Invalid callee metadata | Keep raw readability and declaration isolation in one execution; remove the preliminary duplicate validation. |
| Normal work and cancellation | Move invoke inputs into the existing resource/cancellation tests, retaining zero quotas, exact work bounds, raw isolation and both scan/reference cancellation paths. |
| Debug original facts | Combine positions, NO_INDEX and non-shortest encoded bytes in one input; retain exact UTF16 local names/signatures separately. |
| Debug partial coverage | Use one damaged input to check retained instructions, event prefix, byte range and another method. |
| Debug locals and boundaries | Combine parameter initialization variants and boundary/failure-kind variants; retain distinct lifetime, register-span, line and truncation failures. |
| Debug resources | Keep the parser's work/local-allocation boundaries; reuse the existing UTF8 output-limit test and extend the existing cancellation test with long metadata. Remove the duplicate malformed-ULEB high-bit input already covered by reader tests. |

Changed files: `tests/input/{invoke,debug_program,structure,debug}.rs` under the
engine crate. `AGENTS.md` requires an existing-coverage review and justification
for an additional test. No production algorithm, quota or frozen oracle changes.

The two feature files now contain twelve tests instead of twenty-three. The
workspace executes 97 tests instead of 108, with no skips. These counts describe
the runner; the coverage review above determines which inputs were redundant.

Four temporary production mutations disabled invoke count validation, wide-word
adjacency, result-kind validation and the correct partial byte-range endpoint.
Each retained public test rejected its mutation with nextest exit 100. The source
files were restored byte for byte before the closing gates; the production diff
is empty. The observations and selectors are recorded in
`.cache/e2e/core-test-consolidation/mutations.json`.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
git diff --exit-code -- crates/diana-decomp/src
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked --status-level fail --final-status-level fail
git diff --cached --check
python3 tools/audit/desensitize_check.py --staged
```

The closing workspace run passes all 97 tests without skips. Format, Clippy,
release build, staged diff and sensitive-input checks pass. The fixed Engine
acceptance obligations remain in TASKS; this consolidation revises no oracle.
