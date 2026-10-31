# T03 original operand facts

Goal: expose checked original instruction/payload facts for the debug writer and
reuse the same operand decoder in whole-method checks. Producers: T02 borrowed
code ranges and T03 shared layouts. Contracts: ALGORITHMS §1.2/§7.1–§7.3 and
DESIGN §3.2/§3.7. Reuse the unchanged independent input/expected evidence.

Fixed obligations: F02/F03, D056/D059/D061/D062/D063/D086. No Engine invocation
is accepted by instruction-level evidence.

Not in scope: full member snapshots, reference-value resolution and Java recovery.

The decoder borrows original words and payload bytes without allocating or
choosing source types. Register operands preserve encoded word order; literals
preserve width/bits; offsets remain relative code-unit deltas. Payload iteration
retains keys, offsets and element bits in input order. Each consumer charges its
variable loops through the existing normal or fallback budget.

Files/functions: `p1_input/operands.rs` owns `RawMethodView::visit_instructions`,
`instruction_operands` and borrowed switch/array iterators. `structure.rs` consumes
the same operands for register spans and branch deltas. `substrate/ids.rs` adds
consumed register and signed code-unit identities and checked byte conversion.
`tests/input/operands.rs` uses independent original words, not private IR goldens.

Red: the `t03_original` selector failed compilation before its public producer
existed. Green: all five operand cases pass; the all-features workspace run
passes all 68 core tests with no skips, including the 63 prior cases. The selected
five-case run filtered the 63 regression cases rather than claiming them as run.

Source review: opcode/format facts stay in the existing OpcodeSpec; no second
opcode table is introduced. Register list counts are sealed below their storage
length; range expansion uses u32 for an original out-of-frame end without wrap.
Payload extents are proven before slices, array padding is retained separately
from element bytes, and packed implicit-key overflow remains visible as i64 in
unvalidated facts. The normal structural validator still rejects that overflow.
The decoder is constant-work and allocation-free; callers charge each payload
iteration. Unknown encodings preserve the prefix and stop before another word.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t03_original)'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
python3 tools/audit/desensitize_check.py --range HEAD
```

Artifacts: `target/debug/`, `target/release/`. The audit includes new files through
a temporary Git index. Original facts do not claim a ValidatedRawMethod or
completed Engine snapshots; T03 member rendering, T04 metadata and T05 integration
retain their original acceptance obligations.
