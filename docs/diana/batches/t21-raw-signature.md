# Raw signature declaration evidence

Goal: admit a bounded raw generic no-code declaration without guessing missing
Signature facts. Baseline: 8efc1ad plus T21 interface/bridge slices. Contracts:
DESIGN §4/§4.1; ALGORITHMS §12.1/§12.3; TASKS T21. Preparation reuses accepted
macOS arm64 B01–B04 and unchanged draft-7. The operator approved P1 Signature
facts, optional P0 declaration Signature evidence and P4 erasure checks.

P1 reads system Signature string fragments through existing checked annotation
and encoded-value products. P4 compares original units with supplied catalog
Signature and original descriptor, accepting the bounded parameterized-reference
erasure profile. A single Object-bounded method type variable also erases to Object. Wildcards,
nested parameterization and unknown source substitution remain Unsupported. P8 admits the erased declaration and retains original
Signature units; source type/qualifier checks and target limits remain active.
P9 only prints the verified declaration. No generic call inference or alternate
method analysis is introduced.

The self-made Raw.smali and raw-catalog.json under
crates/diana-decomp/tests/fixtures/covariant-bridge/ provide a List<String>
abstract declaration with an independent erased List descriptor. Assemble with
pinned smali 2.5.2. The public bridge.rs table retains the constructor and rejects
missing or contradicted Signature evidence. Allocation and traversal use existing
request/method ledgers and checked annotations; malformed metadata does not
become an empty signature.

## Execution

P4 RawSignature is a privately constructed evidence product passed by the
scheduler to P8 no-code declaration admission. P8 does not call the P4 producer.
JavaMethodDecl retains the original checked Signature units alongside its erased
source. The List<String> and single Object-bounded identity declarations compile
as a complete recovered abstract unit with the pinned javac --release 8. Their
original reference is the tracked Raw.java. Missing/contradicted signature rows
remain Mixed and preserve the constructor.

Observed on macOS arm64:

- cargo fmt --all -- --check: passed.
- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings:
  passed.
- cargo build --workspace --release --locked: passed.
- cargo nextest run --workspace --all-features --locked: 165 passed, zero skips.
- python3 tools/e2e.py run --case D021 --case D048 --case D075 --case D078 --case D084:
  all selected fixed obligations pass.
- python3 tools/e2e.py run: 76/86 cases and 152/181 seams pass; all 177 invocation
  scenarios execute. Exit 1 retains outstanding requirements. The raw-signature
  slice changes no required fixed-case verdict.
- git diff --check and python3 tools/audit/desensitize_check.py --range HEAD:
  passed.

This slice verifies bounded raw no-code declarations. T21's broader generic
parent/interface substitution, executable generic method binding and bridge
parameter-cast profiles still lack acceptance evidence; the complete card is not
marked done.
