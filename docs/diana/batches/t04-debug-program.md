# T04 original debug programs

Goal: expose checked original debug program events in the existing method
snapshot, retaining a decoded prefix and the actual error after local damage.
Producers: T02 borrowed code/prototype views and T03 instruction layouts and
fallback JSON writer. Contracts: ALGORITHMS §1.3–§1.5/§7 and DESIGN §2.3/§3.7.
Fixed obligations: F03/M04, D029/D055/D062/D063. Reuse unchanged draft-7
B01–B04 certificates and comparator evidence from TASKS §3.

Not in scope: Java local naming and the complete P1 seal.

The existing fallback ledger charges parser work, local-state scratch and output.
Reads remain bounded by the original metadata section; cancellation is terminal.
Expected values remain independent of engine output in `e2e_cases.json` and
self-made public core recipes.

Source review: AOSP [debug_info_item](https://source.android.com/docs/core/runtime/dex-format#debug-info-item)
defines the opcode grammar, parameter names and line/local state. This batch
independently implements that contract; it copies no reference implementation.

Changed functions: `DebugProgram::open` checks the header, parameter references
and frame-derived local scratch before reservation; `next` decodes opcodes,
checks arithmetic and tracks per-register local lifetimes. `check_address` walks
the shared instruction layout in increasing address order. The existing reader
supplies classified LEB/reference errors. `write_program` emits original events
and resolved UTF16 facts through the existing fallback writer. `note_error`
retains the concrete error, missing metadata and the decoded prefix. A partial
byte range ends after the last completed event, before the failing opcode.

Red: both initial public snapshot tests ran and failed: valid metadata was absent
and an out-of-range debug address had no Malformed result. Green: 14 new public
core tests cover every debug opcode, original/noncanonical bytes, UTF16 names,
NO_INDEX, implicit parameters, wide locals, lifecycle errors, invalid references,
line/address overflow, instruction/payload boundaries, truncation, method-local
isolation, exact resource bounds and terminal cancellation.

Manual original-input evidence uses the locked Temurin 21.0.8+9, R8 8.3.37 and
dexlib2 2.5.2. A self-made `DebugCase.java`, compiled with `--release 8 -proc:none -g`
and D8 `--debug --min-api 23`, retains actual debug metadata. The public Rust
snapshot agrees with the independent dexlib2 projection on two methods, nine
semantic debug events and both parameter names. The original certified D029
input also produces nine method snapshots without a failure; that input has no
debug events and therefore does not establish this batch's metadata capability.

Artifacts under `.cache/e2e/t04-debug-program/`: `DebugCase.java`, `classes/`,
`dex/`, `logs/`, `dexlib.json`, `snapshots/`, `comparison.json` and the public
library probe `probe.rs`. Original D029 observations are under `d029/`. These
are local core-operation evidence, not new fixed Engine acceptance values.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t04_debug_program)'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked --status-level fail --final-status-level fail
python3 tools/audit/desensitize_check.py --range HEAD
```

The closing all-features workspace run passes 108 core tests without skips.
Format, Clippy and release build pass. The temporary-index audit includes all new
files while preserving the real index. Unchanged preparation artifacts are reused
without fingerprint verification.

Remaining obligations: T04 encoded values/annotations and native 041, the complete
P1 seal, T03 Report spans and T05 Engine integration retain their acceptance
requirements.
