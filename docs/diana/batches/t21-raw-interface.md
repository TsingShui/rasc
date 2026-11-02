# Raw interface implementation

Goal: admit a complete nongeneric implementation of a checked raw generic
interface when its method contract is independent of the type variable. Baseline:
8efc1ad plus the verified T21 slices. Contracts: DESIGN §4/§4.1; ALGORITHMS
§9.4/§12.1; TASKS T21. The operator authorized the required declaration product
and scheduling changes. Preparation reuses accepted macOS arm64 B01–B04.

P1 retains original interface identities and system Signature. P4 validates the
single Object-bound generic interface declaration and complete erased method
obligations; unresolved substitution is rejected. P8 keeps original implements
identity and exact return/throws/access contracts. The ordinary body still uses
P2–P8 and P9 renders the sealed header. No interface-specific body builder is
introduced. Missing parent/interface evidence leaves Mixed with available members.

Independent evidence uses a self-made raw interface and its implementation,
compiled separately from recovered Java with the same helper and Driver. Resource
and cancellation behavior follow existing charged declaration traversals.

## Execution

P4 interface_scope consumes original checked interfaces, exact caller catalog
interface identities and the complete interface method directory. Raw interfaces
have one evidenced Object-bound type parameter and methods independent of that
variable. Generic raw parents use the same bounded class signature and reject
variable-dependent method declarations. Source owner checks permit these checked
interfaces instead of treating every interface as missing lexical scope.

The String mapper profile compares original DEX class Signature with catalog
Signature and exactly one interface specialization. Its generic method contract
is (TT;)TT; the implementation has matching String argument/result. P8 bridge
proof checks the original String check-cast, entry receiver/argument words, exact
virtual target, adjacent result and return. No extra instruction or protection
is admitted. P8 prints the evidenced interface<String> header so javac generates
the actual parameter-cast bridge; standalone selection remains raw. P4/P8
candidate and proof entries retain scheduler authority. P9 renders owner syntax
and the Generated identity, not a second executable bridge body.

Self-made Marker/RawImpl/InterfaceDriver, PlainBase/RawChild/ParentDriver and
Mapper/StringMapper/MapperDriver sources under the existing covariant-bridge
fixture directory provide independent reference contexts. Smali inputs and
bounded catalogs retain the same original identities. Locked javac --release 8
compiles references and complete recovered units separately. Fresh JVMs agree
on raw-interface value 7, inherited raw-parent value 7, and mapper String/null/
ClassCastException observations. javap of the recovered StringMapper confirms
an actual Object→String checkcast and exact String target in its generated bridge.
The recovered classpaths contain no original target classes.

Public bridge.rs tables reject missing parent/interface Signature, dependent
parent methods, missing interface contracts and contradicted specialization.
Existing effectful D084, return-only/binding and raw completeness counterexamples
remain active. The core producer test checks configured work/allocation and
cancellation for bridge/override construction. No auxiliary tooling test was added.

Observed on macOS arm64:

- cargo fmt --all -- --check: passed.
- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings:
  passed.
- cargo build --workspace --release --locked: passed.
- cargo nextest run --workspace --all-features --locked: 170 passed, zero skips.
- python3 tools/e2e.py run: 76/86 cases and 152/181 seams pass; all 177 invocations
  execute. Exit 1 reflects remaining T22/T23/application/core requirements.
  All fixed verdicts equal the generic-call baseline. Public reports are unchanged
  except D051's constructor gains Java after corrected interface source-scope
  checks; its whole annotated class remains Mixed. Original identities and
  already-available Java members remain preserved.
- git diff --check and python3 tools/audit/desensitize_check.py --range HEAD:
  passed.

T21's bounded raw generic declaration/call/parent/interface, covariant and
parameter-cast bridge profiles pass the task-scoped fixed and independent
supplemental obligations. Unproved substitutions retain classified failures;
no complete generic inference or beautification is claimed. ART/other hosts are
not run and EngineCoreRC remains unaccepted.
