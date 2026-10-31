# T12 adjacent allocation and initialization

Goal: recover D043's prepared-argument new/init pair and D020's initialized
receiver without moving effects across allocation. Contracts: ALGORITHMS §9.2–§9.3
and TASKS T12. T11's committed call/throws producer artifacts supply binding;
its remaining shared class acceptance is still pending. Pre-init aliases have
no readable Java local, and initialization failure does not materialize one.

P4 pairs one original new-instance with its unique direct constructor, consuming
only pure uninit aliases in between. It checks exact allocation owner/proto,
public constructor accessibility, the complete plain candidate group and known
throws compatibility. Intervening effects, uninit argument/ordinary uses,
cross-block preparation, a wrong owner or a second init reject Java explicitly
with InitializationUnavailable. Arguments must already be defined int values
outside the uninit alias set. An overwritten argument register cannot reuse its
old definition.

The closed call-chain producer treats the pair as one consumed allocation atom,
retaining AllocationId/new/init origins. Its materialized value is not a new
physical init-register write or a global mutation of an old ValueType. P6's
sealed DeferredAllocation binding emits no readable pre-init local or default
null; normal completion maps all aliases to the same initialized value. P8
independently replays the original type/reference, every move and alias kill,
constructor words, argument definitions and completion position before source
emission. Public call facts retain the original Direct constructor reference,
receiver register and allocation/init sites, without fabricating move-result.

Known class inheritance additionally permits only proven receiver/return upcasts.
D020 retains a Child allocation and an explicit Base source receiver, so the
original virtual Base reference still dispatches to Child. Generic, hidden or
missing hierarchy facts do not supply a guessed cast. The thin existing context
adapter now recognizes the reference return spelling used by D043; it still
requires one unambiguous original method and actual compilation.

On macOS arm64, `.cache/e2e/engine/run-k2w8w8lx/report.json` passes all twelve
related cases and 24 invocations, including D043's real javac/Driver comparison
of ARG then CTOR:5 and the returned box.n=5. D020's actual target method separately
compiles/runs in its unchanged known context and returns 2; its mandatory class
request remains Mixed. No method evidence is counted as JavaUnit acceptance.

```sh
python3 tools/e2e.py run --case D043 --case D020 --case D018 --case D019 --case D022 --case D023 --case D071
python3 tools/e2e.py run --case D043 --case D018 --case D019 --case D022 --case D023 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

Supplemental authorized inputs are under
`.cache/e2e/t12-adjacent-allocation/`. Locked smali assembles explicit pure-alias
and post-init register-overwrite programs; actual recovered Java matches the
independent D043 Driver in both cases. Seven separate original shapes retain
complete Debug/InitializationUnavailable: effectful preparation, ordinary uninit
use, duplicate init, alias overwriting the argument, a branch before init, uninit
return and wrong-owner init. These are core input/Engine observations, not tool
self-tests or extra fixed-case passes.

A separately authored javac/D8 program preserves ARG→CLASS→CTOR:5 both on normal
return and when its constructor publishes this then throws. Actual recovered Java
matches the independent reference's return/exception, event order and published
field. The caller's deferred local is not materialized on the throwing path.
Original/actual sources, compiler logs and observations.json remain separate in
failure/. The D020 method observation is retained in d020/.

```sh
target/release/examples/inspect_class .cache/e2e/t12-adjacent-allocation/alias/classes.dex 0 'Lfixtures/Case;' f '()Lfixtures/Box;' --catalog .cache/e2e/engine/run-fzf6tbw2/D043/v1/decompile-method/method-signatures.json --output .cache/e2e/t12-adjacent-allocation/alias/public.json
target/release/examples/inspect_class .cache/e2e/t12-adjacent-allocation/failure/dex/classes.dex 0 'Lfixtures/Case;' f '()Lfixtures/Box;' --catalog .cache/e2e/t12-adjacent-allocation/failure/catalog.json --output .cache/e2e/t12-adjacent-allocation/failure/public.json
```

The complete `.cache/e2e/engine/run-0_x7gcj8/report.json` retains all 86 mandatory
cases, 177 invocations and 181 seams: 42 whole cases and 96 seams passed. The
command exited 1 for remaining failures. Workspace fmt, all-target/all-feature
Clippy with warnings denied, locked release build, all 117 cargo-nextest core
tests with zero skips, diff and sensitive-input checks passed. Existing fixed
coverage was reused; no new tests, auxiliary-tool tests, fixed input/expected
changes, fingerprints or TASKS.md edits were introduced.

T12 remains pending. Next action: implement the restricted UninitThis super/this
prologue and its actual constructor-body obligations, preserving the first Java
statement and rejecting complex pre-super preparation.
