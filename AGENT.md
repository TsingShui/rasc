# AGENT.md — Working in this repository

rasc is the Rust rewrite of ASC (an APK/DEX analysis tool). **One core, two targets**, and the
second one is a program like the first:

| Target | Product | Reading the archive | Output / diagnostics | Entries |
|---|---|---|---|---|
| native | `target/release/rasc` | `mmap` | stdout / stderr | rayon pool |
| `wasm32-wasip1` | `target/wasm32-wasip1/release/rasc.wasm` | `open` + `seek` + `read`, in ranges | stdout / stderr | serial |

The `wasm32-unknown-unknown` **host module is gone**: no `rasc_host_*` imports, no `rasc_run`,
no record modes (`--json`, `--outline`, `--count`, `--xrefs`), no `js/` SDK and no byte-source
protocol. A host starts the program with `argv` and a mounted directory; everything a host can
observe is in §2. `git log` has the old shape.

Read this page before changing anything: it records **how to prove you did not break something**
(the gates) and **the traps already stepped on**.

---

## 1. Gates

**The CLI comes first; wasm only after the CLI is green.** A change is judged by the native CLI
it ships, so `cargo test` and the native release build come first, and the wasm build and the
WASI suites follow. A red CLI gate is fixed before minutes are spent on a wasm build that cannot
change the failure.

What each gate **proves**:

| Command | Proves |
|---|---|
| `cargo test` | 86 + 2 unit/integration tests |
| `cargo build --release` × {native, `wasm32-wasip1`} | both targets build, with **zero warnings** |
| `APK=… node js/wasi-test.mjs` | 15 comparisons of the two targets: same stdout, same stderr, same exit code — commands, `getclass` decompilation, clap's answers, a usage error, a missing file, and the inflation ceiling |
| `APK=… node js/torture-test.mjs 40` | hostile input: a decompression bomb is refused with a message, 40 deterministic mutations and 3 truncations end in an exit code and never in a trap, and every one of them agrees with native |

Environment requirements:

- The wasm target must be installed: `rustup target add wasm32-wasip1`
- The suites start the module through Node's WASI, which needs
  `--experimental-wasi-unstable-preview1`; the scripts pass it themselves
- **Sample archives do not travel with the repository**: the suites marked `APK=` need one of
  your own, and any corpus whose identity must not appear in the tree is referred to by alias
  (see "Desensitization" below)
- **The gates hold for any APK**: the probes (a class, a string, a type) come from the archive
  itself, never from a table in this repository

### Desensitization (pre-commit)

A corpus whose identity must not appear in the repository is named by a neutral tag (`corpus-b`)
and an anonymous handle (`corpus-b/d<dex>#<n>`); reproduction rests on a self-made minimal fixture
plus `pc` offsets. `tools/audit/desensitize_check.py` checks the **added and modified lines only** —
the mentions still in the tracked files are reported by `--tree` and cleaned up separately — and
exits non-zero on a hit:

```sh
 tools/audit/desensitize_check.py --staged      # what the pre-commit hook runs
 tools/audit/desensitize_check.py --range HEAD  # the working tree against HEAD
 tools/audit/desensitize_check.py --tree        # the deferred baseline (tracked files)
```

The word list is `tools/audit/desensitize_words.txt`, the one file exempt from the scan because its
job is to hold the tokens. The script's docstring carries a one-liner that installs it as a local
pre-commit hook. Identifiers that really must be written down go to the untracked `.cache/`.

---

## 2. The host boundary

There is no ABI to keep in sync, which is the point of the WASI target. What a host owns:

| Channel | Meaning |
|---|---|
| `argv` | The command line, unchanged. The archive argument is a **real path** the guest opens; there is no label-only mode. |
| the preopened directory | Where the archive and any `-o` output live. A host holding the bytes in memory mounts them at a path instead of writing them out. |
| `RASC_MAX_INFLATED_ENTRY` | The per-entry inflation ceiling, in bytes, read once at startup. Host policy rather than command-line policy — a browser tab has a memory budget and the argument line belongs to whatever wrote it. |
| stdout / stderr | Payload and diagnostics, split exactly as natively: redirecting stdout can never capture a `--debug` line, a panic or an error. |
| the exit code | `0` success, `1` a failed command, `2` a usage error — clap's own code, never a trap. |
| instance lifetime | A WASI call cannot be interrupted from outside, so cancellation is killing the instance. One run per instance is the cheap way to have that. |

