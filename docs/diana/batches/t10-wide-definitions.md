# T10 wide definitions

Goal: consume actual two-word definitions in the scalar producer, pass the D011
long shift, and reject D058's partial overwrite. Contracts: ALGORITHMS §3/§4 and
TASKS T10. Existing int arithmetic, CFG and raw/resource evidence remain
regressions. The input and independent observations stay unchanged.

The scalar producer carries separate int/long descriptor and result facts, exact
parameter word widths, and Low/High words with a shared definition identity.
Every use checks the whole requested definition. Overlapping writes invalidate
an old pair through identity mismatch; both source operands are read before the
destination write. A partial/high-word use produces typed `InvalidWideRead`
classified Malformed, not trusted executable output.

The owned scalar AST carries explicit source types. P5 checks dominance, operand
families, shift-count width, conversion source/result types and return type before
sealing. Long arithmetic, moves, constants, negation/complement and original
int↔long conversions now retain their operations. P9 uses exact long literals
and Java long operators. The existing known-context adapter recognizes primitive
return types instead of int alone; it still replaces only the actual selected
method.

On macOS arm64, `.cache/e2e/engine/run-3rz5_shc/report.json` records all 18 selected
cases and 34 invocations passing, including all D011 calls and D058's mandatory
wide-overwrite rejection. Earlier int, diamond, loop, switch, malformed and
resource observations remain passing.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D007 --case D009 --case D010 --case D011 --case D012 --case D055 --case D056 --case D057 --case D058 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 115 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. Existing fixed core coverage supplied the long and
partial-wide obligations; no auxiliary-tool or additional core tests were added.
The earlier D011 known-context selection failure was recorded in `run-vwbrood5`
and corrected by the actual thin adapter change, not counted as a prior pass.
T10 still awaits its FP, boolean/null and narrow-provenance matrix.
