# Non-default boolean preparation

Goal: emit a checked non-final static boolean encoded preparation before original
class initialization. Baseline: 1d12d2e. Contracts: DESIGN §4/§4.1;
ALGORITHMS §12.1–§12.2; TASKS T20. Preparation reuses accepted B01–B04 on
macOS arm64 and unchanged draft-7 inputs and tools.

The operator approved a read-only preparation view on P8 VerifiedJavaField for
P8 unit admission and P9 rendering. P1 FieldInput supplies checked original
identity, static ordinal, flags, descriptor and encoded bytes. Its preparation
metadata is partial when collection reports damage; absent bytes never imply a
default. P4 supplies the existing field source type. P8 validates the encoded
boolean and owns its source assignment; P9 prints that product after all field
declarations and before original members. Ordinary P2–P8 scheduling and final
field rejection remain unchanged.

The invariant is original preparation order without a constant-expression field
initializer or a new method analysis. Existing public D046 explicit/implicit,
non-default and malformed preparation rows in
crates/diana-decomp/tests/pipeline/initialization.rs provide independent input
and failure evidence. Extend that table rather than add a separate test harness.
Use its existing metadata-limit check and normal request ledger; charge source
construction and printing with fallible allocations and cancellation checks.

Scope: BOOLEAN(true) preparation for supported non-final static fields. Other
non-default encoded forms retain Mixed until their own checked source products
exist. Final/write-count and delegation proofs remain T20 obligations.

## Execution

P8 field_declarations::verify_impl checks the static storage, boolean descriptor
and original BOOLEAN(true) tag/argument/byte before constructing a private source
block. class_unit::verify_impl admits that block as preparation evidence. P9
report::render prints all declarations first, then these blocks in original field
order, then original members. It neither decodes values nor chooses assignments.

The existing preparation table adds the supported true variant and an unsupported
nonzero int variant, preserving malformed count/tag rows and original field,
member and encoded-byte checks. No new test function was added. The new source
buffers and P9 copies use the existing charged fallible TextBuffer and request
checkpoints.

Observed on macOS arm64:

- cargo fmt --all -- --check: passed.
- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings:
  passed.
- cargo build --workspace --release --locked: passed.
- cargo nextest run --workspace --all-features --locked: final integration
  passes 159 tests with zero skips, including the unsupported-int row.
  cargo nextest run -p diana-decomp --all-features --locked t20_ passes all five
  selected tests.
- python3 tools/e2e.py run --case D041 --case D042 --case D043 --case D044 --case D045 --case D046 --case D047 --case D079:
  six cases/six seams pass; D045's two invocations and D079's two seams fail.
- python3 tools/e2e.py run: 70/86 cases and 143/181 seams pass; all 177 invocations
  execute. The command exits 1 for remaining required obligations. All 154 public
  reports and 2,379 original member observations match the prior fixed-input run.
- git diff --check and python3 tools/audit/desensitize_check.py --range HEAD:
  passed.

Independent Java8-target compilation uses locked Temurin 21.0.8+9. Reproduce the
supplemental input from the tracked initialization/d046.dex fixture by changing
byte 409 from 0x1f to 0x3f, as the public test does. Pass that input and the tracked
d046-catalog.json (with environment_id set to 0) to the existing release
inspect_class example. Compile its complete rendered unit with a Driver that
reads b, i and whether s is null. Independently compile a Case declaring those
three fields and setting b to true in a static block. Execute each in a fresh
JVM: both observations are true:0:true. The recovered classpath contains only
the recovered Case and Driver. An initial invocation omitted the example's
required environment_id catalog property; the corrected invocation above passed.

The approved boolean preparation slice is verified. Other non-default encoded
values and final assignment/delegation evidence remain pending.
