# Shared float instruction vocabulary

This slice migrates existing word-sized FP operations, conversions and literal
views into shared CFG/SSA, P5 constraints and P6 storage. Contracts:
[DESIGN](../DESIGN.md), [ALGORITHMS](../ALGORITHMS.md) and the historical
[T10 FP evidence](t10-fp-families.md).

P2 retains float arithmetic/negation, int↔float conversion and original float
comparison operands. Float division/remainder do not acquire integer exception
effects. P5 closes charged representation-family constraints over shared values,
non-literal copies and Phi inputs. Direct constants have independent int/float
use views; non-literal storage does not acquire conflicting families. Exact NaN
literal payloads still lack a Java rule and retain Unsupported/raw.

P5 now seals return conversions before P6. P8 lowers those recorded conversions,
independently checks literal bits, original input/result identity and operand
families, and retains the comparison bias. The common assignment AST owns typed
use views. P9 reuses the reviewed IEEE-field formatter and parenthesizes unary
operands, including negative-zero literals. Natural/dispatcher representations
consume the same assignments and storage.

The existing worked numeric regression now interprets those shared assignments
instead of the removed scalar-only assumption. Its four inputs, independent
worked result, non-literal conflicts and NaN rejection remain unchanged. The
adaptation adds no operations to that finite interpretation. The existing array
regression caught premature generic rejection; array Malformed classification
now precedes an unsatisfied numeric-family obligation.

## Evidence

- Fmt, strict workspace/all-target/all-feature Clippy, release and cargo-nextest
  passed: 122 tests, zero skips. Detached staged-patch gates also passed without
  the frozen static-field draft, reusing the certified D065 fixture.
- `run-160pfds3` passes the six selected numeric cases
  D010/D011/D012/D016/D017/D071 and their 21 seams. The accidentally selected
  D070 application obligations remain not-implemented, so that run exits 1.
- Dispatch-only `run-oqj5qn6_` passes D015/D016/D017: 3 cases and 9 seams.
- Existing `float_subnormal` and `int_float_round` DEX/context Drivers were reused
  unchanged. Emitted Java compiled with locked Temurin 21/Java8 and matched both
  original Java references and existing fixed expectations: `1,1` and
  `16777216,16777216`. These are supplemental observations, not fixed-case passes.
- Full `run-2eatgaie` retains 86 cases, 177 invocations and 181 seams. It exits 1
  with 51 cases/117 seams passing; comparison with `run-m2kvanok` finds no lost or
  gained passing seam. Application and JavaUnit failures retain their statuses.

Artifacts are under `.cache/e2e/shared-float-vocabulary/`. Diff and sensitive-input
checks passed before submission. Inputs, independent expectations, TASKS and the
fixed denominator are unchanged.

The legacy scalar producer still owns wide/double operations pending migration.
Mixed literal/float Phi storage is not proved by these word-sized use views;
failed joins remain explicit rather than fabricating edge conversions. Final
route deletion and architecture reacceptance remain in the current goal.
