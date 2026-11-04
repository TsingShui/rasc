# P2 wide-copy proof trial

The entry `wide_copy.rs` includes `../src/p2/wide_copy.rs`, the exact Rust source
called by the test-only `p2/replay.rs`. It does not copy the executable function
into a model. Ordinary Cargo builds exclude the replay module and this predicate
entirely; test builds erase ghost annotations. Neither requires a Verus Cargo
dependency or runtime process.

## Property and assumptions

The acceptance predicate checks two captured-input word writes against an
original wide source and destination. Its contract establishes both:

- Acceptance is equivalent to the required destination identities, exact Two-slot
  source identities, ordered word components and non-overflowing addresses.
- For every old register-word sequence containing the source and destination,
  and every valid register index, an accepted patch has the same value as the
  DEX wide-copy rule. Both right-hand sides read the old sequence. This covers
  self-copy, overlap in either direction and unchanged words outside the target.

The DEX rule is based on the overlapping `move-wide` requirement in the
[DEX bytecode specification](https://source.android.com/docs/core/runtime/dalvik-bytecode).
The P2 model interprets `WordValue::Input` as an already captured old-slot value.
That interpretation, the correspondence between the formal DEX rule and the
specification, Verus/vstd, the SMT solver and Rust compilation remain trusted.
`--no-cheating` rejects custom assumptions and skipped executable bodies; it does
not eliminate the toolchain's trusted base.

Unproven: the Rust adapter from decoded instructions and symbolic writes into
this predicate, the remaining replay/effect/transfer checks and their whole-method
composition. The theorem does not establish P1 correctness, allocation-failure
retention or ART admission. It is not a correctness proof for all of P2.

## Reproduction

Use the official Verus release
[`0.2026.11.02.8bf68ee`](https://github.com/verus-lang/verus/releases/tag/release/0.2026.11.02.8bf68ee),
commit `8bf68ee741df833ea5d0fe6a2255126fdea87e47`, with its Rust 1.99.1 toolchain
and bundled vstd/solver. Set `VERUS` to that installation's executable. The
workspace's normal Rust toolchain does not need to change.

Run from the repository root:

```sh
"$VERUS" crates/diana-decomp/proofs/wide_copy.rs \
  --crate-type lib --edition 2024 --no-cheating
```

Acceptance: verification completes without errors, checking the executable
`wide_copy_matches` body and its quantified contract. A selected-function run,
skipped body, solver timeout or merely successful Rust compilation is not a pass.
