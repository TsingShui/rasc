# T15 FreshArray stores

Goal: recover required-Java D027 with a real allocation/alias/no-escape certificate,
original-site length read and ordered exact primitive stores. ALGORITHMS §2.4 and
§4.8 own the obligations. Keep D074 raw; successful shape precheck alone is not
permission to lower a fill. Reuse existing literals and fixed independent arrays.

P3 now records one original primitive allocation and fill in a static, no-parameter
returning method, retaining numeric definitions, allocation identity, type reference,
length definition and aliases. P6 alone constructs the sealed FreshArray product:
its independent original-word replay checks every definition, source register,
allocation position/type, all aliases, single-entry linear chain, sufficient
non-negative constant length, absence of escape/barriers and the returned identity.
An invalid certificate is InternalInvariant rather than an unsupported-pattern
success. The original declared result is checked against the actual returned array;
a proven incompatible primitive result is Malformed/InvalidResultKind.

P1 supplies a borrowed checked payload iterator excluding padding. P5 seals exact
literal bits by the known component, including signed narrow values, unsigned char,
wide integers, FP negative zero/subnormal/Infinity and Boolean 0/1. P8 compares each
sealed literal with its original element and retains an owned body with no input
borrow. Non-exact Boolean/NaN payloads stay complete raw/ArrayFillShape. P9 keeps
allocation separate, reads length at the fill site and emits stores in payload
order; it does not merge new/fill into an initializer or add a bit-conversion helper.

```sh
python3 tools/e2e.py run --case D027 --case D073 --case D074
python3 tools/e2e.py run
```

On macOS arm64, `run-5jalz82o` passes all three selected fixed cases / eight
invocations. Both D027 methods are required Java and independently JVM-match their
int/float arrays; D073 retains NPE/no-write behavior and all four D074 shapes retain
complete raw with ArrayFillShape.

Authorized supplemental Smali/Java in `.cache/e2e/t15-fresh-array/` covers all
eight primitive components plus an alias that survives overwriting the original
allocation register. Its allocation length exceeds the payload count, so the
independent result [7,8,0,0] also checks preserved zero suffix and allocation size.
All nine actual methods compile and JVM-match independent full values/FP bits,
including signed extremes, char bounds, negative zero, smallest subnormals and
positive/negative Infinity. Two non-exact payloads remain Unsupported raw, while
an incompatible actual return remains Malformed. Separate original/actual sources,
locked compiler commands/logs and observations.json retain this supplemental core
evidence; these are not extra fixed passes or auxiliary-tool tests.

The complete final `run-4wuua3k5` retains 86 required cases / 177 invocations /
181 seams: 47 whole cases / 109 seams pass. Only D027's two seams are newly passing
relative to run-a6jolxi7; no pass is lost. The full command exits 1 for genuine
remaining obligations. Fmt, strict all-target/all-feature Clippy, locked release,
all 118 cargo-nextest core tests with zero skips, diff and sensitive-input checks
pass. Existing core coverage was reused. TASKS.md, inputs and independent expected
values are unchanged.

T15 remains pending. Next action: implement typed array access with a materialized
index/effect prefix for D024 and runtime reference-array stores for D026.
