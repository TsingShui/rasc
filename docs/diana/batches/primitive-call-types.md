# Primitive call arguments and results

Goal: carry long/float/double arguments and results through ordinary static and
receiver calls, including unused results, without narrowing the invocation word
contract. Contracts: TASKS T10/T11; DESIGN §4; ALGORITHMS §2.2/§4/§9.1/§9.4.
Preparation reuses the accepted macOS arm64 draft-7 tools, D018/D019/D022/D023/
D071 and existing independent numeric/receiver Drivers.

P2 owns original words and one logical read per argument with its descriptor
width. P3 owns wide-pair availability and result commit. P4 retains exact formal
and return constraints; P5 supplies original numeric-family use views; P8 checks
the same views in static and receiver binding. P9 emits the verified argument
vector without analysis or new library calls. Constructor admission is checked
by its separate existing initialization proof.

## Implementation and verification

P2 `CallSite` retains original words and checked logical argument/width vectors;
`Op::reads`, `read_width` and `result_width` expose them to shared SSA. The P3
builder, width ownership analysis and independent raw replay consume those widths,
including move-result-wide and a high word overwritten before a call. P4 adds
primitive formals/results and removes the int-only receiver signature restriction.
P5's existing family constraints and literal views now consume call requirements.
P8 `call_argument` checks those same views for static and receiver calls without
adding casts or helpers. All new vectors/scans use existing charged allocation,
work and cancellation; original raw words and result origins remain available.

The public regression reuses the existing call-definition failure test for wide
ownership and adds self-made `tests/fixtures/primitive-calls/` inputs. Locked
Temurin 21.0.8+9 with `javac --release 8` and R8 8.3.37 D8 min-api 21 produce
`classes.dex`; complete own declarations come from independent classfiles and
Object facts reuse D009 certification. The fixtures use the engine's existing
license. Missing declarations remain complete raw, with no successful binding.

Both representation policies compile the recovered Case methods in the original
known context and reuse the original Dep and Driver. All ten observations match
the reference JVM, covering long MIN, parameter NaN payload bits, signed zero,
mixed-width arguments, String/void receiver returns, unused effects and null
receiver failure. Projections, compiler logs and actual/reference observations
are under `.cache/e2e/primitive-calls/`. These are method-context observations.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run --case D018 --case D019 --case D022 --case D023 --case D043 --case D060 --case D071 --case D080
python3 tools/audit/desensitize_check.py --range HEAD
git diff --check
```

On macOS arm64 the Rust gates pass with 132 tests and zero skips. Fixed
`run-ha8x03lq` passes eight cases and sixteen invocations/seams. The full rollup
follows the remaining reviewed capability changes; ART and other hosts are
not-run. Decision: the reviewed primitive call-width/type gaps are corrected.
