# T03 unknown-encoding excerpts

Goal: expose only an original-word excerpt when decoding cannot begin, without
inventing an instruction width or dropping a checked decoded prefix. Producers:
T02 canonical member lookup and T03 shared encoding facts/snapshot writer.
Contracts: ALGORITHMS §7.1–§7.3/§8, D059 and F02. Reuse independent fixed inputs
and expected values; T05 retains actual Engine acceptance.

Not in scope: Java recovery and native 041 metadata.

The first encoding lookup uses the same registry as the layout decoder. An
excerpt is a word observation, not an instruction. The source kind is explicit;
normal and raw work never restart, and the fallback ledgers remain cumulative.

Files/functions: `formats::encoding_head` supplies the same opcode/version and
payload-signature lookup to `instruction_layout` and the debug writer.
`debug::selected_method` binds the canonical declaration/body before selecting
`DebugSourceKind::Snapshot` or `WordExcerpt`; field-only traversal formats a
snapshot without reading other bodies again. A first unknown encoding produces
one observed original u16 plus an explicit undecoded range, never a width.

Red: the public excerpt cases failed compilation before source-kind support.
Green: both new cases and all 81 regression cases pass in the 83-case
all-features workspace run, with no skips. Known multiword instructions remain
snapshots rather than being mistaken for a truncated head probe. A decoded
prefix still retains its partial snapshot. Selection and output use only the
existing fallback scope; cancellation/resources remain terminal.

Closing review adds two boundary cases: invalid catch metadata retains the
canonical identity without claiming a sealed code view, and an already-known
body malformation survives failure to allocate its first issue entry. The
initial catch case incorrectly expected a code view after the checked reader
had rejected the handler descriptor; its expectation now checks that actual
boundary. The closing workspace run passes all 85 core cases with no skips.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t03_debug_unknown_first) | test(t03_debug_known_multiword)'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
python3 tools/audit/desensitize_check.py --range HEAD
```

Artifacts: `target/debug/`, `target/release/`; the temporary-index audit includes
new files without staging the real index. T03 Report spans, T04 metadata/041 and
T05 fixed public Engine invocations retain their acceptance obligations.
