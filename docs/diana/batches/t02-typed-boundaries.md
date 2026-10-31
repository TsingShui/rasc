# Typed input boundaries

Goal: apply the approved Rust conventions to existing input APIs before adding
instruction decoding. The operator authorizes the coordinate/ID and method-body
API migration; original selectors, input bytes and expected semantics stay fixed.
Producer: T01/T02; contracts: DESIGN §2/§3.2/§5.1, ALGORITHMS §1.1/§13.
Preparation: unchanged accepted schema 2, draft-7 evidence.

Fixed obligations: existing T02 F01/M01/M02 public tests and downstream
D055/D059/D062/D063/D086. No fixture certification or expected-value revision.

Not in scope: changing input acceptance or introducing semantic method verification without its validator.

Use distinct byte coordinates/lengths, code-unit coordinates/counts and table IDs
at module boundaries. Replace optional method bodies with an explicit enum;
retain independent declaration/code failures. Range-checked code construction
belongs to the code reader, with read-only access for subsequent consumers.

Implemented `substrate/ids.rs` for byte, code-unit, UTF16 and register-word
coordinates/counts and the six consumed table-ID types. Cursor/header/index/code
interfaces now carry these types. `TableKey` selects the matching ID table and
stride together. `MethodBody` distinguishes code from declaration-only members;
range-checked code fields and MUTF-8 length are read-only.

`TypeShape`, class-data member/replay state, name syntax and quota source use
explicit enums. Independent declaration/body Results and ordinary predicates
remain separate. Error propagation, local failures, input recipes and expected
semantics are unchanged; tests only adapt their public-result projection.

Platform: macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149. Default-feature
contracts passed 34 tests; the final all-features run passed 34 with no skips.
The following gates passed after correcting a Clippy parity-check warning:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
```

Artifacts: `target/debug/`, `target/release/`. No new mechanical getter, enum or
auxiliary-tool tests were added. Source review checked validation ordering and
that original byte/ID projections retain the existing independent assertions.
RawMethodView still proves ranges only; an instruction/semantic validator owns
any stronger product. Next: the stateless instruction-layout seam of T03.
