# Shared wide vocabulary and scalar-path retirement

This slice migrates existing long/double operations and atomic two-word register
identity into shared CFG/SSA, numeric constraints, storage and Java validation.
Contracts: [DESIGN](../DESIGN.md), [ALGORITHMS](../ALGORITHMS.md) and the historical
[T10 rollup](t10-primitive-rollup.md).

P2 owns primitive operation metadata, parameter/result widths, wide constants,
moves, arithmetic, shifts, comparisons and conversions. P3 binds one ValueId per
wide definition. Reads precede writes, including overlapping moves. Its shared
environment records high-word ownership; live-in candidate pairs are checked
against original entry/edge environments, width closure and grounding before the
SSA body is exposed. Partial-word reads retain InvalidWideRead at the read site.
The new proof uses charged fallible storage and finite fixed-point facts.

P5 selects long/double storage families and original per-use literal bits.
Non-literal families remain exclusive; exact NaN literal payloads retain
Unsupported/raw. P6 uses two-word LocalTypes and the existing atomic CopyPlan
producer/verifier. P8 checks original definitions, operand families and literal
bits; P9 reuses exact IEEE spelling and native Java primitive semantics. Unused
integer division/remainder remain materialized throwing operations. Builtin null
return views retain the legacy zero/String behavior through P5 evidence and an
independent P8 check.

The engine scalar attempt, complete scalar parser/register tracker, scalar-only
P5 solver, P8 method variant/verifier and P9 renderer are deleted. Shared method
ownership moves from `p8_java/scalar.rs` to `p8_java/method.rs`. Primitive metadata
lives in `p2_flow/primitives.rs`; the remaining `pipeline::scalar` name is only a
metadata re-export, not an execution route. Array literal formatting directly
consumes the reviewed numeric formatter rather than constructing legacy ASTs.

## Evidence

- After deletion, fmt, strict workspace/all-target/all-feature Clippy, release and
  cargo-nextest pass: 122 tests, zero skips. Detached staged-patch gates pass
  without the frozen static-field changes, reusing the certified D065 fixture.
- Existing T10 DEX, Drivers and fixed observations were reused without new inputs
  or a validation framework. After deletion, all 22 JVM contexts match their
  independent Java reference and fixed expected value. They cover overlap,
  MIN/-1, negative shift counts, FP rounding/subnormal/zero/Infinity, comparison
  biases, dual long/double literal views and unused throwing arithmetic.
- The existing low/high partial reads retain their original InvalidWideRead
  coordinates; the double NaN payload retains Unsupported and original raw.
  These three negative observations remain separate from the JVM matches.
- Full `run-i1u5zztp` retains 86 cases, 177 invocations and 181 seams. It exits 1
  with 51 cases/117 seams passing, with no lost/gained passing seam against
  `run-2eatgaie`. Supplemental matches are not additional fixed-case passes.

Artifacts are under `.cache/e2e/shared-wide-vocabulary/`. Diff and sensitive-input
checks pass before submission. TASKS, fixed inputs and independent expectations
are unchanged. ART and other host-platform execution were not run.

Mixed literal/FP Phi edge conversions remain unproved. Remaining receiver,
constructor, call-chain and heap method routes, engine preflight/raw consolidation
and final architecture reacceptance stay in the current goal.
