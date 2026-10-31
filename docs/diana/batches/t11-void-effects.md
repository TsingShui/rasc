# T11 void and discarded-call effects

Goal: execute D022 with its original log, counter and escaping exception, keeping
void exits distinct from value returns and preserving unused calls. Contracts:
ALGORITHMS §9.1/§9.5 and TASKS T11. Existing static binding, primitive, CFG and
resource cases remain regressions.

The CFG result type now distinguishes Void from a typed value. A void terminal
has no register, definition or successor, and P8/P9 retain a separate void-return
node. Static call statements still execute before that exit even without a
result local. The independent dispatcher checker validates the original void
contract instead of accepting a fabricated int return.

Source-scope admission now distinguishes class annotations from annotations on
other members in the same directory. A generic field does not invent a generic
or enclosing class scope; original class-level evidence remains independently
checked. The thin known-context adapter accepts a void method header.

The selected-input validator also checks known original method flags against the
invoke family before Java/catalog selection. A known static/instance mismatch is
Malformed/InvalidInvokeKind even if its raw receiver/argument word counts fit.
Existing invoke tests covered layout/arity, not this semantic admission path.
One minimal table-driven public MethodInput regression fills that fixed M10 gap,
including ordinary and range formats. It adds no CLI or auxiliary-tool tests.

On macOS arm64, `.cache/e2e/engine/run-huhotmlg/report.json` records all 23 selected
cases and 55 invocations passing. D022's actual recovered caller compiled and
preserved the B event, counter write and escaping ExpectedError through the
existing JVM driver. D071 remains a passing whole case; prior primitive, CFG,
malformed and resource observations remain passing.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D009 --case D013 --case D014 --case D015 --case D016 --case D017 --case D022 --case D055 --case D056 --case D057 --case D058 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D071 --case D072 --case D076
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. The targeted new regression run passed one test
with 116 unselected; only the later full run supplies full core evidence.
Fixed inputs, independent expectations, manifest and TASKS.md are unchanged.
T11's remaining dispatch/overload/access/throws and field-constant obligations
stay pending.
