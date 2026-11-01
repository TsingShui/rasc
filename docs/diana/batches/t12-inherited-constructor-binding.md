# Constructor and covariant receiver binding

Goal: repair the T12/T11 binding gap blocking D033 without changing its required
Java contract. Contracts: DESIGN §4/§3.4; ALGORITHMS §9.1–§9.4. Preparation reuses
accepted B01–B04 on macOS arm64, draft-7 and D033's certified DEX, ct.sym/source
declarations and independent source-JVM Driver.

P4 owns source type/hierarchy and member-selection evidence; P5's exact typed
arguments and P8's final call binding consume that evidence through the ordinary
method chain. The invariant is unchanged allocation/init/effect order and the
same constructor or virtual dispatch, with complete relevant candidates. Missing
ancestors, uncertain bridge/generic binding and real return-only collisions
remain classified failures.

Baseline `run-l1endj5g` reports BindingEvidenceMissing for StringBuilder's
constructor at PC 2. Inspection finds an interface-free class guard, a
builtin-only overload parser and an Object-only receiver-parent guard. The
original API directory also contains covariant compiler bridges. These need
separate checked proofs rather than a class-name exception or a concatenation
rewrite. Existing core constructor, primitive-call and nominal-binding tests
supply regression coverage.

## Implementation and invariants

P4 `hierarchy::collect` publishes a sealed, checked ancestry product only after
all referenced parents/interfaces are present and the explicit DFS finishes
without a cycle. It accepts the existing classfile interface internal names and
DEX descriptor references, normalizing only lookup keys. Subtype rejection uses
a complete ancestor traversal; unknown ancestry is not false evidence.

`contracts::{member_directory,directory,plain}` keep member completeness separate
from printed-owner lexical scope. Public inherited signatures can participate
without spelling their owner in source. Printed types still require the existing
nongeneric, enclosing-scope, access and qualifier proofs. The original strict
caller/scope checks remain with `plain` and `clear_scope`.

`overloads::unique_target` parses reference/array competitor descriptors as well
as primitive formals. Source argument views remain fixed to the original target
formals from P5. Constructors consider their own declaration directory, not
inherited constructors. Receiver binding examines the checked source-receiver
ancestry and verifies equal-vector overrides with covariant results. A bridge is
hidden from source candidates only when public bridge/synthetic flags, a unique
same-class nongeneric implementation, an inherited nongeneric declaration and
the covariant result relation all agree. An original call targeting the bridge
itself is not admitted by that projection. Duplicate non-bridge parameter vectors
in one owner remain conflicts, including inherited return-only collisions.

`allocation::resolve`, `receivers::resolve_receiver` and static binding consume
these shared selection checks. Initialization, argument conversion, operation
lowering, CopyPlans and printing remain in their existing phases. No name-based
StringBuilder rule, concatenation intrinsic or allocation/helper rewrite is used.
Every hierarchy, descriptor, candidate and bridge scan charges work and checks
cancellation; vector growth uses the existing fallible allocation accounting.

## Independent coverage and execution

The focused fixture copies D033's certified DEX and retains complete relevant
ct.sym/source class directories from its prepared declaration catalog. It adds
no generated expected values. The fixed D033 Driver verifies the recovered Java
return and cleanup event against the existing independent specification and
source-JVM baseline. Its recovered source compiles directly in the known method
context. The initial partial repair progressed from the constructor failure to
`toString` at PC 17; final selection now verifies both inherited `toString` and
covariant `append` against their original identities.

The public Engine table uses both internal-name and descriptor interface facts.
Negative rows remove a parent or interface, introduce a hierarchy cycle, remove
bridge provenance or its inherited declaration, mark the target generic, or add
an own/inherited return-only collision. Each remains Debug with
Unsupported/BindingEvidenceMissing. The positive retains the original new/init
pair and exact String append target. The ancestry producer independently rejects
configured work/allocation limits and cancellation. Existing constructor-chain,
primitive-call and nominal-binding cases remain the regression set.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run -p diana-decomp --all-features --locked t12_
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run --case D033
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64, Cargo gates pass with 143 tests and zero skips. The focused T12
selection runs three tests. Full `run-2r3kb0zb` executes all 86 required cases,
177 invocations and 181 seams in unchanged draft-7; it exits 1 with 57 cases and
126 seams passing. D033 now passes, and the entire fixed D029–D035 source-JVM
group passes. The remaining whole-suite failures are not counted as passes.

Review against `run-l1endj5g` covers all 2379 original member observations.
Exactly two observations improve from Debug to Java: D033's `f()I` and D042's
`<init>(I)V`, whose log expression also uses these calls. Original identities
are joined by MethodId and checked against the retained original Debug
name/prototype when header facts were previously unavailable. All other
availability, classified failures and Debug source remain unchanged; no
previously passing seam regresses. D042's full-unit requirement remains failed
because its class is still Mixed; the newly available constructor does not mark
the case passed.

Evidence: `.cache/e2e/t12-inherited-constructor-binding/` contains focused/full
E2E logs, `nextest.log`, `targeted-nextest.log` and `member-comparison.json`.
The run directory contains actual recovered Java, javac commands/results and
Driver observations. Diff and sensitive-input checks pass. ART and other
platforms are not run, and EngineCoreRC remains unaccepted. The recorded D033
implementation blocker is resolved without changing its acceptance contract.
