# T05 typed request observations

Goal: expose actual public input, method and cancellation failures plus existing
normal/raw controls through the current E2E binding. Fixed draft-7 cases:
D055–D059, D062, D063 and D086. Contracts: DESIGN §2.2–§3.7,
ALGORITHMS §1, §3, §8 and §13, and TASKS T01–T05.

The caller supplies limits and arms cancellation after open. Error observations
retain typed failure kinds; missing metadata stays explicitly absent. Fixed
expectations remain unchanged.

## Implemented

`Limits` accepts its existing optional fields through serde without treating zero
as unlimited or accepting unknown fields. The public example consumes caller
controls, arms the callback after `Engine::open`, and serializes concrete errors
from open/decompile/render. An absent Report stays absent. Standalone symbolic
caller selection uses public checked input APIs without reading method bodies.

The adapter retains unavailable metadata as null. A diagnostic span can use the
certified spelling of the actual returned original key; this does not populate
missing `MethodFacts`. Hard failures retain their actual members and sources but
are not trusted Debug successes. The materializer only constructs trusted Debug
observations for available Debug members, not Unavailable excerpts. Error-only
observations do not claim rendered-span certification.

## Evidence

Executed on macOS arm64 using the locked release development example:

```sh
python3 tools/e2e.py run --case D055 --case D056 --case D057 --case D058 --case D059 --case D062 --case D063 --case D086
python3 tools/e2e.py run --case D055 --case D056 --case D059 --case D060 --case D061 --case D062 --case D063 --case D075 --case D084
```

Reports: `.cache/e2e/engine/run-he6qku4r/report.json` and
`.cache/e2e/engine/run-8u8evqtj/report.json`. Both record `0218f5e` plus the dirty
source tree for this batch. The latter selects 9 whole cases and 16 invocations;
8 whole cases and 14 invocations passed. The latter command exits 1 because the
two D075 Java obligations remain unimplemented. D057/D058 remain failures in the
first command; their required semantic rejection needs the definition/use
producer. D086 explicit-code-limit passed, while unlimited-default still fails
its required Java assertion.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, 110 existing cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. The fixed specification, manifest and TASKS.md are
unchanged.

Remaining: normal member/report output limits, original field-initializer
observation agreement, and executable Java/definition-use production. Continue
with the D009 scalar chain using the validated raw-method and owned-report
producers; task-wide T01–T05 acceptance remains pending.
