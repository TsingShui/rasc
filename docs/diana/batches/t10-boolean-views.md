# T10 boolean views

Goal: recover D013 and D014's literal boolean method with finite Known01 facts,
keeping boolean source storage separate from integral word operations.
The same batch now consumes D014's original Object/null branch.
Contracts: ALGORITHMS §4.3–§4.6 and TASKS T10. An unproven int-to-boolean use
remains Unsupported. Existing numeric, word-pair and CFG cases remain regressions.

P5 keeps the Integral32 family separate from the selected boolean source type.
Descriptor booleans retain boolean storage. Numeric word uses have explicit
BooleanToInt views; scalar Known01 facts authorize the reverse direction only
when the final non-empty domain excludes Other. Pure literal 0/1 select their
own boolean views instead of constraining unrelated literal uses.

The ordinary CFG producer admits the Java8 builtin Object descriptor for null
comparisons. It preserves typed Object locals/copies and rejects ordered ref
comparisons. Its constant/move/Phi bit closure supports the original boolean
return. P8 owns exclusive direct/BooleanWord/Boolean01 return nodes; the
dispatcher checker checks the result type and Known01 evidence independently.
P9 emits null, not integer zero, for a reference zero comparison.

On macOS arm64, `.cache/e2e/engine/run-t2f5zgzr/report.json` records all 26 selected
cases and 58 invocations passing. Every D013/D014 invocation passed actual source
compilation and JVM comparison, along with prior numeric, loop, switch,
malformed-wide and resource observations.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D004 --case D005 --case D007 --case D008 --case D009 --case D010 --case D011 --case D012 --case D013 --case D014 --case D016 --case D017 --case D055 --case D056 --case D057 --case D058 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D083
```

Supplemental core observation: copy the certified D013 bytes and change the
code-unit at byte 262 from `0x10b5` (and-int/2addr) to `0x10b0` (add-int/2addr).
The resulting return can carry 2, so it has no Known01 proof. The public Engine
operation below retained complete original words with an Unsupported diagnostic
and no Java. Only format-defined DEX header fields were populated after this
local mutation; no input fingerprint gate was introduced. This observation is
not an added fixed-case pass.

```sh
target/release/examples/inspect_class .cache/e2e/t10-boolean-views/non01.dex 0 0 f '(ZZ)Z' --output .cache/e2e/t10-boolean-views/non01.json
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 116 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. No new tests, fixed inputs, independent expectations
or TASKS.md edits were introduced. T10's remaining matrix stays pending.
