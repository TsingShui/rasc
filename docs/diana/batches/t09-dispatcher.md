# T09 ordinary dispatcher

Goal: execute fixed D002/D003 using verified CFG/SSA, exact-type edge-copy plans
and a normal no-handler dispatcher. Contracts: ALGORITHMS §3/§5/§6,
DESIGN §3.3–§3.5, TASKS T08/T09. Extend only the actual narrow scalar operations
and comparisons needed by these existing inputs. Required outcomes stay Java;
unsupported handlers and broader types remain explicit non-passes.

## Implemented

The normal branch producer now binds int arithmetic/bitwise-and and zero/two-word
comparisons before writes. It independently validates right-hand uses and their
original definitions. Each reachable block becomes one owned dispatcher case;
the selected edge's certified copies execute before its pc transfer. Entry copies
run before the loop. Every original operation retains its sequence.

`p8_java/dispatcher_verify.rs` independently enumerates case coverage, original
operations and terminals, solved types and exact EdgeIds. It replays each actual
copy sequence against the SSA parallel assignment through the P6 definition
checker. Its exact case domain, entry key and checked targets establish that the
synthetic default is unreachable. DA-only zero/false values never replace an
original definition, and the result owns no CFG/SSA arena or input bytes.

## Evidence

On macOS arm64, `.cache/e2e/engine/run-_2vfo8zp/report.json` records all 17 selected
whole cases and 29 invocations passing. D002/D003 pass their complete fixed
iteration sets; D004 continue, D005 nested break and D008 multiple exits also pass
actual source compilation and JVM behavior comparison. D001/D009 and the
input/raw/resource regressions remain passing. Parent: `3f724b7` plus this dirty
batch.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D004 --case D005 --case D008 --case D009 --case D055 --case D056 --case D057 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D084
```

Two minimal core regressions fill missing obligations: a hand-written two-entry
cycle exercises the actual owned AST with unassigned-local detection, and a legal
unthrowing protected method remains outside the ordinary dispatcher. They reuse
the existing core DEX builder. No auxiliary-tool tests were added.

The two-entry input was also independently projected with the existing physical
reader and dexlib2, executed with the B03 scalar oracle, and compared with actual
recovered Java on the locked JVM. Supplemental artifacts live in
`.cache/e2e/t09-dispatcher/{irreducible.layout,irreducible.dexlib,scalar-observations,jvm-observations}.json`.
The supplemental holder preserves the independently checked field, code-free
member and other canonical method; only the selected method comes from Engine.
It is a known-context method observation, not a native JavaUnit or an added fixed
case pass. Only format-defined DEX header integrity fields were populated for the
self-made supplemental input; no fingerprint acceptance gate was added.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 115 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Independent fixed values, manifest and TASKS.md are
unchanged.

Remaining T09 profile work includes switch transfer and throw/ref terminals.
They retain their actual unsupported outcomes. T08's loop entry/latch and critical
edge rollup now has actual dispatcher execution; general wide/FP legalization
remains T10.

Acceptance update after `9df7d96`: the ordinary dispatcher now also consumes the
original switch tables, and D007/D083 pass their complete required invocation
sets. T09's eligible narrow scalar CFG, loop/critical-edge behavior, independent
case/copy correspondence and no-handler rejection obligations are accepted.
Upstream type/ref/effect producers still gate broader inputs; their unsupported
results are not counted as successful Java by this acceptance.
