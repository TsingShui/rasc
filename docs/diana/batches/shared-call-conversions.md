# Shared call-use conversions

Goal: make P4 retain original formal constraints independently of catalog evidence,
make P5 determine call-use conversions before storage, and remove type solving from
P8 argument construction. Contracts: DESIGN §3.3–§4; ALGORITHMS §4 and §9.

This continues the call/member migration from
[call evidence](shared-call-evidence.md), using the existing normal profile and
actual shared call/array consumers.

## Production boundary

P4 extracts supported formal constraints from the original prototype. Declaration
lookup failure retains those constraints explicitly; unsupported formals are not
replaced with an empty list. Known declarations reuse the same original-formal
parser.

`p5_types/calls.rs` determines typed argument uses after value facts and array-type
checks. TypedBody contains sealed CallUses before P6 runs: identity, typed null,
proven reference upcast, boolean-word/known-01 and proven narrow conversions.
Missing catalog evidence still prevents Java. The existing normal-profile limits
on formal/reference families remain in force.

P8 no longer runs its argument type/range solver during validation or AST
construction. It maps the final conversion to Java nodes, checks definition/order
correspondence and performs final source binding. Its independent dispatcher
checker still validates argument/target/type correspondence. Call and array uses
share one conversion enum and one Java-value lowering implementation; neither
retains another register tracker.

Evidence and converted-use growth uses charged fallible allocation. Formal, call,
argument and output loops retain work/cancellation checks. Failed conversions
remain classified failures rather than added casts or successful truncation.

## Evidence

Existing public call-definition/missing-evidence coverage, fixed reference/narrow
binding cases and array composition evidence cover the unchanged conversion
contracts. No test functions were added.

On macOS arm64, fmt, strict locked workspace/all-target/all-feature Clippy, locked
release build and all 120 cargo-nextest tests passed with zero skips. The same Rust
gates passed in an isolated `eb46ccd` checkout containing only this contribution
and the existing certified D065 container. Frozen field changes remain separate.

- Full `python3 tools/e2e.py run`: `run-30zmd452`, 86 cases/177 invocations/181 seams,
  51 cases/117 seams passed; overall failed for remaining obligations. Passing seam
  identities are unchanged from `run-yshqyni6`.
- Dispatch-only D006, D018, D019, D022, D024, D026, D071 and D077:
  `run-wvkv7yyv`, all 8 cases/18 seams passed.
- Re-emitted the existing five self-made array-composition methods with both
  adapters. Each output compiles with the locked JDK and matches all 31 existing
  independent value/exception-class/array-state observations, including writes
  before a later bounds failure. These remain method-context evidence, not extra
  fixed cases or JavaUnit acceptance.

Logs, observations and the isolation patch are under
`.cache/e2e/shared-call-conversions/`. Next: nominal/reference identities, receiver
and field evidence through the shared products, followed by call/field/array
composition.
