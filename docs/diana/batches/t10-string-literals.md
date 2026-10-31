# T10 original UTF16 string literals

Goal: recover D076's below/at-boundary values and classify its over-limit value
using original per-u16 MUTF8 length. Contracts: ALGORITHMS §4.8 and TASKS T10.
The string load stays at its original definition and is not rematerialized per
use. Existing numeric, boolean/null, narrow and resource evidence remain
regressions.

MethodInput resolves a typed StringId through its own logical ProgramIndex. The
scalar producer copies original UTF16 units with charged allocation and retains
the const-string at its original definition. P5 treats it as a single reference
producer, not a pure numeric literal view. String moves and returns keep the
Java8 builtin String type. The owned AST has no input borrow.

P8 checks every string constant's MUTF8 bytes against the actual CONSTANT_Utf8
limit using the same per-unit rule as target names/descriptors. It emits typed
JavaTargetLimit/ConstantUtf8Bytes on overflow. P9 escapes units without combining
surrogates or normalizing text; NUL uses octal, CR/LF use language escapes, and
backslash cannot create an unintended Unicode escape. No chunking/helper changes
intern identity or hides a target failure.

On macOS arm64, `.cache/e2e/engine/run-5xzedk_w/report.json` records 26 selected
cases and 65 invocations: 25 whole cases and 64 seams passed. All three D076
boundaries passed, including exact complete u16-value comparison for both Java
positives and independent original Debug comparison for the over-limit role.
D071 call-byte remains the single non-pass; the command exited 1.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D007 --case D009 --case D010 --case D011 --case D012 --case D013 --case D014 --case D015 --case D016 --case D017 --case D055 --case D056 --case D057 --case D058 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D071 --case D072 --case D076
```

The thin known-context adapter accepts a String return and reuses the prepared
reference holder for a generated input role. It never derives program values
from the expected result. No preparation or source generator was changed.

A supplemental original DEX in `.cache/e2e/t10-string-literals/escapes.dex` uses
an authored 22-unit vector containing NUL, a literal backslash-u sequence, quote,
line/control characters, isolated surrogates, a surrogate pair and line separators.
Actual recovered source compiled under the locked JDK/Java8 target; the JVM's
complete charAt hex vector equalled `independent-units.json`. The existing D076
builder initially rejected isolated surrogates in its Python text encoder. The
local input recipe reused it with surrogatepass unit extraction; the repository
builder and frozen inputs remain unchanged. This is actual core-input execution,
not a builder self-test or another fixed-case pass.

```sh
target/release/examples/inspect_class .cache/e2e/t10-string-literals/escapes.dex 0 0 f '()Ljava/lang/String;' --output .cache/e2e/t10-string-literals/actual.json
.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home/bin/javac --release 8 -proc:none -g:none -d .cache/e2e/t10-string-literals/classes .cache/e2e/t10-string-literals/context/fixtures/Case.java .cache/e2e/t10-string-literals/context/Driver.java
.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home/bin/java -cp .cache/e2e/t10-string-literals/classes Driver
```

The same public operation with max_method_output_bytes=16 retained complete raw
and ResourceLimitExceeded for the actual escaped source, with no executable Java
or successful truncation (`output-limited.json`). Workspace fmt, all-target/all-
feature Clippy with warnings denied, locked release build and all 116
cargo-nextest core tests with zero skips passed. Diff and sensitive-input checks
passed before commit. No new tests, fingerprint gates, fixed input/expected
changes or TASKS.md edits were introduced.
