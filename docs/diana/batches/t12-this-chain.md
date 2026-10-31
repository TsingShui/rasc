# T12 this-constructor delegation

Goal: retain D042's this(4) delegation as the first Java statement and reject
recursive or unproven constructor chains. Contracts: ALGORITHMS §9.2–§9.3 and
TASKS T12. Chain inspection reads only the required sibling prologues, not their
unrelated bodies; no super call or initialization effect is deleted.

P1 exposes a sealed constructor link from bounded original code ranges. It
checks int entry words, register bounds, pure moves/literals, the uninit-this
receiver, method identity and invocation words before returning a this/super
edge. It stops at the required chain and does not validate an unrelated later
body. Protected or more complex prefixes remain unproven.

P4 distinguishes allocation, super and same-class this access. This permits
private own constructors while keeping the complete candidate group, original
flags/proto and known throws checks. An iterative, work-charged traversal follows
original constructor IDs, rejects a revisit or unresolved prefix and proves a
terminal bound superclass call. It does not recurse on untrusted chain depth.
P8 rechecks the chain before owning the constructor AST; P9 emits this rather
than substituting super. Parameter/local availability still begins after normal
chain completion.

On macOS arm64, D042's actual selected no-arg constructor compiles in its
unchanged known class context and matches the original Driver: n=4 and INT:4 then
ZERO. Its int constructor remains independently Raw because its field/body
producer is pending; the mandatory class request remains Mixed. Actual source,
javac log and actual-driver.json are in `.cache/e2e/t12-this-chain/d042/`.

```sh
target/release/examples/inspect_class .cache/e2e/prepared/D042/dex/classes.dex 0 'Lfixtures/Case;' '<init>' '()V' --catalog .cache/e2e/engine/run-5pbnbb5o/D042/v1/decompile-class/method-signatures.json --output .cache/e2e/t12-this-chain/d042/public.json
python3 tools/e2e.py run --case D042 --case D041 --case D043 --case D018 --case D023
```

Supplemental authorized inputs are under `.cache/e2e/t12-this-chain/`. A
javac/D8 three-constructor program validates private multilevel delegation:
actual/reference JVMs agree on value 9 and TWO→ONE→ZERO, or an escaping
fixtures.Failure with only TWO. Sources, compiler logs and observations.json stay
separate in private/. Locked smali inputs for direct recursion, an indirect
cycle and a sibling's effectful pre-chain preparation all retain complete
Debug/InitializationUnavailable.

A separately anchored mutation changes a sibling's post-chain const-string
register from 2 to 255 in a three-register method. Independent physical and
dexlib projections identify that original method/pc. The selected delegating
constructor remains Java; the class report independently marks original method
ID 1 Unavailable/Malformed(InvalidRegisterSpan) while preserving the delegating
member. Optional declaration facts are not used to relabel that failure; the
assertion uses its original method ID. The anchor and selected/class public
reports remain in private/. Only original format-defined DEX header fields are
populated after mutation; no content fingerprint gate is added.

```sh
target/release/examples/inspect_class .cache/e2e/t12-this-chain/private/dex/classes.dex 0 'Lfixtures/Case;' '<init>' '()V' --catalog .cache/e2e/t12-this-chain/private/catalog.json --output .cache/e2e/t12-this-chain/private/public.json
python3 tools/e2e.py run --case D043 --case D018 --case D019 --case D022 --case D023 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

`.cache/e2e/engine/run-g321glus/report.json` passes all twelve related selected
cases and 24 invocations. The complete final
`.cache/e2e/engine/run-irh4esrt/report.json` retains 86 mandatory cases, 177
invocations and 181 seams: 42 whole cases and 96 seams passed; its command exits
1 for remaining failures. Supplemental method/context evidence is not another
fixed case or JavaUnit pass.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff and sensitive-input
checks passed. Existing fixed coverage was reused; no new tests, auxiliary-tool
tests, fixed input/expected changes or TASKS.md edits were introduced. T12 remains
pending for its post-chain field/body and whole-class obligations.
