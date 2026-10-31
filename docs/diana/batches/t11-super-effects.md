# T11 super binding with ordered effects

Goal: execute D021's original prefix log, invoke-super and result arithmetic
without virtualizing super or moving effects. Contracts: ALGORITHMS §3/§9.1/§9.4
and TASKS T11. Initialized ordinary this is distinct from constructor UninitThis;
constructors remain outside this producer. Existing call-kind, primitive and
resource observations remain regressions.

The closed receiver producer now distinguishes initialized ordinary this from a
static parameter. It rejects constructors before seeding this and keeps original
receiver aliases. Super binding requires the exact direct superclass, a complete
plain non-interface/nonfinal parent directory and a public, nonabstract/nonfinal
own target with known empty throws. Local original flags/hierarchy corroborate
catalog facts; an inherited or interface-special target is not guessed.

The producer retains ordered string definitions and String-only static void
calls before the receiver call. It materializes literals at their original
statement position rather than moving resolution across effects. A single
original int literal addition after the adjacent result remains after the call.
P8 checks builtin scope and constant UTF-8 limits and owns the source/facts.
Its independent replay walks original instructions and register definitions,
checks exact prefix coverage, original method/string references, receiver kind,
argument roots, adjacent result and the actual returned definition. P9 emits
super rather than virtual dispatch. Public call facts include both prefix calls
and the original Super site in program order.

On macOS arm64, `.cache/e2e/t11-super-effects/method-final.json` records actual
Java for D021 Child.f. Compiling that actual method in its unchanged known class
context and running the original certified Driver matches the independent
reference: int 2, no exception, events C then B. Sources, javac log and
actual-driver.json remain under that directory. This method-in-context evidence
is not a whole-class or additional fixed-case pass.

```sh
target/release/examples/inspect_class .cache/e2e/prepared/D021/dex/classes.dex 0 'Lfixtures/Child;' f '()I' --catalog .cache/e2e/engine/run-43wq5mib/D021/v1/decompile-class/method-signatures.json --output .cache/e2e/t11-super-effects/method-final.json
.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home/bin/javac --release 8 -proc:none -g:none -cp .cache/e2e/prepared/D021/reference/classes -d .cache/e2e/t11-super-effects/actual .cache/e2e/t11-super-effects/context/fixtures/Child.java
.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home/bin/java -cp .cache/e2e/t11-super-effects/actual:.cache/e2e/prepared/D021/reference/classes:.cache/e2e/prepared/D021/reference/driver-classes fixtures.Driver v1
```

Actual public Engine operations with wrong-parent, abstract-parent-target,
unknown parent/prefix throws, generic caller or shadowed java catalogs retain
Debug/BindingEvidenceMissing. Existing private/virtual supplemental public
operations still admit Java after the independent replay was added.

`.cache/e2e/engine/run-60csltiv/report.json` records all ten related selected cases
and 22 invocations passing. The final complete report is
`.cache/e2e/engine/run-u4r4wia4/report.json`: 86 mandatory cases, 177 invocations
and 181 seams retained; 39 whole cases and 93 seams passed. D021's mandatory
class request remains Mixed because its constructor and JavaUnit assembly remain
unimplemented. The complete command exited 1; its failed class requirement was
not converted into method acceptance.

```sh
python3 tools/e2e.py run --case D021 --case D023 --case D019 --case D022 --case D071
python3 tools/e2e.py run --case D023 --case D019 --case D022 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Existing core inputs, context and Driver were
reused; no new tests, auxiliary-tool self-tests, fixed input/expected edits,
fingerprint gates or TASKS.md changes were introduced. T11 remains pending for
broader effectful receivers/arguments, checked throws and required class scope.
