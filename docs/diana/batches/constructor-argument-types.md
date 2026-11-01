# Constructor argument types

Goal: remove the int-only constructor admission while preserving allocation/init
order and the first-statement rule for super/this chains. Contracts: TASKS T12;
DESIGN §4; ALGORITHMS §9.2–§9.5. Preparation reuses the accepted macOS arm64
draft-7 tools and D043/D020/D041/D042 constructor evidence.

P4 consumes the common formal contracts. P5 retains parameter/constant origins
for a pure prologue and the existing UninitThis/deferred-allocation proofs. P6
independently checks those origins and original formats. P8 consumes the common
call-use verifier for allocation and prologue arguments; P9 renders already
checked views with their parameter/constant source. No side effect moves before
allocation or before the explicit constructor chain.

## Implementation and verification

`allocation::resolve` consumes the shared formal vector instead of an int count.
P5's initialization proof permits pure prepared-argument moves and numeric
literals between allocation and init, while retaining uninit alias checks and
rejecting effects. `initialization::chain` records parameter/word/wide literal
origins. P6 replays original literal bits and move formats. P8 allocation and
constructor-entry verification share `call_argument`; a prologue argument keeps
both its independently checked source origin and its sealed typed view. P9
`value_with` prints the same view over either a local or a prologue origin.

The existing P1 bounded constructor-link proof now retains reference/null and
wide-word ownership in its pure prefix. It stops at the first constructor call,
retains cycle/access checks and never analyzes the later body. Checked ranges,
metadata, origin walks, new vectors and output all use the existing control
ledger; no numeric default quota or retry path is added.

Self-made `tests/fixtures/constructor-types/` sources use the engine's existing
license. Locked Temurin 21.0.8+9, `javac --release 8` and R8 8.3.37 D8 min-api 21
produce the input and independent classfile declarations. Object facts reuse
D009 certification. The public Engine regression covers factory allocation,
String/wide/FP/boolean/narrow super and this arguments and unknown constructor
throws. Both policies compile all recovered Case/Box/Parent methods in their
original class contexts, retaining only the original helper and Driver. Five
observations per policy match the reference, including super-before-body traces,
exact FP bits and a failing super constructor that skips the body. Artifacts are
under `.cache/e2e/constructor-types/`; these are method-context observations.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run --case D043 --case D018 --case D019 --case D022 --case D023 --case D060 --case D071
python3 tools/audit/desensitize_check.py --range HEAD
git diff --check
```

On macOS arm64, Rust gates pass with 133 tests and zero skips. Fixed
`run-7b2dmew6` passes seven selected cases and fourteen invocations/seams.
ART and other hosts remain not-run. Decision: the reviewed constructor argument
restriction is corrected without moving effects or bypassing source binding.

## Integrated fixed regression

Full `run-r0ex1_dw` executes all 86 required cases, 177 invocations and 181 seams.
It retains the same 51 passing cases and 117 passing seams as `run-9zorol_p`,
with no lost passing seam or lost Java member. All 2379 original member
observations were compared. D050's enum constructor remains Debug/Unsupported;
its reason advances from InitializationUnavailable to BranchJavaUnsupported
because its String/int prologue now reaches the later source-profile rejection.
Every other member state, language, kind, code and cause is unchanged. The full
command exits 1 for remaining obligations, including complete class assembly.
The full log and per-member comparison are in `.cache/e2e/constructor-types/`.
