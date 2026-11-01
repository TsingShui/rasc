# Empty fill in shared flow

## Slice and implementation

This slice migrates zero-element primitive array fills and their LengthOnly
obligation into common operations, SSA and storage. Contracts:
[DESIGN](../DESIGN.md) and [ALGORITHMS](../ALGORITHMS.md).

Primitive array kinds now retain their component identity across declarations,
values, storage and source types. Array references remain one-word values;
private storage identities remain disjoint from every nominal TypeId. Existing
int access operations keep their int component checks. Length and Object views
consume the general primitive-array reference kind.

P2 records each empty fill's register, payload PC and width. P3 supplies the
original array definition/use without inventing a DEX result. P5 checks primitive
component and width, retaining InvalidArrayPayloadType for known incompatible
inputs. P6 constructs sealed LengthOnly evidence by checking the original fill
and zero-count payload. It owns a distinct synthetic int local for the array
length result. This local is not an original DEX definition or a copy temporary.

P8 consumes the same storage and checks statement/definition/origin identity.
Its copy verifier accounts for LengthOnly locals separately from edge-copy
temporaries. P9 materializes array.length at the original fill position, retaining
the null check without writes or extra size checks. No method alias tracker is
added. Nonempty payloads retain their pending FreshArray proof obligation.

## Existing independent evidence

No test, fixture, Driver or validation framework was added. The cached self-made
zero-fill input and Driver were reused. Both representations match all 16
existing observations: eight primitive components with null or populated arrays,
including unchanged signed-zero and integer boundary elements. Existing malformed
width/reference-array cases retain InvalidArrayPayloadType; the undefined alias
retains UndefinedRegister. The observed output uses shared storage and LengthOnly.

Related fixed `run-e2wg08tt` passed D024/D026/D073/D074/D086: 5 cases,
13 invocations and 13 seams. Independent expected values were unchanged.

## Regression correction and gates

The first full run, `run-k6wy2fkd`, exposed a prior shared-String migration gap:
D076 over incorrectly produced Java for a constant exceeding the class-file
Modified UTF-8 limit. The runner then requested a reference source that correctly
does not exist for that rejected target. The correction reuses the original P8
literal-limit check in shared lowering; no input, fixture recipe or expected
value was changed. D076 below/at/over all pass in `run-7xundg1t`.

Final full `run-pi4safz6` remains overall failed: 51 cases/117 seams passed out
of 86 cases/177 invocations/181 seams. No lost or gained pass against
`run-qmrpwcl1`. Remaining failures keep their actual status.

Format, strict Clippy, release build and cargo-nextest passed: 122 tests, zero
skips. The staged patch also passed those Rust gates independently of the frozen
static-field changes in a detached worktree, reusing the certified D065 fixture.
Diff and sensitive-input checks passed before submission.

Artifacts are under `.cache/e2e/shared-empty-fill/`. Allocation/constructor and
nonempty FreshArray proofs remain in the current task; deletion of the migrated
zero-fill tracker remains owned by task 6.
