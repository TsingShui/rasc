# H06 layered limits, cancellation and publication

Baseline: 67763dd. Contracts: DESIGN §7 and §10.3, TASKS H06. `ALGORITHMS.md` is
unchanged.

## Products

`analysis/limits.rs` maps the caller's explicit options to the two ledgers: the
input lease (`max_input_bytes`, `max_inflated_bytes`) and the engine
`Limits` (`max_request_work_steps`, `max_request_allocation_bytes`,
`max_work_steps`, `max_method_output_bytes`, `max_report_output_bytes`). Every
field stays `None` unless the caller sets it; the application adds no numeric
default and no estimated rejection threshold.

The CLI exposes those fields as global options on every command. `get-class` and
`get-method` create one `RunContext` from them and pass a cancellation closure that
reads an atomic flag, so a stop is cooperative and never continues into the raw
fallback. Cancelling during locator resolution now reports `Stopped` instead of a
misleading `NotFound`; an engine resource or cancellation error is never converted
into a stale-locator answer.

Every delivered payload is checked by `limits::enforce_output` on its **actual
bytes**, after escaping and after the complete artifact is built. An overrun is a
classified failure with the limit name, the actual size and the maximum; the
temporary file is not published, so no truncated success can be observed. Text and
JSON are checked separately and are never added together, and the check is not
charged again as an allocation.

## Evidence

`cargo test -p diana --test product_contracts h06_` runs three public cases:

- An explicit `max_input_bytes` below the file size fails with
  `BudgetExceeded { limit: InputBytes }`; `enforce_output` admits the exact
  boundary, rejects one byte more with the reported actual/maximum, and does
  nothing when the bound is unset.
- A request cancelled before it starts and a request cancelled after 200 ledger
  checks both return a classified failure with no report instead of a completed
  result.
- `max_method_output_bytes` keeps the request alive and turns the affected member
  into a `BudgetExceeded` diagnostic, while `max_report_output_bytes` stops
  rendering with a classified error instead of a truncated report.

Manual CLI checks on the same fixtures: `--max-report-output-bytes 100` on a class
request reports `max_report_output_bytes exceeded: final output is 2292 bytes,
maximum 100` with exit 1 and no output; `--max-input-bytes 1000` on a 152 MB APK
reports `input_bytes exceeded: requested 152526071, maximum 1000` with exit 1.

Gates: formatting, strict workspace Clippy and the locked release build pass;
`cargo nextest run --workspace --all-features --locked` passes 228 tests with zero
skips.

## Remaining work

A SIGINT handler is not claimed: cancellation is caller-driven through the atomic
flag, and a process killed by a signal is not described as a catchable
cancellation. Per-request caching, dependency-catalog charging and the D064
report-overrun fixed cases remain with the later H batches.
