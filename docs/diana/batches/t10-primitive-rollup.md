# T10 primitive acceptance rollup

Goal: close the remaining long/double, conversion and word-overlap evidence gaps
identified by the existing T10 batches, then rerun the mandatory engine set.
Contracts: ALGORITHMS §3/§4 and TASKS T10. Supplemental public Engine observations
exercise authored self-made code through the existing core DEX builder and locked
JVM. They do not change or add passes to the fixed-case denominator.

## Accepted producer evidence

| Obligation | Actual evidence |
|---|---|
| int width, overflow, div/rem exception behavior, signed/unsigned masks | D009–D012; supplemental long MIN/-1 and negative-count shifts |
| boolean word views and non-empty Known01 | D013/D014; the non01 public rejection in `t10-boolean-views.md` |
| FP cmp bias with both NaN positions and original branch | D015, four actual methods per invocation |
| FP negative zero, Infinity, NaN saturation and truncation | D016/D017; supplemental double negative zero/Infinity/subnormal and rounding ties |
| wide word identity, overlap and partial reads | D058; supplemental valid overlapping move and malformed low/high reads |
| lossless byte/short/char return after a Phi | all six D071 return invocations; D072; three proof-negative public operations in `t10-narrow-provenance.md` |
| original UTF16 and MUTF8 target boundary | D076; exact complete 22-unit supplemental JVM observation in `t10-string-literals.md` |
| incompatible non-literal families and exact NaN literal rejection | the public-Engine core regression plus supplemental double-payload rejection |

The native source still contains no synthetic bitcast/NaN helper. Type conflicts
and ruleless payload views retain classified failure/raw rather than executable
Java. Every produced scalar family/use product is sealed; the CFG range closure
uses only original fixed producer bounds and finite hulls. These are the admitted
producer profiles, not a claim that every legal DEX method is representable.

## Supplemental execution

`.cache/e2e/t10-primitive-rollup/inputs.rs` reuses the existing core `CodeRecipe`
builder. It authors 25 small native methods. Twenty-two Java outcomes compiled
under the locked JDK/Java8 target; every result/exception matched both a worked
expected value and an independently authored Java reference. This includes the
64-bit literal selecting both long and double views without a new SSA identity,
rounding boundary conversions, float/double subnormals, both double NaN biases,
long remainder and unused division/remainder whose exceptions must not be
eliminated. Sources, compile/JVM logs and the complete comparisons are under
that directory, with `jvm-observations.json` as the rollup.

The other three original inputs produced the required negative outcomes: a
noncanonical double NaN literal retained Unsupported/raw, and both partial-word
reads were Malformed/InvalidWideRead without executable source. The actual
results are in their named public JSON files and `public-observations.json`.

```sh
DIANA_ROOT="$PWD" rustc --edition=2024 .cache/e2e/t10-primitive-rollup/inputs.rs -o .cache/e2e/t10-primitive-rollup/inputs
.cache/e2e/t10-primitive-rollup/inputs
target/release/examples/inspect_class .cache/e2e/t10-primitive-rollup/literal64_views.dex 0 0 f '(I)I' --output .cache/e2e/t10-primitive-rollup/literal64_views.json
```

## Full gate and acceptance

On macOS arm64, `.cache/e2e/engine/run-1k0tap7y/report.json` records the complete
mandatory set at `796edd1`, clean engine revision: 86 cases, 177 invocations and
181 seams retained; 33 whole cases and 85 seams passed. It exited 1 for the
remaining capabilities and application seams. Every fixed primitive observation
listed above passed except D071 call-byte, whose exact call binding belongs to
T11 under the existing T10 contract. D071 remains a failed whole case.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
```

All 116 cargo-nextest core tests passed with zero skips. Fmt, strict Clippy,
locked release build, diff and sensitive-input checks passed. No new tests,
auxiliary-tool self-tests, comparator falsification, fixed input/expected edits,
fingerprint gates or TASKS.md changes were introduced in this rollup. ART and
other host-platform runs were not performed.

T10's fixed primitive producer obligations are accepted. T11 consumes the actual
family, descriptor and narrow-range evidence for calls; its unresolved call and
the other full-suite failures remain non-passes.
