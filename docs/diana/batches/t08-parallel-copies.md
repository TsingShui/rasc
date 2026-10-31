# T08 typed parallel copies

Goal: implement destination-leaf sequentialization with exact-type cycle temps
and an independent symbolic definition check. Contracts: ALGORITHMS §5.1/§5.3,
DESIGN §3.5, TASKS T08. The actual D001 edge-copy consumer will use this producer;
fixed D001/D009 and input/resource regressions remain authoritative.

`p6_variables/copies.rs` checks unique destinations and exact local-type/word-width
identity, removes only identity copies, emits a destination absent from pending
sources, or saves a cycle source into a fresh exact-type temporary and rewrites
all its pending uses. Every scan, rewrite, growth and emitted copy charges the
normal method ledger. No cast, boxing, call or invented default is introduced.

The independent definition check replays old-store symbols, validates temporary
definition before use and verifies every original local, including unwritten
locals. SSA remains responsible for edge-source availability. The D001 consumer
now uses this plan on its selected edges and owns actual temporary declarations.

Two necessary core-module tests cover the new algorithm through its real module
boundary. An internal value interpreter checks chains, 2/3-cycles, fan-out,
identity and repeated entry/latch transfers with full 64-bit payloads. The
certificate rejects naive cyclic assignments, changed types/widths, missing temp
definitions and extra writes. It does not assert names, copy order or private
IR layout. These are core copy/definition semantics, not tool self-tests.

On macOS arm64, `.cache/e2e/engine/run-zexf5y6_/report.json` records 11 selected
whole cases and all 16 invocations passing through the existing compiler/JVM and
raw comparator. The parent is `6238ee5` plus this dirty batch.

```sh
python3 tools/e2e.py run --case D001 --case D009 --case D055 --case D056 --case D057 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 113 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. The selected two-test copy run passed; its 111
unselected tests were not counted as passes. No independent expectation or
TASKS.md changed.

T08 remains pending actual loop-storage and critical-edge rollup with T09. The
copy producer is ready; D002/D003 still need arithmetic/two-operand branch flow
and dispatcher emission before their Java behavior can be accepted.

Acceptance update after `c4748d5`: the complete D002/D003 iteration sets now pass
actual compiler/JVM comparison in `run-_2vfo8zp`. The independently executed
hand-made two-entry cycle also exercises critical incoming edges and proves that
DA-only locals are not read. The T09 structure/definition checker reconstructs
parallel assignments from SSA and independently validates the actual stored edge
sequences. Combined with the exact-type 2/3-cycle and wide-bit core checks, this
closes the T08 copy/storage obligations without claiming Java support for every
wide or effectful method.
