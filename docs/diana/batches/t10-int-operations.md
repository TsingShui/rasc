# T10 int operations

Goal: recover the fixed D010, D011 int and D012 operations with original int
width, shift behavior, explicit narrowing and unchecked exception order.
Contracts: ALGORITHMS §4, TASKS T10. Existing scalar/branch/loop and resource
cases remain regressions. No host arithmetic or synthetic intrinsic supplies
program results; independent expectations stay unchanged.

The sealed scalar AST now retains int division/remainder, bitwise operations,
signed/unsigned shifts, negation/complement and original byte/char/short
conversions. Each operand remains an explicit prior local; operations retain
original order. Java int operators supply the specified width, shift masks and
unchecked division exceptions. Original narrowing opcodes produce explicit
casts; they do not authorize truncating unrelated uses.

On macOS arm64, `.cache/e2e/engine/run-tlf6z3sk/report.json` records 17 selected
cases and 33 invocations: 16 whole cases and 32 seams passed. D010 verifies both
MIN/-1 and zero-divisor behavior; both D011 int calls and both D012 narrowing
calls passed actual Java compilation/JVM comparison. D011's long call remains
Debug, so D011 whole-case acceptance remains failed and the command exited 1.
The complete T10 primitive matrix remains pending.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D007 --case D009 --case D010 --case D011 --case D012 --case D055 --case D056 --case D057 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build and all 115 cargo-nextest core tests with zero skips passed. Existing fixed
core invocations cover this slice; no new tests or independent expected values
were introduced. Diff and sensitive-input checks passed before commit.
