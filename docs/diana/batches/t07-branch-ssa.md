# T07 branch SSA

Goal: build fixed-identity entry Phis for the D001 normal branch graph, verify
edge-indexed incoming values independently, then connect its minimal diamond to
owned Java output. Contracts: ALGORITHMS §3/§5–§6, DESIGN §3.3, TASKS T07/T08.
Only actual narrow scalar consumers are added. Fixed D001 and the existing
scalar/input/resource regressions remain authoritative; unsupported profiles
retain their real non-pass outcomes.

## Implemented

`pipeline/branch.rs` builds original-order leaders and distinct normal EdgeIds,
including two edges to the same successor. Sparse live-in demands grow
monotonically. Every demanded entry slot receives a fixed Phi, including
single-predecessor and EntrySeed inputs. Each block binds once; the normal edge
snapshot stores only demanded slots. IDs are checked and never reassigned during
propagation.

The sealing validator independently replays original read/write order, constants,
latest uses, edge post-commit environments and the complete incoming set. Bad and
grounded information grows monotonically. Unknown types stay unknown; a later
type conflict cannot replace an already-known malformed definition failure.

`p8_java/branch.rs` lowers the admitted acyclic diamond with separate locals and
edge-indexed Phi copies. The source declares locals without invented initial
values and assigns join storage only on the actual selected edge. This profile
checks that its copy sources do not overlap destinations; general copy cycles
remain a T08 obligation. Report owns only the verified source AST, not the CFG or
SSA arena. P9 renders the verified exclusive Scalar/Diamond body variant.

## Evidence

On macOS arm64, `.cache/e2e/engine/run-fiq7j68i/report.json` records all 12
selected whole cases and 17 invocations passing, including both D001 branches
through the fixed compiler/JVM driver and the D009/input/raw/resource regressions.
The report records `91ea0d1` plus this dirty batch.

```sh
python3 tools/e2e.py run --case D001 --case D009 --case D055 --case D056 --case D057 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D084
```

One minimal core regression reuses the existing `f(I)I` input builder through
public Engine calls. Existing coverage had no Phi consumer before this batch.
Its table checks equal-target distinct edges, nested diamonds, a grounded back
edge, undefined joins, an ungrounded read cycle and a larger branch graph under
an explicit normal-work bound. Unstructured valid graphs retain classified Debug;
undefined graphs are Malformed; the bounded graph retains complete raw output.
No private IR counts, pass order or text goldens are asserted.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 111 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. The fixed expectations and TASKS.md are unchanged.

Remaining: general edge-copy cycles, wide slot overlap, dispatcher/loop emission
and broader type/effect profiles. These are not counted passed by the D001
observation. The fixed narrow SSA artifact and diamond consumer are ready for
the T08/T09 chain.
