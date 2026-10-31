# T04 version-aware debug positions

Goal: validate original debug addresses using the selected method's DEX version.
Producer: the native-container work item `83df690`, whose `RawMethodView` retains
version and logical origin. Contracts: ALGORITHMS §1.2–§1.4/§7. Reuse unchanged
F03/M04 and draft-7 preparation under TASKS §3.

Coverage review: existing public snapshot tests own debug positions and metadata
failures. Extend the existing positions test with a supported newer-width
instruction before a debug address; the independently specified PC must remain
decodable under its own version. Code review found that the debug boundary reader
still hard-codes 035 despite the selected method retaining its actual version.

Red: the extended positions test failed with Unsupported for a known-width
039/040 instruction preceding the debug address. `DebugProgram::check_address`
now uses `RawMethodView.version` for both its forward scan and current-position
check. The same public cases pass with the independent address and line retained.
No new parser, quota or cancellation path is introduced.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t04_debug_program_positions)' --status-level fail --final-status-level fail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked --status-level fail --final-status-level fail
python3 tools/audit/desensitize_check.py --range HEAD
git diff --cached --check
python3 tools/audit/desensitize_check.py --staged
```

The workspace run passes all 110 core tests without skips. Format, Clippy and
release build pass. TASKS.md remains unchanged. Complete P1 sealing and the
remaining T/H acceptance obligations still require their existing evidence.
