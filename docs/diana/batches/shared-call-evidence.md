# Static-call evidence after shared SSA

Goal: make normal-flow decoding independent of declaration-catalog selection,
collect call evidence after SSA, and discharge source-binding obligations after
typing. Contracts: DESIGN §4; ALGORITHMS §3, §4 and §9.

This is the static-call ordering slice of the call/member migration. Receiver,
nominal and field migration follows through the same operation arena; existing
whole-method routes retain the baseline's temporary boundaries.

## Implemented slice

P2 records the original MethodId, parameter words, return descriptor family and
invoke/result sites without a catalog lookup. P3 checks the resulting shared
inputs and normal definition before any declaration selection. The current normal
profile still rejects wide argument frames and consumed non-builtin reference
results; those existing capabilities remain migration inputs.

`p4_calls/normal.rs` consumes the sealed SSA and produces ResolvedBody with explicit
declaration-or-missing call evidence. P5 uses original return facts to solve value
types even when source declaration evidence is missing. Missing evidence does not
become an empty declaration or an assumed no-throw contract.

P8 first validates the typed argument views and then checks fixed-arity overload
selection and escaping checked-throws compatibility. The existing helper algorithms
remain shared with temporary legacy producers, but the common route no longer
performs these source-binding decisions during decoding. Full candidate/type and
member migration remains part of the current milestone.

The engine runs the shared chain before its legacy evidence preflight. A shared
Java-admission failure temporarily allows the existing legacy routes to retain
capabilities such as checked-throws annotation emission; its classified reason is
kept for the final raw result. This transition is not the final engine schedule.
There is no quota increase or repeated shared SSA construction.

All call/evidence and typed-view loops use the existing MethodBudget; owned
contracts/evidence use charged fallible growth. P2/P3 retain borrowed input facts
and do not retain declaration-catalog references in their products.

## Evidence

The existing public SSA definition and missing-binding coverage was inspected
before adding the distinct call-input control-path regression. With the same empty
catalog and original recursive-call reference, an undefined argument produces
Malformed/UndefinedRegister; a defined argument produces
Unsupported/BindingEvidenceMissing rather than guessed Java. All 120 native tests
run with zero skips.

On macOS arm64, fmt, strict locked workspace/all-target/all-feature Clippy, locked
release build, cargo-nextest, diff and sensitive-input checks passed. An isolated
`0abfabe` checkout with only this staged contribution passed the same Rust gates
and all 120 tests using the existing certified D065 container. Only the normal
module declaration is staged from the partially modified P4 module file; the
frozen field exports and other static-field work remain uncommitted.

- Full required execution: `.cache/e2e/engine/run-yshqyni6/report.json`, all
  86 cases/177 invocations/181 seams selected, 51 cases/117 seams passed. The overall
  result remains failed. No passing seam from `run-35_lx6_3` was lost or added.
- Dispatch-only D006, D018, D019, D022, D024, D026, D071 and D077:
  `run-169x5fcs`, all 8 cases/18 seams passed.
- Reused the existing checked-throws DEX/catalog and requested `target()I` through
  the public example. It remains Java with its original `throws java.lang.Exception`
  clause (`checked-throws.json`). This is an admission observation, not a newly
  executed JVM comparison or JavaUnit acceptance.

Logs, the isolated patch and the checked-throws observation are under
`.cache/e2e/shared-call-evidence/`. Next: shared nominal/reference obligations,
receiver and field operations, and independent call/field/array composition.
