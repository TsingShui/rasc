# T04 native container E2E

Goal: execute D065's existing engine-original-view obligations on original 041
bytes and header offsets. Contracts: DESIGN §3.3/§6.5, ALGORITHMS §1.1/§14.2,
TASKS T04/T05. The existing certified Java mirrors supply only the known
compilation context; the selected method is replaced by actual owned Engine
source. Shared application observations retain their independent non-pass state.

The adapter calls the same native public Engine example with the original
container path and each certified header coordinate. It does not overlay a
header, normalize to 035, copy a logical image or change any input bytes. The
known mirror holder's namespace is mapped back to the original holder before
compilation; its selected method is then replaced, while its constructor stays
unchanged. The comparison uses the existing independent physical projection and
JVM driver.

On macOS arm64, `.cache/e2e/engine/run-erf6grqd/report.json` records both engine
seams passing, with their distinct driver results and matching original header
coordinates. The selected set contains 8 cases, 12 invocations and 14 seams;
7 whole cases and 12 seams passed. D065 remains a whole-case non-pass because its
two application seams are unimplemented. The command exited 1:

```sh
python3 tools/e2e.py run --case D065 --case D009 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, 110 existing cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. No tests, source fixtures, independent expectations
or TASKS.md changes were added. ART and other host-platform executions were not
run; this observation certifies the executed native/JVM profile only.
