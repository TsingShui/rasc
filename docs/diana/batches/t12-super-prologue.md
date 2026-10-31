# T12 restricted super constructor prologue

Goal: recover D041's super-before-body constructor method and permit only
parameter moves/numeric literals before its single legal super call. Contracts:
ALGORITHMS §9.2–§9.3 and TASKS T12. UninitThis is not an ordinary Ref before
normal chain completion; no Java local declaration or helper precedes super.

P4 now shares public constructor binding between allocation and a direct
superclass chain, with separate instantiability rules. Super requires the exact
original/source superclass, corroborated caller/target flags and plain complete
scope; an abstract superclass is not confused with instantiating an abstract
class. A final/interface/enum parent or unresolved access/throws stays raw.

The closed constructor producer seeds UninitThis and explicit int parameter
roots. Only pure parameter moves, numeric literals and this aliases precede its
one chain. It folds those roots into super arguments, then makes this aliases
initialized only on normal completion. P8 independently replays every original
register/root, callee/proto, argument and body effect before owning the AST. P9
prints super as the first Java statement and emits body locals/effects only
afterwards. Shared owned prefix conversion/emission also preserves the existing
ordinary receiver path. Other annotations, handlers, this-delegation and broader
constructor bodies remain unproven.

D041's actual selected constructor, compiled in its unchanged known class
context, matches the original independent Driver: BASE then BODY, no exception.
The source and compiler/Driver records remain in
`.cache/e2e/t12-super-prologue/d041/`. D021 additionally exposes both its original
constructor and f method as Java members. Their required class requests remain
Mixed; no method evidence is counted as JavaUnit acceptance.

```sh
target/release/examples/inspect_class .cache/e2e/prepared/D041/dex/classes.dex 0 'Lfixtures/Case;' '<init>' '()V' --catalog .cache/e2e/engine/run-204u0l3h/D041/v1/decompile-class/method-signatures.json --output .cache/e2e/t12-super-prologue/d041/public.json
python3 tools/e2e.py run --case D041 --case D043 --case D018 --case D021 --case D023
```

Supplemental authorized input is under
`.cache/e2e/t12-super-prologue/arguments/`. Locked smali makes the original
parameter move explicit, overwrites its physical input register with literal 4,
and aliases UninitThis before a Base(int,int) chain. The recovered first statement
is `super(p0, 4)` with no preceding local. Independent reference/actual JVM runs
agree on value 7:4 and BASE:7:4→BODY. When the superclass constructor throws,
both preserve fixtures.Failure and BASE:7:4 without BODY. Original/actual sources,
compiler logs and observations.json remain separate.

Eight original shapes retain complete Debug/InitializationUnavailable: pre-super
field read/write, effectful argument, branch, monitor, string resolution, a
second init and a clobbered this receiver. These are core input/Engine operations,
not auxiliary-tool self-tests or additional fixed-case passes.

```sh
target/release/examples/inspect_class .cache/e2e/t12-super-prologue/arguments/pure/classes.dex 0 'Lfixtures/Case;' '<init>' '(I)V' --catalog .cache/e2e/t12-super-prologue/arguments/catalog.json --output .cache/e2e/t12-super-prologue/arguments/pure/public.json
python3 tools/e2e.py run --case D043 --case D018 --case D019 --case D022 --case D023 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

On macOS arm64, `.cache/e2e/engine/run-x31z42lt/report.json` passes all twelve
related selected cases and 24 invocations. The complete final
`.cache/e2e/engine/run-0eq4yz1r/report.json` retains 86 mandatory cases, 177
invocations and 181 seams: 42 whole cases and 96 seams passed; the command exited
1 for remaining failures. Workspace fmt, all-target/all-feature Clippy with
warnings denied, locked release build, all 117 cargo-nextest core tests with zero
skips, diff and sensitive-input checks passed. Existing fixed coverage was reused;
no new tests, auxiliary-tool tests, fixed input/expected changes, fingerprint
gates or TASKS.md edits were introduced.

T12 remains pending for this-delegation, post-super field/body obligations and
required whole-class evidence. Next action: add the this-chain binding and its
cycle/availability checks without treating it as a superclass call.
