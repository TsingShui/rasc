# T12 post-chain own int field writes

Goal: preserve a simple own-field assignment after normal constructor chaining,
without moving it into an initializer or permitting uninit-this access.
Contracts: TASKS T12 step 5 and ALGORITHMS §9.2–§9.3. Exact field ownership,
source-name uniqueness and original flags must corroborate declaration evidence;
final-field initialization remains unproven here.

P1 now projects original own-field flags and same-name counts without reading
method bodies. Its instruction validation also checks known original local
field staticness against iget/iput versus sget/sput before catalog-dependent
analysis; mismatches are Malformed/InvalidFieldKind even with an empty catalog.

P4 seals an own int write only when the original field ID, owner, type, unique
source name and complete declaration evidence agree, including exact flags.
Static/final writes and conflicting/absent source facts do not produce Java.
Volatile fields keep their actual modifier and ordered assignment in the known
class context. The constructor producer permits the write only through an
InitializedThis alias after normal chain completion. Pure int formal/literal
roots are retained; P8 independently checks the original word/reference and
value roots. P9 emits this.field assignment in original effect order, not a
field initializer. Shared effect records now live in a named producer module.

On macOS arm64, supplemental authorized input is under
`.cache/e2e/t12-post-chain-fields/`. A javac/D8 subclass writes an ordinary int
and a volatile int after its superclass constructor, then logs BODY. The
superclass observes/publishes this before returning. Actual recovered Java
matches independent reference JVMs:

- normal value and published fields 7:3, with BASE:0:0 then BODY;
- escaping fixtures.Failure, published fields still 0:0, with only BASE:0:0.

Original/actual sources, compiler logs and both observations stay separate. The
superclass failure does not execute the subclass writes. These are native core
method/context observations, not extra fixed-case or JavaUnit passes.

```sh
target/release/examples/inspect_class .cache/e2e/t12-post-chain-fields/dex/classes.dex 0 'Lfixtures/Case;' '<init>' '(I)V' --catalog .cache/e2e/t12-post-chain-fields/catalog.json --output .cache/e2e/t12-post-chain-fields/public.json
```

Actual public operations reject missing field evidence, wrong type/flags and a
missing source name as Debug/BindingEvidenceMissing. Locked smali additionally
supplies a duplicate raw field name with different types, a final-field write
and an instance opcode targeting an original static field. The first two retain
Debug; the static-kind mismatch is Unavailable/Malformed(InvalidFieldKind) even
with the empty catalog. No read failure is converted to a default declaration.

```sh
python3 tools/e2e.py run --case D043 --case D018 --case D019 --case D022 --case D023 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
python3 tools/e2e.py run
```

`.cache/e2e/engine/run-47sgubw6/report.json` passes all twelve related selected
cases and 24 invocations. The complete final
`.cache/e2e/engine/run-txwt7mv4/report.json` retains 86 mandatory cases, 177
invocations and 181 seams: 42 whole cases and 96 seams passed, with no lost
previously passing seam. The full command exits 1 for remaining failures.
D042's int constructor still requires its later StringBuilder/body producer;
the mandatory class request remains Mixed.

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 117 cargo-nextest core tests with zero skips, diff and sensitive-input
checks passed. Existing fixed coverage was reused; no new tests, auxiliary-tool
tests, fixed input/expected changes, fingerprints or TASKS.md edits were
introduced. T12's new/init, prologue and simple own-field producer artifacts are
ready for dependent field analysis; final task acceptance remains pending.