`js/wasi-run.mjs` is the reference host and the smallest expression of this table; `js/README.md`
carries the same contract from the host's side. `-o`, `skill` and bare-DEX input all work through
the mounted directory, so a host does not implement output files itself any more.

---

## 3. Invariants that must not be broken

1. **Native output is the baseline.** No change may alter native byte output. How to check: the
   `manifest` / `classes` md5s are unchanged and the parity suite passes.
   - `manifest` = `7756cac7a9cc02b74cffe9ecb1fe5eb8`
   - `classes --threads 1` = `d839067e5a8d95fafdda2e006acc3b8e`
2. **Error text is a contract too.** Both targets must match word for word, so messages are
   **generated in the core** rather than paraphrasing a backend: libdeflate and the Rust decoder
   word things differently, and `incomplete_stream()` exists for exactly that. Where a decoder
   folds two different failures into one answer, the core has to tell them apart itself — the
   whole-entry inflate path in `zip.rs` walks `flate2::Decompress` by hand for this reason.
3. **The core only sees `&[u8]` and `BytesSource`.** Host differences are allowed only in `Archive`
   in `src/apk.rs`, the writer in `src/main.rs`, and `src/diag.rs`. Do not leak `cfg(target…)` into
   the parsing logic of `zip.rs` / `dex/` / `manifest.rs`; the deflate backends and the branches
   that already exist are the exceptions.
4. **This wasm target has no threads.** `std::thread::spawn` returns `ENOTSUP`, so rasc's own
   parallel call sites take the serial branch under `cfg!(target_family = "wasm")`, and **both
   branches must keep compiling** (`cfg!()`, not `#[cfg]`). Vendored code may lean on `rayon`'s own
   fallback instead (it runs `par_iter` sequentially when threading is unsupported); `crates/dexdec`
   does, and it never calls `rayon::spawn`, so no work is dropped.
   - `wasm32-wasip1-threads` does exist (tier 2, full `std`), and `wasi_threads` is a real proposal,
     so "WASI cannot do threads" is the wrong sentence. What is true is narrower: this target is not
     the `-threads` variant, the module imports no shared memory, and neither host we run on
     implements `wasi_thread_spawn` — Node's `node:wasi` does not export it, and
     `browser_wasi_shim` does not mention it. See the trap row below before reaching for it.
   - Order of work matters more than the feature: the WASI build is serial *and* on the Rust
     inflate backend, and measured on the same archive `classes` is 320 ms there against 33 ms
     natively. Threads would split the serial half; a real deflate backend would shrink both.
   - **Decided: stay on plain `wasm32-wasip1`.** The threads variant compiles (`rustup target add
     wasm32-wasip1-threads`, 46 s, zero warnings) but its module then imports `env.memory` (shared)
     and `wasi.thread-spawn`, and no host we run on implements the second one — Node's `node:wasi`
     does not export it and `browser_wasi_shim` does not mention it. Plain WASI runs in every host,
     which is worth more here than the parallel walk. If it ever matters, the path is the
     `-threads` variant plus ~80 lines of host glue that instantiates the module again on a shared
     memory and calls the exported `wasi_thread_start`; **not** wasm-bindgen-rayon, which would cost
     the program model, a nightly `build-std`, and COOP/COEP on a host that cannot set headers.
5. **The inflation limit is host policy**: 256 MiB per entry by default, lowered by the host
   through the environment, and it keeps its "**decide before the first allocation**" semantics —
   it is read once before dispatch, never per entry.
6. **`skill/SKILL.md` is the payload of `rasc skill --print`** (embedded with `include_str!`):
   editing it edits the CLI's output bytes, and both targets must agree — the parity suite has a
   `skill --print` comparison.

