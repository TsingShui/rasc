# Implicit receiver and special calls

## Implementation

This slice moves ordinary initialized `this`, direct calls and super calls into
the shared flow and storage consumers. Contracts: [DESIGN](../DESIGN.md) and
[ALGORITHMS](../ALGORITHMS.md).

P1 retains the original method-owner TypeId. P2/P3 distinguish static and instance
frames and retain original invoke kind. The instance frame includes its implicit
receiver definition, separately from explicit Java parameters. Constructor flags
exclude uninitialized receivers from this ordinary-method path.

P5 requires a super receiver to derive from the entry receiver through moves or
Phi inputs. The iterative traversal requires an entry anchor, rejects other
origins and uses a value-indexed Phi lookup. Its work is O(V + E) per checked
receiver, with charged fallible storage and no recursive traversal. P6 seals
`Parameter::{This, Explicit}` origins; P8 independently checks them against frame
definitions. P8 produces `CallReceiver::{Value, Super}`, verifies its consistency
with original invocation kind and consumes existing access/subtype/throws proofs.
P9 prints that verified expression rather than selecting dispatch semantics.

The shared headers omit the implicit receiver and preserve explicit parameter
numbering. Existing storage, CopyPlans and both representation policies are reused.

## Core evidence

The existing shared-fields fixture adds Reader.secret/direct and SpecialReader.
The latter overrides value through super.value with an int argument. The existing
public regression adds only their complete-evidence requests; irrelevant field
metadata variants are not multiplied across these methods. There are still 122
core tests and no added test function or harness.

The prior fixture recipe adds SpecialReader.class. The existing Driver adds two
fixed boundaries, not a new Cartesian product. Both policies match the reference
on its 88 retained observations. The two new boundaries exercise the recovered
Reader.direct and SpecialReader.value in unchanged dependency contexts. Emitted
method compilation verifies implicit-this storage and explicit argument numbering;
observed values and callback state distinguish private dispatch and super dispatch.
The owned call evidence retains Direct/Super, original registers, MethodId and PC.
These are supplemental method-context observations, not fixed JavaUnit acceptance.

## Executed checks and remaining boundary

- Format, strict Clippy, release build and cargo-nextest passed: 122 tests,
  zero skips. The staged patch passed these Rust gates in an isolated worktree
  without the frozen static-field changes, reusing the certified D065 fixture.
- Related fixed E2E `run-di2tj1o6`: overall failed; 4/7 cases and 7/10 seams
  passed, with no lost pass against `run-qmrpwcl1`. D021, D041 and D042 retain
  their earlier whole-class failures.
- D021's selected f remains on the temporary receiver path because its String
  literal prefix is not yet decoded by shared flow. Shared String-prefix and
  checked-throws migration remain the next ordinary-call gaps.
- Not run: full required E2E after this slice. The latest full result remains
  `run-qmrpwcl1`; final reacceptance still requires the complete fixed set.
- Diff and sensitive-input checks passed before submission.

Artifacts are under `.cache/e2e/shared-this-calls/`. The frozen static-field
changes remain uncommitted. Constructors retain their initialization-specific
obligations for the next architecture task.
