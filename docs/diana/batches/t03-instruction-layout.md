# T03 stateless instruction layout

Goal: expose the shared `raw::instruction_layout` seam with typed code-unit PCs,
complete instruction/payload ranges and original indexed operands. Producers:
T02 range views and the approved typed-boundary migration. Contracts: DESIGN
§3.2, ALGORITHMS §1.2/§7/§13. Reuse unchanged B01–B04 schema 2, draft-7 evidence.

Fixed obligations: D059 unknown width, D055 range safety and the T03 F02 original
instruction facts. Unit-level layout evidence does not accept whole D cases.

Not in scope: whole-method branch/register/catalog verification and debug rendering.

Use one opcode specification for format, minimum version, mnemonic and indexed
operand kind. The layout product is constructed only after its format/range
checks. The helper is stateless and constant-work; callers charge/check their
iteration. Variable payload key/target semantics stay with the owning validator.

Files: `src/p1_input/formats.rs`, the public `raw` re-export, two consumed
reference-ID newtypes and `tests/input/layout.rs`. Functions: `instruction_layout`,
`opcode_spec`, format width, checked word/span reads and payload layout.
Opcode facts follow ALGORITHMS §1.2 and the existing cached AOSP
`dalvik-bytecode.html` (SOURCE_MAP). Packed/sparse/array extents use checked
arithmetic; odd array padding is not treated as another element or given an
invented zero-value requirement.

Platform: macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149. The first selector
failed to compile while the formats module was absent. The initial targeted run
passed 7 layout tests, filtering 34 regression tests. After adding typed reference
domains and nonzero-PC coverage, the all-features workspace run passed all 42
core tests (8 layout and 34 regression), with no skips:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t03_)'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
```

The tests use independent original-word recipes, not production registry counts.
Coverage includes truncating every representative fixed instruction, raw jumbo
and dual references, reserved words/padding, version mismatch, payload overflow,
alignment, and an unknown encoding after a readable prefix. Source review checked
all guarded mnemonic-array indexes, version gates, private layout construction,
constant-work payload bounds and unresolved reference ownership.

Artifacts: `target/debug/` and `target/release/`. Fixed engine invocations remain
unexecuted; T03 acceptance requires the original-information snapshot and T05
adapter evidence. Next: whole-method instruction/reference/branch checks and
original debug facts, with the existing method and request resource controls.
