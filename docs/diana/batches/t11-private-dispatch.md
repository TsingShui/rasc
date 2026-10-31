# T11 same-class private receiver binding

Goal: preserve invoke-direct for a private method on an initialized same-class
parameter, including non-virtual behavior and null failure. Contracts:
ALGORITHMS §9.1/§9.4 and TASKS T11. Cross-owner and interface-special forms remain
unproven; ordinary virtual calls must not repair a direct target. Existing fixed
call-kind and resource observations remain regressions.

The receiver contract now distinguishes Direct from Virtual/Interface. Direct
requires an original same-class private, nonstatic, nonabstract declaration with
known empty throws; the original input flags/hierarchy still corroborate the
catalog. Candidate enumeration admits same-class access without pretending that
private methods are public. The initialized-parameter alias proof and ordinary
Java receiver expression preserve null timing and private dispatch. Public call
facts retain Direct and the original reference/sites.

Supplemental authorized input/source evidence is under
`.cache/e2e/t11-private-dispatch/`. Locked javac/D8 compile an authored Case with a
private m, a static target(Case), and a Child with a same-named public method.
Recovered Java runs against the unchanged known-context private member. Both the
independent reference and actual JVM return `1,1,java.lang.NullPointerException`:
the Child name does not turn the private call into virtual dispatch. This is a
public Engine behavior observation, not another fixed-case pass.

The same original DEX with partial directories, a public target counterfact,
unknown throws or a generic target retains complete Debug with
Unsupported/BindingEvidenceMissing. No call kind is silently rewritten to repair
those inputs.

```sh
target/release/examples/inspect_class .cache/e2e/t11-private-dispatch/dex/classes.dex 0 'Lfixtures/Case;' target '(Lfixtures/Case;)I' --catalog .cache/e2e/t11-private-dispatch/complete.catalog.json --output .cache/e2e/t11-private-dispatch/actual.json
python3 tools/e2e.py run --case D023 --case D019 --case D022 --case D071 --case D080 --case D015 --case D076 --case D056 --case D060 --case D064
```

On macOS arm64, `.cache/e2e/engine/run-_fvec5r8/report.json` records all ten
selected cases and 22 invocations passing. Workspace fmt, all-target/all-feature
Clippy with warnings denied, locked release build, all 117 cargo-nextest core
tests with zero skips, diff checks and the sensitive-input audit passed. No new
tests, auxiliary-tool self-tests, fixed input/expected edits, fingerprint gates
or TASKS.md changes were introduced. T11 still needs super/effectful argument and
checked-throws evidence.
