# T01 final output limits

Goal: enforce the configured normal member and final rendered-report byte limits
on the existing public request path. Contracts: DESIGN §10.3, ALGORITHMS §8/§13,
TASKS T01/T05; fixed draft-7 D064 with D009/D055–D063 regressions.

Bounds remain opt-in. The final UTF8 buffer includes actual escaped source and
report markers; a failure publishes no successful truncated buffer. Execution
evidence is recorded below.

`TextBuffer::method` checks the normal member's final byte length before growth;
`TextBuffer::report` checks one cumulative final text buffer, including markers
and diagnostics. Intermediate signature/analysis storage remains allocation
accounting, not an output-length estimate. The existing fallback byte ledger
remains independent. All omitted bounds stay None.

The development caller exports the concrete exceeded RequestLimit. If final
rendering fails, its error observation retains known hard Report diagnostics
rather than replacing Malformed with a later output-budget failure.

On macOS arm64, the fixed release run is
`.cache/e2e/engine/run-rr25h4qf/report.json`: 11 selected whole cases and 15
invocations passed, including both D064 invocations, D009 and D055–D063 selected
regressions. Command:

```sh
python3 tools/e2e.py run --case D009 --case D055 --case D056 --case D057 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D084
```

Supplemental public-consumer operations reused certified D009 and D055 inputs.
`.cache/e2e/t01-output-limits/D009-observed.json` retains Debug with the normal
`max_method_output_bytes` diagnosis. `D055-observed.json` retains Malformed ahead
of the final report-output failure. These are supplemental observations, not new
fixed-case passes.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, 110 existing cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. TASKS.md and independent expectations are unchanged.

Remaining: D064's NUL-returning method currently uses raw because executable
string lowering is not implemented; normal escaped-string emission still needs
its producer and evidence. Original field-initializer agreement and the remaining
T01–T05 acceptance obligations also remain pending.
