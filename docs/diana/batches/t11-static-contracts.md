# T11 complete declarations and static call contracts

Goal: consume complete external declarations for D071 call-byte, retain original
invoke/result/argument order, and discharge byte narrowing plus static Java
binding. Contracts: ALGORITHMS §9.1/§9.4/§9.5 and TASKS T11. Unknown/partial facts
remain explicit missing evidence; a signature match alone does not admit Java.
Existing primitive, CFG, raw and resource cases remain regressions.

P0 now distinguishes partial signature spelling from classfile/API declaration
coverage, generic/enclosing scope and known/unknown throws. Consuming loops charge
candidate scans and unit comparisons. API directories do not prove package-private
absence in the caller's own package. Duplicate class directories are unproven.

The admitted static contract has one exact own declaration in the relevant name
group, no unresolved inherited/interface candidates, a public static owner/member,
known empty declared throws and a non-generic plain source scope. The qualifier
is checked against generated names and original caller field headers. Original
caller hierarchy and locally defined callee flags/hierarchy corroborate catalog
facts; catalog entries do not replace those available DEX facts. More complex
binding remains explicitly unsupported.

P2 consumes the original invoke/move-result atom. P3 binds argument words before
the result write, retains the fixed call/result sites and independently replays
the original reference, words and result opcode/target. P8 owns separate call
statements, typed argument views and the original MethodId/owner/name/proto plus
both origins. ProvenNarrow comes from the sealed Phi range, not the callee formal.
The independent dispatcher checker validates every emitted call site and argument
view. P9 preserves the call's statement position and a discarded result does not
remove the call. No helper, catch wrapping or throws widening is introduced.

On macOS arm64, `.cache/e2e/engine/run-rhlrhp17/report.json` records the complete
mandatory set: 86 cases, 177 invocations and 181 seams retained; 34 whole cases
and 86 seams passed. D071 now passes all seven invocations, including the actual
byte call after its merged Phi. The command exited 1 for the other real
capability/application failures. `run-gdpbnee6` separately passed all 18 selected
invocations of D009/D015/D058/D061/D064/D071/D076.

```sh
python3 tools/e2e.py run --case D071 --case D061 --case D009 --case D015 --case D076 --case D058 --case D064
python3 tools/e2e.py run
```

The same D071 bytes were also submitted to the public Engine with eight caller
catalog variants in `.cache/e2e/t11-static-contracts/`. Complete evidence produced
Java; signatures alone, empty directories, unknown throws, unmatched checked
throws, a non-static declaration, a shadowed qualifier and a missing parent all
retained complete Debug with Unsupported/BindingEvidenceMissing. These are
actual core API observations, not auxiliary-tool tests or additional fixed-case
passes. The compact observations and full public results are retained there.

```sh
target/release/examples/inspect_class .cache/e2e/prepared/D071/dex/classes.dex 0 0 call '(ZBB)I' --catalog .cache/e2e/t11-static-contracts/complete.catalog.json --output .cache/e2e/t11-static-contracts/complete.json
target/release/examples/inspect_class .cache/e2e/prepared/D071/dex/classes.dex 0 0 call '(ZBB)I' --catalog .cache/e2e/t11-static-contracts/partial-signatures.catalog.json --output .cache/e2e/t11-static-contracts/partial-signatures.json
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 116 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Existing core coverage was reused; no new tests,
comparator/runner self-tests, fixed input/expected edits, fingerprint gates or
TASKS.md changes were introduced. The thin adapter's profile label now states its
explicit directory coverage instead of describing every catalog as signatures-only.

T11 remains pending: the remaining call kinds, overload/dispatch discharges,
checked-throws contracts, void caller execution and constant-field guards still
require their fixed evidence.
