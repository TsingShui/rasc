# T11 effectful receiver and argument order

Goal: execute D018 through the original factory, left-to-right int argument
calls and virtual sum binding. Contracts: ALGORITHMS §9.1/§9.4 and TASKS T11.
The returned initialized receiver keeps its exact declared source type; no
allocation/init rewrite or speculative cast supplies it. Existing call-kind,
super/effect and resource observations remain regressions.

P4 now admits a plain public non-builtin class return with its exact descriptor
retained in the sealed original prototype. Nominal return descriptors do not
stand in for generic/array/interface type proofs. Source-visible overload
parsing retains return-only collision rejection. Receiver contracts additionally
admit an exact int formal vector; the older zero-argument receiver producer
retains its explicit arity guard.

A closed straight-line producer assigns fixed typed value IDs to constants and
call results. Arguments and the receiver are read before the adjacent result
writes its register. Calls complete in original order, including calls whose
results are unused. The initialized factory result retains its exact declared
source type and its original reference/dispatch identity. Unsupported flow or
conversions stay raw. P8 independently replays the original word operands,
references, value definitions, result adjacency/kinds and returned value before
building the owned call AST. Source calls use materialized locals, so null is not
checked before argument effects. The AST's compact statement IDs refer to a
separate owned call arena; input bytes and register analysis are not retained.

`.cache/e2e/engine/run-0bybwpb7/report.json` passes D018 and all selected prior
call cases. D018's actual compilation/JVM comparison preserves receiver R,
arguments A1 then A2, and the sum call/result. Public call evidence includes the
factory, both arguments and original virtual sum reference in program order.

Supplemental authorized core input is under
`.cache/e2e/t11-effectful-arguments/`. Locked javac/D8 produce a program with an
unused result-producing call before the factory and independently controlled
factory/argument failures. Actual recovered Java matches the reference JVM for:

- normal return 3 with A9,R,A1,A2,S;
- null receiver failure after A9,R,A1,A2, with no sum effect;
- first argument failure after A9,R,A1;
- second argument failure after A9,R,A1,A2;
- factory failure after A9,R.

These are public Engine method-in-context observations, not extra fixed-case
passes. Separate original/actual source and compilation artifacts and all five
independent comparisons remain in observations.json.

```sh
target/release/examples/inspect_class .cache/e2e/t11-effectful-arguments/dex/classes.dex 0 'Lfixtures/Case;' target '()I' --catalog .cache/e2e/t11-effectful-arguments/complete.catalog.json --output .cache/e2e/t11-effectful-arguments/public.json
python3 tools/e2e.py run --case D018 --case D019 --case D022 --case D023 --case D071
python3 tools/e2e.py run --case D018 --case D019 --case D022 --case D023 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064 --case D077 --case D078
python3 tools/e2e.py run
```

The same fixed D018 bytes with partial directories, a missing/generic result
class, unknown factory/sum throws, a return-only factory collision or a shadowed
qualifier retain Debug/BindingEvidenceMissing. Unknown facts are not promoted to
a successful type or dispatch proof.

On macOS arm64, the complete `.cache/e2e/engine/run-z3gcj7lt/report.json` retains
86 mandatory cases, 177 invocations and 181 seams: 40 whole cases and 94 seams
passed. The command exited 1 for remaining failures. D077's public method is
Java, but its existing context adapter requests an absent generated Case.java
path; that execution failure is not a pass. D078 remains Mixed pending class
production. Next action: resolve D077's certified generated-source location in
the thin context adapter, without inventing source from engine output.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Existing fixed core inputs/expectations were
reused; no new tests, auxiliary-tool self-tests, fingerprint gates or TASKS.md
changes were introduced. T11 remains pending for checked throws and its remaining
required object/class scope.
