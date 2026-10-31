# T10 FP families and literal views

Goal: implement finite scalar family constraints and per-use numeric literal
views, then recover D016/D017 without host FP evaluation or synthetic calls.
Contracts: ALGORITHMS §4.1–§4.8 and TASKS T10. Int/long, malformed-wide and CFG
observations remain regressions. Pure literals retain their SSA identity;
non-literals keep one sealed family.

P5 now closes scalar representation constraints with charged union-find merges
and set intersections. Non-literal moves share a family; each literal use has an
independent view. Width defaults resolve only unconstrained bits. P8 receives a
sealed type/use product and cannot construct it. P9 materializes each literal
view in its own exact-type temp before its consuming operation, without adding
SSA identities or reinterpreting a non-literal.

The scalar producer supports float/double arithmetic, negation and the original
primitive conversion opcodes. Finite FP spelling comes directly from IEEE fields
as Java hex; zero keeps its sign and Infinity uses a constant division expression.
An exact NaN literal without a rule remains Unsupported. No host FP computation,
NaN normalization or library call supplies production results.

Existing fixed coverage distinguishes runtime negative zero/Infinity and
FP-to-int NaN, overflow and truncation. It did not cover one constant used in two
families or a non-literal storage conflict. One minimal public-Engine regression
therefore interprets the actual owned scalar AST against a worked bit example,
and checks non-literal int/float conflicts plus an unexpressible NaN literal.
It shares the existing core DEX builder with the dispatcher regression. No
auxiliary-tool tests were introduced.

On macOS arm64, `.cache/e2e/engine/run-s9fqkgb8/report.json` records all 20 selected
cases and 41 invocations passing, including every D016/D017 call and the previous
int/long, loop, switch, malformed-wide and resource obligations.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D007 --case D009 --case D010 --case D011 --case D012 --case D016 --case D017 --case D055 --case D056 --case D057 --case D058 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064
```

Workspace fmt, all-target/all-feature Clippy with warnings denied, locked release
build, all 116 cargo-nextest core tests with zero skips, diff checks and the
sensitive-input audit passed. The earlier targeted new regression run passed one
test and left 115 unselected; only the subsequent full run supplies full core
evidence. TASKS.md, fixed inputs and independent expectations are unchanged.
T10 still needs cmp/NaN branch lowering, boolean/null, narrow-Phi provenance and
the remaining literal/target boundaries.
