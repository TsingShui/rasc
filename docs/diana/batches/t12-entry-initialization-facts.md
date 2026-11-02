# T12 entry initialization facts

Goal: remove the P1 constructor register interpreter while preserving caller Java
when a delegated constructor fails after initialization. Baseline: 25e6208.
Contracts: DESIGN §4/§5, ALGORITHMS §9.2–§9.3/§12.2 and TASKS T12. Preparation
reuses accepted B01–B04 and unchanged draft-7 inputs on macOS arm64.

P1 produces a range-checked method envelope. P2's shared local/call decoder
produces an explicit entry window. P3's ordinary definition/use binder produces
its bounded first-entry SSA product. P4 supplies original constructor declaration
evidence; P5 proves the initialization link from those definitions, uses and
effects. The graph consumer checks delegation before P8 sealing. An entry fact
does not certify the remaining body or authorize Java output for that body.

Independent evidence starts with the D042 post-super definition-fault, missing
body binding and cyclic-delegation regressions in
`crates/diana-decomp/tests/pipeline/initialization.rs`, plus the existing T12
constructor fixtures. Original member identity, classified failure, complete raw
output and resource/cancellation behavior remain acceptance obligations.

## Execution

`p1_input/constructor.rs`, its Root/register model and
`p4_calls::allocation::prove_this_chain` are removed. P1's `MethodEnvelope`
retains declaration/code-range guarantees without manufacturing a complete
ValidatedRawMethod for a damaged suffix.

`p2_flow::local_ops` supplies the local operations, constructor atoms and call word
products used by both ordinary decoding and the entry window. P3 shares
`seed_parameters`, `bind_inputs` and `define_result` with the ordinary SSA builder;
the window has a distinct read-only EntrySsa product. Its independent replay shares
the original call-word verifier and checks literal bits, latest definitions, word
ownership, parameters and the initialization endpoint. It creates no fake return or
full CFG. The ordinary P5 constructor argument/receiver proof and the entry proof
share SSA-origin traversal. P4 retains formal families, original this/parent scope
and constructor declaration candidates. The pipeline schedules those artifact
queries and discharges the recorded binding. P8 `constructor_graph` owns cycle
checks, original link-source identity and the certificate tied to the selected
input and initialization site before sealing.

This is a first-activation fact query for another original constructor, not a
second Java builder or a retry from a failed method's raw exit. It does not run the
callee's full body analysis. The scoped products and their limitations are specified
in ALGORITHMS §9.2. The ordinary full-method route, its entry/back-edge constraints
and the strict JavaUnit requirement remain in force.

The D042 regression now covers post-init definition, reference, unknown-encoding
and return-shape failures, missing body binding, an unrelated constructor owner
and a this-delegation cycle. It checks affected original identities, caller Java,
callee failure kind/code, retained fields and complete raw output where applicable.
As falsification, temporarily requiring whole-method callee analysis again made
the definition-fault case fail by withdrawing caller Java. That prerequisite was
removed before final verification.

Per operator direction, the entry replay and ledger regressions are consolidated
in `crates/diana-decomp/src/p3_ssa/tests.rs`; no entry-specific test file remains.
They corrupt definition inputs, literals, call argument order/width, initialization
coverage and parameter identity. Separate fixed ledger cases exercise the new P2
window producer, P3 binder/replay and P5 proof with work/allocation limits and
cancellation. Production uses existing fallible allocation and charged finite
traversals; it introduces no default numeric limit, global cache or retry.

Observed on macOS arm64 with unchanged draft-7 preparation and Java8-target tools:

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`:
  passed.
- `cargo build --workspace --release --locked`: passed.
- `cargo nextest run --workspace --all-features --locked`: 159 passed, none skipped.
- `python3 tools/e2e.py run`: 70/86 cases and 143/181 seams passed; exit 1 for
  outstanding required cases. Reproduce the member comparison with matching
  draft-7 invocation `public.json` reports at 25e6208 and this change: all 2,379
  original member observations and complete public reports are unchanged,
  including identities, availability, source and diagnostics. No previously
  passing seam regressed.
- `python3 tools/e2e.py run --representation dispatch_only --case D006 --case D011 --case D012 --case D013 --case D033 --case D041 --case D042 --case D044 --case D046 --case D047`:
  all 10 selected cases and 17 invocations/seams passed.
- `git diff --check` and
  `python3 tools/audit/desensitize_check.py --range HEAD`: passed.

The constructor-prefix architecture finding is repaired within the documented
entry-window profile. T12's broader acceptance and T20's final/write-set obligations
retain their own task criteria; EngineCoreRC is not accepted.
