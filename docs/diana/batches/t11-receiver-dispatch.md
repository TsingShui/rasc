# T11 initialized-parameter receiver dispatch

Goal: execute D023 with exact interface binding and null behavior, using an
initialized nominal parameter as the receiver. Contracts: ALGORITHMS §9.1/§9.4
and TASKS T11. The closed producer preserves the original invoke/result/return
and only consumes pure receiver aliases; unknown hierarchy, dispatch or scope
facts retain raw. Existing static, overload, void and field-guard cases remain
regressions.

The closed receiver producer accepts one initialized nominal parameter, pure
move-object aliases, one zero-argument virtual/interface invoke, its adjacent
int result and the matching return. No allocation, default/null seed, branch,
handler or skipped effect enters that product. P4 requires the receiver's exact
static owner type, complete plain declaration/access/throws scope, matching
original local flags/hierarchy and an unambiguous source-visible method group.
The Java receiver remains the parameter; it is not replaced with a concrete
implementation or a static call. A null receiver therefore fails at the call.

The owned verified AST retains original method identity, invoke/result/return
origins and receiver register. Public `VerifiedJavaMethod::calls` now exposes
borrowed verified call facts without retaining engine/input data; the existing
public development example records those facts and the method EnvironmentId.

On macOS arm64, `.cache/e2e/engine/run-me1_dbf8/report.json` records the complete
mandatory set: 86 cases, 177 invocations and 181 seams retained; 39 whole cases
and 93 seams passed. Both D023 invocations passed actual compiler/JVM comparison
of implementation dispatch and null behavior. The command exited 1 for remaining
real capability/application failures. `run-da1xel9l` separately passed all ten
selected cases and 22 invocations.

```sh
python3 tools/e2e.py run --case D023 --case D019 --case D022 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

Supplemental actual core execution is under
`.cache/e2e/t11-receiver-dispatch/`. An authored static f(Base) calls the virtual
Base declaration. Locked javac/D8 produced the original input, the existing
classfile reader supplied declarations, and recovered Java matched the independent
reference JVM for Base, Child override and null. The complete observation was
`1,2,java.lang.NullPointerException`; original and actual sources/logs remain
separate. This is method-in-known-context evidence, not another fixed-case pass.

The original D023 input also ran through complete/partial directory, wrong owner
kind, private target, unknown throws and missing-parent catalog variants. Only
complete facts admitted Java; all other modes retained Debug/BindingEvidenceMissing.
A self-made mutation increases registers_size while leaving the original receiver
word unassigned; the public Engine reported Malformed/UndefinedRegister and no
Java. Only format-defined DEX header fields were populated after that local
mutation; no fingerprint gate was added.

```sh
target/release/examples/inspect_class .cache/e2e/t11-receiver-dispatch/dex/classes.dex 0 'Lfixtures/Case;' f '(Lfixtures/Base;)I' --catalog .cache/e2e/t11-receiver-dispatch/virtual.catalog.json --output .cache/e2e/t11-receiver-dispatch/virtual.json
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Fixed core coverage and existing input/classfile
producers were reused; no new tests, auxiliary-tool self-tests, fixed expected
changes or TASKS.md edits were introduced. T11 still needs its broader effectful
receiver/arguments, private/super and checked-throws obligations.
