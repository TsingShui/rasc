# T10 cmp and NaN branches

Goal: retain original cmp-long/cmpl/cmpg semantics and execute every D015 method
and NaN position in its known source context. Contracts: ALGORITHMS §4.2/§4.6
and TASKS T10. Operands stay materialized; the NaN bias stays explicit and no
synthetic library call implements comparison. Existing numeric, boolean/null,
narrow, CFG and resource evidence remain regressions.

Scalar flow now retains cmp-long and both FP bias forms with exact operand
families. The ordinary CFG producer retains original float cmpl/cmpg operands,
fixed result identity and bias; its independent replay checks those facts.
Typed float locals and exact-type edge storage feed the owned AST. P9 emits
`x < y ? -1 : x > y ? 1 : x == y ? 0 : bias` for FP, without deleting the NaN
branch or repeating operand effects. The original following branch remains a
separate operation.

The thin adapter now assembles explicitly requested method groups in a known
context. Every requested original identity must be present and every selected
member must actually be Java. It replaces only those method bodies in the
certified holder. The outcome retains the full original class status, diagnostics
and member slices under `context_report`; the actual public artifact remains
unchanged. D015's constructor remains Debug and its class Report remains Mixed.
This is method-group evidence, not native JavaUnit acceptance.

On macOS arm64, `run-dg2er_m1` passed D015's complete two four-method NaN-position
invocations and existing D001/D016/D017 calls. The full mandatory run is
`.cache/e2e/engine/run-j6t_18p4/report.json`: 86 cases, 177 invocations and 181
seams retained, with 29 whole cases and 78 seams passed. It exited 1 for the
remaining real capability/application failures. D071 call-byte still fails;
D076 and the rest of the pending matrix retain their actual outcomes.

```sh
python3 tools/e2e.py run --case D015 --case D001 --case D016 --case D017
python3 tools/e2e.py run
```

An intermediate adapter variable-shadowing failure is preserved in `run-4ys8v522`;
only the corrected actual core runs supply acceptance evidence. No auxiliary-tool
tests or comparator falsification were run. Workspace fmt, all-target/all-feature
Clippy with warnings denied, locked release build, all 116 cargo-nextest core
tests with zero skips, diff checks and the sensitive-input audit passed.
Fixed cases, certified inputs, independent expected values and TASKS.md are
unchanged. Supplemental long/double cmp execution and the remaining target
literal boundaries still need evidence before whole-T10 acceptance.
