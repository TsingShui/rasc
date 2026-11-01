# Nominal reference identities in shared storage

Goal: preserve original TypeId for named reference declarations through shared
flow/SSA/types/storage and resolve owned Java type names only in P8. This supplies
actual parameter, return, Phi and array-store consumers for member migration.
Contracts: DESIGN §3.3–§5; ALGORITHMS §3–§5 and §9.

## Production changes

P1 retains the checked return TypeId and provides parameter TypeId/descriptor facts
through the existing charged reader. P2 nominal declarations and original call
results carry that identity, rather than a method-wide descriptor. P3 preserves it
through definitions/Phi; P5 propagates nominal kinds and validates existing
non-throwing Object views. Different unproved reference joins remain unsupported.
The closed named-class admission rules remain in force for interfaces/generics.

P6 gives each nominal type a disjoint copy-type identity. Its widened private
LocalTypeId represents every original u32 TypeId without an input-pool cap.
Temporary locals preserve the same kind as their source; no ValueId is rebuilt.

P8 resolves each distinct nominal type once against original descriptors and
catalog/scope evidence. VerifiedJavaMethod owns the validated names alongside its
AST. Parameters, returns and local declarations use those names; P9 only looks up
these owned facts and fails on a missing entry instead of guessing Object or
re-reading input. Builtin type spelling remains static. Both adapters and the
independent return/argument/array correspondence checks admit the proven kinds.

All added descriptor, name-table, copy-type and output traversals use the request's
MethodBudget. Name/table growth is fallible and charged. No parallel register
tracker, method recognizer or runtime type check is added.

## Independent core evidence

Existing public SSA/array/call and copy coverage was inspected before adding the
new producer's regression. The added public-Engine test uses two distinct nominal
parameter types, a same-type reference Phi/return, branch null tests and looped
Object-array stores. It renders after dropping Engine and the input bytes. Removing
one named declaration rejects only dependent method requests; the independent
same-type return remains Java. Both representation policies execute this table.
The first fixture request used the wrong class index; the D8 class order is Box,
Case, Token, and the corrected request selects Case at class_def_idx 1.

The self-made distributable fixture is
`crates/diana-decomp/tests/fixtures/shared-nominal/`: Java sources, reference Driver,
reviewed catalog facts and the original 1260-byte DEX. The source files are authored
for this regression under the crate's existing license. The catalog reuses the
unchanged Object/Box declaration evidence and records the self-made Case/Token
members from their sources; it is not generated from decompiler output.

Recipe: compile Case/Box/Token with the prepared Temurin JDK 21.0.8+9
`javac --release 8`; run prepared R8 8.3.37 `com.android.tools.r8.D8 --debug
--min-api 23 --lib <prepared JDK>` over those classes. Compile Driver against the
reference classes. Decompile each selected method with the complete catalog,
compile its Java in the unchanged known class context, and run that same Driver.
Default and dispatch-only output each match all 32 independent observations for
reference identity, null tests, array state and exception classes, including
writes before bounds/ArrayStore failures. These are method-context observations,
not JavaUnit or additional fixed passes.

## Gates

On macOS arm64/JVM, fmt, strict locked workspace/all-target/all-feature Clippy,
locked release build, cargo-nextest, diff and sensitive-input checks passed.
All 121 native tests ran with zero skips. An isolated `dec2e70` checkout containing
only this staged contribution also passed the Rust gates and all 121 tests using
the existing certified D065 container. Frozen static-field work remains separate.

- Full required execution: `.cache/e2e/engine/run-zafjrdz3/report.json`,
  86 cases/177 invocations/181 seams selected, 51 cases/117 seams passed; overall
  failed for remaining obligations. Passing identities are unchanged from
  `run-30zmd452`.
- Dispatch-only D001, D006, D018, D019, D023, D024, D026, D028, D071 and D077:
  `run-cx8fea7i`, all 10 cases/23 seams passed.

Commands, observations and isolation patch are under
`.cache/e2e/shared-nominal-types/`. Next: shared receiver and field operations using
these identities, followed by their call/field/array effects evidence.
