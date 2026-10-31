# T06 scalar method

Goal: recover the fixed D009 method through a closed single-block int profile,
with explicit definitions, types, locals and return completion. Consumers use the
existing validated raw input, owned Report and fixed E2E binding. Contracts:
DESIGN §3.3, ALGORITHMS §3–§7 and §12.4–§12.6, TASKS T05/T06.

The profile preserves int-width arithmetic and all formal parameters. Unsupported
members remain original output in Mixed; this batch does not claim JavaUnit.

## Implemented

`pipeline/scalar.rs` consumes validated raw input through private Flow, SSA,
Resolved, Typed and Storage products into a sealed single-block plan. Parameter
seeds and every definition receive distinct checked ValueIds. Uses bind existing
definitions; an undefined read returns a classified malformed error. Each
definition has an explicit LocalId. The closed vocabulary proves the empty call
binding and the single-block region; it does not implement general calls or CFGs.

`p8_java/scalar.rs` admits static int signatures, identifiers, completion,
original-code ownership and directly determined target limits. Its private
constructor owns the Java AST and records EnvironmentId and Java8 compilation
contract. `p9_emit/java.rs` writes only that verified AST. Arithmetic assigns int
locals rather than evaluating in host arithmetic or producing expression
statements. Report owns the AST, source and original identity after input release.

The existing known-context E2E caller replaces only the selected declaration in
the original self-made Java source. Other members and the original holder stay
unchanged. No engine JavaUnit is claimed.

The new producer exposed a D060 regression: recovering one return-only overload
lost its required Debug member observation. Header-only source-group admission
now rejects that collision without reading another method's body.

## Evidence

```sh
python3 tools/e2e.py run --case D009 --case D055 --case D056 --case D057 --case D059 --case D060 --case D061 --case D062 --case D063 --case D084
```

On macOS arm64, `.cache/e2e/engine/run-uts252qu/report.json` records all 10
selected whole cases and 13 invocations passing through the existing comparator.
D009 compiled and executed both recovered methods against the fixed JVM driver.
D057 now rejects its actual undefined return. D055/D056/D059 rejection and excerpt
behavior, D060/D061/D084 raw integrity, and D062/D063 resource/cancellation
observations remain passing. The report records `e76b19b` plus this dirty batch.

Checks passed: workspace fmt, all-target/all-feature Clippy with warnings denied,
locked release build, 110 existing cargo-nextest core tests with zero skips,
diff checks and the sensitive-input audit. No new tests, fixtures or independent
expectations were added. TASKS.md remains unchanged.

## Remaining

The profile has no general Phi, dispatcher, wide/FP, effectful, handler or monitor
lowering. D058's wide overwrite obligation remains pending; it is not converted
to int or accepted as a pass. T01–T05 still need output-limit and original field
initializer evidence. T06 task-wide acceptance remains pending those obligations;
the D009 producer is ready for the D001 chain.