---

## 4. Traps already stepped on (the kind that cost you half a day)

| Symptom | Cause / correct approach |
|---|---|
| a measurement says "no improvement / a regression", but it is false | confirm first that the **build succeeded** and the product is **newer than the sources**. I once drew a wrong conclusion from an old binary whose build had failed — scripts must assert both |
| the same corrupt entry gives two different messages on the two targets | a decoder that reports "ended early" and "wrong size" with one return value folds two claims into one. `read_to_end` over a truncated deflate stream does exactly this; walk `flate2::Decompress` and break on `Status::StreamEnd` instead |
| inflating a legitimate entry suddenly reports an incomplete stream | `FlushDecompress::Finish` tells the decoder the output buffer must be able to complete the stream in one call. A loop that grows its buffer must pass `None` |
| a size test that only ever passes | a stream that produces *more* than the declared size cannot be finished natively either (libdeflate reports `InsufficientSpace` past the declared size) — it is an incomplete stream, not a size mismatch, and the wasm path has to say the same |
| native and WASI stderr differ on a missing file | the errno number belongs to the host's table (`os error 2` natively, `os error 44` under WASI). The wording, the stream and the exit code are the contract; fold only the number away when comparing |
| a WASI run cannot open the archive it was pointed at | a WASI guest has no working directory: hand it an absolute path its preopens cover |
| a large payload is truncated (especially through a pipe) | `process.exit()` does not wait for stdout to flush → use `process.exitCode` and let Node finish naturally |
| native memory numbers jump around, by as much as 4× | macOS malloc's large-block cache → set `MallocLargeCache=0` when comparing native memory, or you are measuring the allocator instead of the program |
| a corpus is "all green" but tested nothing | every corpus needs a **control** (the unmutated input must really parse) and a count of whether the comparison actually happened. This rule has caught 5 false conclusions in this repository |
| after installing a wasm target, the build still fails with a basic error inside a dependency (e.g. `IndexMap<K, V, S>` generic count) | the target was missing on the first build and **the build script's probe result was already cached**: indexmap only declares `rerun-if-changed=build.rs`, so a changed environment does not make it re-run → `cargo clean -p <that crate>` and build again. check the exit status as well as the warnings, or a "failed build with 0 warnings" can look green |
| a custom harness reports a "mismatch" | suspect the harness first: a different error stream, a misspelled argv, comparing bytes from different sources, or a path the guest cannot resolve. Core and harness can both be wrong, and the harness is wrong more often |

---

## 5. Layout

```
src/            core (one copy for native and WASI)
                ├── apk.rs      Archive: Mapped (native) / Ranged (WASI), plus entry scheduling
                ├── zip.rs      BytesSource, ZIP parsing, inflation and its limit
                ├── dex/        scanner (mod/container/filter/mutf8/opcodes/prefix)
                ├── manifest.rs binary AndroidManifest → XML
                ├── skill.rs    `rasc skill`: installs the embedded SKILL.md for pi / Codex / Claude Code
                └── diag.rs     diagnostics: stderr, on both targets
js/             the host side, three files
                ├── wasi-run.mjs     the reference WASI host, also a usable command line
                ├── wasi-test.mjs    the parity gate: WASI against native
                └── torture-test.mjs hostile input: bomb, mutations, truncations
vendor/         patched crates.io dependencies (axmldecoder comes in through [patch.crates-io] as a relative path; **do not delete**)
crates/         workspace members. `crates/dexdec` is the vendored Java emitter used by `getclass`
                (see its FORK.md for the upstream commit and the deliberate trims); `crates/rusty-dex` is the DEX
                parser it was built against.
tools/audit/    the desensitization gate only: `desensitize_check.py` and its word list.
docs/           release notes (`docs/releases/`).
tests/          self_contained.rs (no process spawning, no Python/JVM bindings)
skill/          the SKILL.md that `rasc skill` writes (embedded in the binary and the payload of `--print`)
```

When adding a target, wire it up at `Archive` in `src/apk.rs` and the writer in `src/main.rs`
first — the parsing logic should not change at all.
