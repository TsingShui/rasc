# Numeric field producers

Goal: extend the existing field operation chain to float, long and double
reads/writes. Contracts: TASKS T15, DESIGN §3.4/§4, ALGORITHMS §2/§4/§9.4.
The frozen specification remains draft-7. Existing D024–D028, D073/D074 and
D080 evidence retains its original acceptance strength.

## Contract audit

The current shared chain supports int instance/static fields, int/Object array
access, builtin explicit casts and checked primitive FreshArray/LengthOnly fill.
The static-int working changes remain intact; their public regression and JVM
evidence is recorded in `t15-shared-static-fields.md`. Whole-T15 still needs
broader field/array types, instanceof and nominal cast/use obligations, plus
complete F15/M13/B(F15) evidence. D044 whole-class initialization belongs to T20
and is not treated as a T15 pass.

T16's producers are absent: P2 rejects protected methods, P3 has only normal
incoming edges and no ExceptionAt definition. T07's existing fixed-Phi normal
producer is present. T16 starts after T15's required producers and evidence are
ready; it extends these same graphs and SSA arenas rather than another method
entry. Its Java protected output is owned by T18.

## Producer and invariant

P2 retains numeric field descriptors in operations and assigns their actual word
width to definitions/uses. P3 consumes those widths. P4 seals declaration flags,
constantness and source owner/name obligations. P5 derives producer kinds and
field-store conversions from those descriptors before storage selection.
P6/P7 retain the same checked operations and copies. P8 independently replays
opcode, descriptor width, original operand/reference and value conversions;
P9 renders sealed statements only.

Invariant: descriptor families never become a numeric bitcast; a wide write
consumes both original words before any destination update. Unused reads retain
the original field access and its null/init/volatile effects. Missing facts,
constant-inlined reads and unsupported conversions keep classified raw results.

Existing public field-composition coverage checks native volatile consistency
and static constantness; the numeric extension needs independent bit/value and
failure-prefix observations through those public boundaries. All new traversals
and allocations use the existing method ledger and cancellation checkpoints.

## Implementation and evidence

`normal::numeric_field_type` reads the original field descriptor; field Ops
retain ScalarType, and their result/read widths feed the existing SSA word
ownership validator. `field_facts::collect_numeric_field` retains the same
native/catalog/constantness checks. `numeric::expected` constrains store uses
before family sealing; `solve_fields` consumes its checked literal or identity
view. P8 reuses the existing independent numeric-view replay for field stores.
P1 `validate_references` now rejects descriptor/opcode-family mismatches for all
field formats rather than allowing a later wide-read error to hide the original
invalid access.

The engine's `tests/fixtures/numeric-fields/` contains self-made Java and Smali,
a compact catalog projected from the original Java classfiles using the existing
independent reader, and the two DEX inputs. Existing Object facts are reused;
there is no new JDK scan. The repository's existing fixture licensing applies.
Build the Java fixture with locked Temurin 21.0.8+9 `javac --release 8`, then
D8 8.3.37 `--release --min-api 21` over its fixture classfiles. Assemble
`Invalid.smali` with locked smali 2.5.2. Actual inputs, original classfile
projections and command results are retained under `.cache/e2e/t15-numeric-fields/`.

The existing field-composition public test is extended with numeric rows in both
representation policies, preserving the missing/contradicted-volatile evidence
checks. A distinct malformed-input test checks overwritten wide-load words,
partial wide-store words and a wide opcode targeting a float field. The latter
exposed the P1 descriptor-validation gap before the correction.

Both policies' emitted Case methods compile with Java8 targeting in the original
helper context. All five fixed Driver observations match independent original
Java, including long values, FP signed-zero/infinity/NaN payload bits, null and
helper-throw prefixes, unchanged fields on failure, and unused reads. The
production methods introduce no bit-reinterpret helper calls. The sources and
Driver carry the behavior; `.cache` retains original/actual observations.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run --case D024 --case D025 --case D026 --case D027 --case D028 --case D073 --case D074 --case D080
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64 all Cargo gates and 124 core tests pass with zero skips. Diff and
sensitive-input checks pass. Related `run-khn4qhph` passes every selected
invocation. Full `run-pz027ipo` executes 86 cases/177 invocations/181 seams and
exits 1 with 51 cases/117 seams passing. Its passing seam set and every original
member's state/failure classification equal `run-ix2eoiq7`. Existing required
Java failures remain failures. ART and other host platforms are not run.

The numeric-field slice is verified. Whole-T15 remains pending; the next slice
extends array component typing and word widths through these same products.
