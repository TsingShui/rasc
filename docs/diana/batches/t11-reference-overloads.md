# T11 typed null and reference overloads

Goal: execute D019 using the original Object formal, not the more-specific String
null overload, while retaining original dispatch identity and effects. Contracts:
ALGORITHMS §9.1/§9.4 and TASKS T11. Explicit pure null views introduce no runtime
check; unknown/non-literal reinterpretation stays Unsupported. Existing static,
void-effect, field-constant and primitive observations remain regressions.

The static binder enumerates the complete plain own-name group against the
original target formal vector. It checks fixed-arity identity/widening, rejects
equal-vector return collisions and unresolved generic/bridge/varargs candidates,
and retains the original owner/name/proto. This is conditional on P5/P8 fixing
each source argument to that vector; it does not guess a winner from printed text.
Inherited/hiding groups beyond the admitted scope remain unproven.

P8 now owns typed null and proven Object-upcast views. Null is admitted only for
an original pure zero literal; an arbitrary non-literal int is not a reference.
The source emits `(java.lang.Object) null` for the Object target, so the String
overload does not steal the call. Reference call results retain their original
move-result-object site, String/Object storage identity and return type. The
independent SSA/dispatcher checkers validate those sites, argument views and
original method identity. Builtin type spellings are checked against source-scope
shadowing. No additional runtime check or helper supplies a result.

On macOS arm64, `.cache/e2e/engine/run-4er1_a5n/report.json` records the complete
mandatory set: 86 cases, 177 invocations and 181 seams retained; 38 whole cases
and 91 seams passed. D019 passed actual compilation/JVM comparison of the return
and pick:Object event. The command exited 1 for the remaining real capabilities
and application seams. `run-dzw76n2w` separately passed all ten selected cases and
24 invocations, including existing void, narrow, field guard and resource cases.

```sh
python3 tools/e2e.py run --case D019 --case D022 --case D071 --case D080 --case D015 --case D014 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

The same original D019 input was submitted to the public Engine with seven
catalog variants in `.cache/e2e/t11-reference-overloads/`. Complete evidence gave
Java with the explicit Object null view. Partial directories, return-only
collision, generic or varargs candidate, unknown target throws and a shadowed
java qualifier all retained Debug/BindingEvidenceMissing. These are core binding
observations, not auxiliary-tool self-tests or extra fixed-case passes.

```sh
target/release/examples/inspect_class .cache/e2e/prepared/D019/dex/classes.dex 0 0 target '()Ljava/lang/String;' --catalog .cache/e2e/t11-reference-overloads/complete.catalog.json --output .cache/e2e/t11-reference-overloads/complete.json
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Existing fixed core coverage was reused; no new
tests, auxiliary-tool tests, fixed input/expected edits, fingerprint gates or
TASKS.md changes were introduced. T11 remains pending for its remaining dispatch,
private/super and checked-throws obligations.
