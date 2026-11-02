# Phase product hardening

Goal: remove P9 numeric legalization and narrow verified products and declined
outcomes. The attempted constructor-prefix replacement stopped on a member
availability counterexample, recorded below. Baseline: 60066ab plus the
source-control replay batch. Contracts: DESIGN §4/§5; ALGORITHMS §6/§9/§12.2.
Preparation reuses accepted B01–B04 and unchanged draft-7 inputs on macOS arm64.

## Changed producers and consumers

- P8 `numeric_expression` produces Java conditional-expression nodes for DEX
  compares and explicit unary/cast operations. `dispatcher_verify` checks the
  final nodes and typed operands against the original operation; unary checking
  uses its own exhaustive relation, not the producer's lowering helper. P9 prints
  those nodes without reading a DEX comparison bias or conversion signature.
- P8 `local_declarations` selects source type references and entry/default
  initializers from P6 storage. The dispatcher verifier checks both products.
  `TypeNames` exposes an opaque source spelling reference to P9, not Kind-driven
  lookup. P8 also fixes array literal interpretations and constructor source
  arguments; P9 no longer imports P5 analysis types.
- The actual shared descriptor, array-family, integer-range and operator
  vocabulary lives in `substrate::{primitive_types,operators}`. P5 owns conversion
  from P2 declaration families to solved kinds/ranges. P2 and P4 no longer depend
  on P5. P5 still owns array-element validity and typed-use solving.
- P2–P7 expose read-only checked products. Their private owning constructors
  remain in the producing modules; ownership transfer is explicit where the
  next producer needs to move fields. Mutable dereference exists only for the
  existing native fault-injection tests. Final Java method fields are writable
  only inside P8; P9 receives immutable views. The Java unit header is private.
- P2/P5/P6/P8 return explicit `Outcome::Ready/Declined` values carrying existing
  classified reasons. The scheduler's `Analysis` distinguishes ready storage
  from a declined result. P4 missing-binding identity/PC evidence still survives
  later refusal. Engine code consumes those reasons instead of choosing one from
  an absent storage/body. Internal optional-evidence queries retain Option where
  absence is their actual contract. No public error taxonomy changed.

New expression nodes, source declarations and replay maps use fallible allocation
and existing work/cancellation accounting. No numeric default limits were added.
The checked-product wrappers add no heap allocation. Constructor delegation uses
its baseline implementation after the rejected migration was withdrawn.

## Independent evidence

`src/p8_java/numeric_expression_tests.rs` obtains comparisons through the public
Engine using the existing small code fixture recipe. It mutates NaN bias and
comparison relations and checks rejection, with independent conversion-target
checks. Existing Java8 E2E Drivers remain the executable numerical oracle.
`src/p8_java/source_control_tests.rs` covers the final control/protection replay;
its evidence is recorded in the companion batch.

`tests/pipeline/initialization.rs` extends the existing D042 regression with:

- missing StringBuilder declaration evidence in the delegated body, while the
  delegating constructor remains available;
- a post-super definition fault, while preserving the independent caller and the
  callee's original identity and Malformed/UndefinedRegister classification;
- a this-delegation cycle, retaining both original members with complete raw
  output and InitializationUnavailable.

Manual production compilation probes in P9 attempted to change a verified Java
header and a solved type vector. `cargo check --workspace --locked` rejected them
with E0616 and E0596 respectively. Both deliberate probes were removed before
final verification. Reproduce with `method.header = String::new()` for an owned
VerifiedJavaMethod in P9, and `typed.kinds.clear()` for an owned TypedBody.

## Constructor replacement: decision required

The attempted replacement removed P1's register-prefix interpreter and checked
callee initialization entries by running the same ordinary P2–P6 chain. It passed
the fixed draft-7 public reports, but the added D042 definition-fault counterexample
failed: damaging a new-instance destination after the delegated constructor's
super call caused its SSA error to erase the delegating constructor's Java.
This contradicts member failure isolation. A complete callee-body success is too
strong a prerequisite for an initialization-link fact.

The affected replacement was withdrawn; the baseline
`p1_input::constructor_link`/`p4_calls::allocation::prove_this_chain` path remains.
The counterexample now passes and guards that availability. No retry/prefix
fallback was added to the proposed shared-chain replacement.

Required next decision: approve the contract and owning producer for shared
partial initialization facts that survive an unrelated body failure, then replace
the prefix interpreter without adding a second register tracker or requiring
complete callee Java/type/storage success. The follow-up contract and repair are recorded in
[T12 entry initialization facts](t12-entry-initialization-facts.md). TASKS T12 owns
current progress.

## Verification

Final observations on macOS arm64, unchanged draft-7 inputs and pinned Java8-target
preparation:

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`:
  passed.
- `cargo build --workspace --release --locked`: passed.
- `cargo nextest run --workspace --all-features --locked`: 157 passed, none skipped.
- `python3 tools/e2e.py run`: 70/86 cases and 143/181 seams; exit 1 for outstanding
  required cases. Compare matching invocation `public.json` reports with baseline
  60066ab: all 2,379 original member observations and complete public reports are
  unchanged, including identity, availability, source and diagnostics.
- `python3 tools/e2e.py run --representation dispatch_only --case D001 --case D002 --case D003 --case D004 --case D005 --case D006 --case D007 --case D008 --case D009 --case D010 --case D011 --case D012 --case D013 --case D041 --case D042 --case D044 --case D046 --case D047 --case D082`:
  all 19 selected cases and 41 invocations/seams passed.
- `git diff --check` and
  `python3 tools/audit/desensitize_check.py --range HEAD`: passed.

Overall EngineCoreRC acceptance remains open. The constructor finding is blocked
on the decision above; the other listed product/replay changes have the evidence
recorded here.
