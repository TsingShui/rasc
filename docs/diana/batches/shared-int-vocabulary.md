# Shared int instruction vocabulary

This slice migrates existing int unary, arithmetic, bitwise, shift and reverse-
subtract operations into common CFG/SSA, use conversions and storage. Contracts:
[DESIGN](../DESIGN.md), [ALGORITHMS](../ALGORITHMS.md) and the historical
[int-operation evidence](t10-int-operations.md).

P2 decodes the normal, two-address and literal forms with original operand order.
Reverse-subtract keeps its literal left operand explicitly. Division/remainder
remain observable throwing operations. Negation, complement and byte/char/short
conversion retain their original operation and word definition.

P5 retains narrowed ranges and computes integer/boolean-word use conversions
before storage. Its existing fixed-point facts now preserve proven 0/1 results
of bitwise operations. P8 consumes and independently checks definition/order/view
identity. The assignment AST retains those views; P9 emits Java int operations
and explicit narrowing at the original sites. The natural/dispatcher consumers
reuse the same assignments and P6 storage.

No test, input, Driver or validation framework was added. The existing pure-loop
interpretation stays limited to its existing obligations rather than becoming a
new universal arithmetic validator. Related fixed `run-mrb4h6e9` passes
D009/D010/D011/D012/D071/D077: 6 cases, 17 invocations and 17 seams. D010's
observed shared method preserves division, including the fixed MIN/-1 and
zero-divisor expectations. D011 includes the retained legacy wide invocation;
that pass is not evidence of shared wide support.

Format, strict Clippy, release and cargo-nextest passed: 122 tests, zero skips.
The staged patch passed those Rust gates in a detached worktree without the
frozen static-field changes, reusing the certified D065 fixture. Diff and
sensitive-input checks passed before submission. Artifacts are under
`.cache/e2e/shared-int-vocabulary/`.

Not run: full required E2E after this slice. Wide/FP vocabulary parity and
remaining route deletion stay in the current task. No legacy scalar path is
removed on the strength of this int-only slice.
