# Declaration and source-condition phase boundaries

Goal: repair the audited P4/P8 declaration-evidence and P8/P9 source-condition
boundaries before expanding T20. Baseline: 78fba58. Contracts: DESIGN §4;
ALGORITHMS §4/§9; TASKS §1/T11/T12/T20. Preparation reuses accepted draft-7
B01–B04 on macOS arm64. The operator authorized these repairs after the independent
architecture review.

P8 owns legal Boolean/integer/reference conditions and independent replay against
the original branch. P9 prints the sealed source operator and operands without
consulting DEX opcodes or selecting null from a type. P4 owns declaration
candidates, access/ancestry/throws facts and binding obligations; P8 consumes
those products with final typed uses rather than first resolving declarations.
All traversals retain existing work/allocation and cancellation accounting.

The existing public branch, numeric, binding and initialization cases provide
regression coverage. A small original Class/null branch checks the audited
missing-reference-family counterexample through Engine and independent Java8
compilation. Current production behavior and fixed expectations remain distinct
from pending review hypotheses.

## Execution

P8 conditions::lower now seals Boolean conditions, comparison operators and
local/null/numeric-zero operands. Independent replay checks those source choices
against original branch definitions and opcodes. P9 open_if prints the sealed
expression and no longer consults DEX opcode/type families. The Class/null
counterexample reaches VerifiedJavaMethod at the baseline and fails independent
Java8 compilation because its source compares Class with int. Corrected source
compiles and matches the checked-in Driver under both representation policies.
Native mutation checks reject changed literals, operators and operand identities.
P5 classifies ordered comparison of a definite reference as Malformed; the public
negative retains original code in Unavailable instead of emitting Java.

P4 collects receiver declaration groups, original/local access consistency and
throws before P5. ConstructorDeclarations retains the allocation and applicable
this/super declaration obligations; P5 initialization identity chooses their
use later without rerunning constructor declaration resolution. P4 also retains
the method's SourceThrows product. P8 consumes these products, checks typed
receiver/argument identities and discharges source binding for the final receiver
type, including subtype and additional hiding/overload obligations. The existing
bounded constructor-prefix proof now runs during P4 declaration collection;
its structural review remains separate from a claim of complete constructor
semantics.

The member entry catches failed explicit fallback reservations after the budget
scope has ended. Cancellation remains request-terminal. A resource failure
returns the original MethodKey as Unavailable when the request can still retain
the diagnostic/report; it does not start normal analysis or raw decoding.
Public tests cover method-allocation and request-work reservation failures while
retaining both original constructor identities in a class request.

The E2E dirty check now includes the tracked specification, complete runner and
source tree, active application/engine and dependency locks. This is a provenance
scope correction, not fingerprint verification. The query was inspected and
exercised during core E2E execution; no auxiliary-tool tests were added.

## Reproduction and results

Inputs: crates/diana-decomp/tests/fixtures/source-conditions/{Conditions.smali,
fixtures/Conditions.java,fixtures/Driver.java}; the locked Smali 2.5.2 toolchain
assembles classes.dex. The compact catalog retains the original fixture and
existing JDK declaration evidence. Public regressions are in
crates/diana-decomp/tests/pipeline/{conditions,reservations}.rs. Existing binding,
constructor, exceptional-control and monitor cases remain unchanged.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64, 155 Rust tests pass with zero skips; fmt, strict Clippy and locked
release build pass. The initial isolated-worktree run was blocked by its missing
certified D065 input. Reusing that unchanged input resolves the preparation
blocker; the complete test run passes both before and after integration.

Full E2E remains 70/86 cases and 143/181 seams passing and exits nonzero for
outstanding cards. Comparison with 78fba58 covers 2,379 original member
observations: identities, member statuses, sources and classified diagnostics
are unchanged. No passing seam regresses. Original and recovered Class-reference
Drivers compiled with Temurin 21.0.8+9 targeting Java8 agree in both representation
policies. T20 final-field/delegation and cross-unit constantness proofs remain
pending; these repairs do not count their required cases as passed.
