# Class initialization and final-field assignment

Goal: implement T20 for the unchanged draft-7 D041–D047 and D079 obligations.
Contracts: DESIGN §2.2/§4; ALGORITHMS §12.1–§12.2; TASKS T20 and §3–§4.
Preparation reuses the accepted macOS arm64 B01–B04 artifacts and the locked
Temurin 21.0.8+9 / Java8 and Smali toolchains. Implementation baseline: f2303e5.

## Producers and independent evidence

P1 supplies checked original class/member identities, encoded-value tags, bits,
ranges and static-field ordinals. P4 owns field access, constructor target,
visibility, throws and class-init binding obligations. Ordinary methods retain
P2 effects → P3 SSA → P4 evidence → P5 typed uses → P6 copies → P7 regions.
P8 owns source declarations, initializer lowering and final-path-count checks;
P9 renders the sealed products and original field facts.

T15's acceptance and derived-array batch records establish the shared checked
heap producers and public semantic/resource evidence. Active field binding
currently rejects final writes; this remains a guard until initializer-owned
obligations discharge them. T19 supplies checked synchronized regions and the
initial verified JavaUnit result. Its closed class profile rejects fields,
encoded initial values and non-Object parents.

The existing source-JVM Drivers supply constructor/delegation, field-value,
initialization-order and failure observations. D046's self-made encoded input
and independent projection distinguish explicit tags from implicit defaults.
D079 requires independently recovered Case/Reader units, joint compilation and
an actual non-inlined getstatic. Expected values remain in e2e_cases.json.

## Preparation status

The operator approved public field/declaration/initializer increments and the
FinalAssignmentUnproved diagnostic. ClassOutput now retains original FieldOutput
entries and their declaration/initial-value status. MethodOutput::JavaInitializer
retains the original MethodKey and a sealed initializer body rather than a fake
callable method.

The focused baseline passes D043; the other selected obligations fail.
The original D042 run exposed a false JavaUnit missing field n. Commit f2303e5
corrects that T19 completeness guard: D042 is Mixed with both constructors still
available as Java. The public static/instance-field regression and the full
147-test Rust suite pass. This does not change D042's required Java acceptance.

Reproduce the baseline with the certified sources in tests/e2e/sources/ and
expected values in docs/diana/e2e_cases.json:

```sh
python3 tools/e2e.py run --case D041 --case D042 --case D043 --case D044 --case D045 --case D046 --case D047 --case D079
cargo nextest run --workspace --all-features --locked
```

## First implementation slice

P1 field_declarations::collect binds original field identities and static
ordinals to encoded ranges/tags/bytes. Explicit defaults and omitted defaults
remain distinct. A malformed encoded suffix retains checked prefix facts and
available methods; it never becomes an implicit default. Directory and encoded
value traversal charge the request work/metadata ledger and checked allocation
before growth, with cancellation checks throughout.

P4 field_declarations supplies primitive and checked String source types.
class_scope reuses the shared checked ancestry product to bind a source parent;
unknown generics, ownership or inherited member obligations still reject a unit.
P8 admits non-final default preparation, checks original field/member completeness
and emits fields before method/initializer bodies. It preserves final fields and
FinalAssignmentUnproved rather than deleting final or inventing an assignment.
Non-default preparation remains explicit original metadata with Mixed output.

P2 now decodes clinit through the ordinary method chain. P8 checks its original
name, static-constructor flags, void prototype and throws obligations. Native
operation/edge replay checks initializer exits; P9 renders them as labeled breaks.
Necessarily abrupt bodies use the specified constant-true wrapper. The shared
P6 storage and P7 control representation remain authoritative.

Core coverage extends tests/pipeline/initialization.rs under crates/diana-decomp.
Fixtures reuse certified D041/D042/D044/D046 inputs and their declaration facts.
Abrupt.smali and fixtures/Abrupt.java are independent self-made equivalents;
fixtures/AbruptDriver.java checks the first initialization failure and the later
erroneous-class failure. Smali 2.5.2 assembles the DEX, and Temurin 21.0.8+9
compiles both sources with --release 8. Original and recovered Driver results
agree in fresh processes.

Verification on macOS arm64:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

The Rust suite passes 152 tests with zero skips; formatting, Clippy and release
build pass. Full E2E passes 70/86 cases and 143/181 seams, including newly passing
D041/D042/D044/D046/D047. The full command still exits nonzero for outstanding
cards. Comparison against f2303e5 covers 2,379 original member observations:
no identity/availability regression, unchanged source for unchanged member
statuses, and no passing-seam regression. D044/D047 clinit gain Java initializer
bodies. D050 clinit stays Unsupported with identical Debug and a more precise
InitializationUnavailable reason after entering the ordinary initialization chain.

T20 remains incomplete: final write-count/delegation proofs, non-default encoded
preparation emission and D079's independently recovered related units remain
required. D045 and D079 are not counted as passed. EngineCoreRC is unaccepted.
