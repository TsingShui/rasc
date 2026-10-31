# T01–T03 public acceptance

Goal: verify the existing T01–T03 producers against their fixed public contracts
after T05 wiring. This record supplements TASKS.md without changing it.

## Reviewed producers

- T01 uses direct native calls through the typed scalar schedule and the same
  RunContext for open, method work, fallback and rendering. It has no pass
  registry or reset/retry. Explicit fallback reservations and normal/raw charges
  are independent; unspent holds alone return on scope exit.
- T02 uses borrowed bytes, typed coordinates and original IDs/full prototypes.
  Public catalog/range checks cover F01, M01/M02 and independently locatable
  declaration/code failures.
- T03 shares the instruction registry across layout, validation and original
  output. Java rejection discards unverified source; fallback reads original
  ranges and preserves classified failures, coverage and safe excerpts.

## Fixed evidence

The macOS arm64 artifacts use unchanged draft-7 inputs and independent values:

- `.cache/e2e/engine/run-rr25h4qf/report.json`: D062 normal/raw quota isolation,
  D063 actual armed cancellation without a fallback Report, and both D064
  output-resource obligations passed. The supplemental native normal-output
  operation and hard-error rendering observation are recorded in
  [output limits](t01-output-limits.md).
- The same run passed D055's two minimum-size/count faults, D056's invalid
  instruction target and D057's actual undefined use. D009 supplied compiled
  executable Java rather than an empty forwarding request.
- `.cache/e2e/engine/run-pybwguro/report.json` and
  `.cache/e2e/engine/run-erf6grqd/report.json` retain passing D059/D060/D061
  original source, span, word, branch, payload, handler and reference checks.
  D065's two original native views passed their engine obligations; its
  application obligations remain unimplemented and the whole case stays failed.

Existing complex public core tests cover exact work/allocation boundaries,
non-refunding failures, cancellation during parsing/output, normal/raw isolation,
F01 declarations/overloads, M01/M02 ranges, LEB faults, partial class failures,
format/version facts and unknown widths. No wrapper/getter/forwarding tests or
auxiliary self-tests were added.

```sh
cargo nextest run --workspace --all-features --locked -E 'test(t01_) | test(t02_) | test(t03_)'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
```

The selected run passed 89 tests; 21 outside that selector were skipped and are
not counted as passes. The full unchanged workspace run passed all 110 tests
with zero skips in [native container evidence](t04-native-container-e2e.md).
The selected log is `.cache/e2e/t01-t03-acceptance-nextest.log`. Fmt, Clippy,
release build, diff and sensitive-input checks passed.

The task acceptance is limited to these producers and the executed host profile.
General scalar, string, calls, complete JavaUnit and later semantic obligations
retain their own task evidence; this record does not count them passed.
