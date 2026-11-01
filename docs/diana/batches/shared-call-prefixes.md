# Shared call prefixes and checked throws

## Implementation and boundary audit

This slice closes the remaining ordinary-call prefix/header gaps identified in
[shared-this-calls](shared-this-calls.md). Contracts: [DESIGN](../DESIGN.md) and
[ALGORITHMS](../ALGORITHMS.md).

P2 decodes const-string/jumbo into original StringId plus a request-local literal
identity and borrowed P1 value. P3 carries this literal arena with the shared
operations. P5 gives the result its String kind. P6 uses existing storage. P8
copies UTF-16 units into the owned AST and independently checks original opcode,
reference, register, result, type and exact units. Literal materialization remains
at the original effect position. P9 reuses the reviewed literal writer.

P8 SourceCalls retains the existing sealed source-throws proof together with
receiver bindings. Dispatcher headers consume that proof; supported Throws
annotations no longer fail a blanket method-annotation admission check. Other
annotations and unsupported throws retain explicit rejection. Both representations
use the same operation/storage consumers.

The ordinary legacy call-chain operations are constants, calls and construction.
Its non-construction call profiles now have shared producers. The initialized
receiver route's aliases, String/effect prefixes, invocation and int-result
addition also have shared producers. Reviewed int instance heap accesses use the
shared field path. Construction and constructor-owned field writes still require
the initialization migration in task 5; corresponding whole-method trackers stay
until parity permits task 6 deletion. The frozen, unverified static-field draft
is not an accepted capability baseline. Original constant-read guards and static
call class-initialization positions remain intact.

## Independent core evidence

No new test, fixture, Driver or validation framework was authored. Existing
prepared D010/D021/D022/D071/D077 inputs and cached self-made String/throws inputs
were reused.

- D021 f now uses shared this storage, literal, static log, super call and
  addition. Its emitted method, compiled with unchanged Base/Dep and the existing
  Driver, matches the independent original ordered-event observation.
- Both policies' checked-throws target methods compile and match the two existing
  normal/throwing JVM observations, including call counts.
- Both policies' escaped literal methods reproduce the existing independently
  recorded UTF-16 units, including NUL, a literal backslash-u sequence, control
  characters, lone surrogates, a surrogate pair and line/paragraph separators.
- Existing unknown-caller-throws and undeclared checked-exception inputs retain
  Unsupported/BindingEvidenceMissing. The malformed Throws value retains
  Malformed/InputMalformed(InvalidEncodedValue).

These are method-context observations, not JavaUnit acceptance or new fixed cases.

## Executed gates

Format, strict Clippy, release and cargo-nextest passed: 122 tests, zero skips.
The staged Rust patch also passed those gates independently of the frozen
static-field changes in a detached worktree, reusing the certified D065 fixture.

Related E2E `run-bdbztvkk` remains overall failed: 4/5 cases and 11/12 seams
passed, without losing any corresponding pass from `run-qmrpwcl1`. D021 retains
its whole-class failure while its available methods remain Java. Not run: full
required E2E after this slice; final reacceptance retains that obligation.

Diff and sensitive-input checks passed. Artifacts are under
`.cache/e2e/shared-call-prefixes/`. Temporary route deletion and the unified raw
exit remain owned by task 6, not claimed complete here.
