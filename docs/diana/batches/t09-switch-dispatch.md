# T09 switch dispatcher

Goal: consume the original checked packed/sparse switch payload in the native
CFG dispatcher and execute fixed D007/D083 without source restructuring.
Contracts: ALGORITHMS §2/§3/§6, TASKS T09; D001–D005/D008/D009 and existing
input/raw/resource obligations remain regressions. Shared decoding is retained;
no independent expected values or input bytes change.

The existing payload decoder exposes its borrowed switch cases to normal flow.
The producer bases case deltas on the switch opcode, creates a separate normal
EdgeId per key/default even when targets coincide, and preserves original signed
keys. Sparse and packed payload data remain non-executable. The liveness/SSA
consumer binds the selector as an original read.

The dispatcher AST and independent structure checker retain each key, selected
edge identity and default successor. P9 writes an inner Java switch whose chosen
copy sequence runs before pc/continue. This does not perform natural switch
restructuring or copy a body into multiple cases.

On macOS arm64, `run-8sgtb_bd` passed all 13 selected invocations of
D002/D003/D007/D083, including shared cases/fallthrough and both signed key
extrema. The full required-set run is
`.cache/e2e/engine/run-fueh18wt/report.json`: 86 cases, 177 invocations and 181
seams were retained; 19 whole cases and 44 seams passed. It exited 1 for the
remaining real Java/capability and shared application failures. No required
case or invocation was removed to obtain these counts.

```sh
python3 tools/e2e.py run --case D007 --case D083 --case D002 --case D003
python3 tools/e2e.py run
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 115 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. No tests, manifests, independent expected values or
TASKS.md changed. Remaining broader primitive/ref/throw and complete class
obligations keep their actual non-pass outcomes.
