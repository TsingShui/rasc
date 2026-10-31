# T11 exact checked-throws propagation

Goal: retain original DEX Throws declarations and admit checked calls only when
the original static caller contract covers their declared escaping types.
Contracts: ALGORITHMS §9.1/§9.5 and TASKS T11. Unknown facts and incompatible
contracts retain raw; no helper, catch wrapping or widened declaration supplies
successful Java. Existing core call-kind and resource observations remain
regressions.

P1 now exposes sealed original Throws metadata from the selected method's
checked annotation set. It recognizes only the system Throws/value/type-array
shape, retains other-annotation presence and rejects a malformed recognized
shape with its original input offset. Other members remain independent.

P4 retains known declared callee throws in the sealed contract. A static caller
with a nonempty throws declaration requires matching original DEX and covered
classfile declarations, a plain direct Object superclass with no conflicting
inherited name, known Throwable ancestry and faithfully resolvable type names.
Cycle/missing facts reject binding. The escaping checked set is covered by the
original declared types via known inheritance; known RuntimeException/Error
subtypes do not require a new declaration. P8 rechecks the retained set in the
closed no-handler call-chain profile and emits the original declaration, not a
new throws inferred from the callee. Handler subtraction and instance override
contracts remain unproven. A local Clippy allowance keeps the transient member
result inline rather than introducing another fallible AST allocation.

Supplemental authorized input is under `.cache/e2e/t11-checked-throws/`. Locked
javac/D8 compile an original caller declaring Exception and a callee declaring
its Problem subtype. Four named locked-JDK class entries supply the exception
ancestry; no JDK/cache scan or new fingerprint gate is used. Actual recovered Java
retains `throws java.lang.Exception` and matches the independent reference JVM:
return 7/calls=1 or escaping fixtures.Problem/calls=1. A separate declared
RuntimeException program preserves an empty caller throws declaration and
matches both normal and throwing executions. Source, original/actual compiler
logs and the four independent JVM observations remain separate.

```sh
target/release/examples/inspect_class .cache/e2e/t11-checked-throws/dex/classes.dex 0 'Lfixtures/Case;' target '()I' --catalog .cache/e2e/t11-checked-throws/complete.catalog.json --output .cache/e2e/t11-checked-throws/checked-final.json
target/release/examples/inspect_class .cache/e2e/t11-checked-throws/unchecked/dex/classes.dex 0 'Lfixtures/Case;' target '()I' --catalog .cache/e2e/t11-checked-throws/unchecked/catalog.json --output .cache/e2e/t11-checked-throws/unchecked-final.json
```

Public operations with unknown caller/callee throws, a changed caller contract,
uncovered Throwable, missing/cyclic exception ancestry, generic caller or a
shadowed java name retain Debug/BindingEvidenceMissing. A self-made original DEX
without the caller Throws association likewise stays Debug instead of acquiring
a widened declaration. An independently anchored encoded Throws array element
changed from Type to int produces Malformed/InvalidEncodedValue and no Java.
The physical directory/element anchors and original/new fields are recorded;
only format-defined DEX header fields are populated after those local mutations.
These observations are native core behavior, not auxiliary-tool self-tests or
additional mandatory-case passes.

```sh
python3 tools/e2e.py run --case D018 --case D019 --case D022 --case D023 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

On macOS arm64, `.cache/e2e/engine/run-tfcwis_k/report.json` passes all eleven
related selected cases and 23 invocations. The complete
`.cache/e2e/engine/run-esej6lnz/report.json` retains 86 mandatory cases, 177
invocations and 181 seams: 41 whole cases and 95 seams passed; the command exited
1 for remaining failures. The final public operations preserve the exact sources
used by the successful JVM comparisons. Class/constructor obligations remain
pending; this method evidence is not JavaUnit acceptance.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Existing fixed core coverage was reused; no new
tests, auxiliary-tool tests, fixed inputs/expected changes or TASKS.md edits were
introduced. The call-contract producer artifacts are ready for the dependent
allocation/init work; remaining T11 object/class acceptance stays pending.
