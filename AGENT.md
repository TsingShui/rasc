# AGENT.md — Working in this repository

rasc is the Rust rewrite of ASC (an APK/DEX analysis tool). **One core** now has several hosts:

| Host | Product | Byte source | Output / diagnostics / clock |
|---|---|---|---|
| native CLI | `target/release/rasc` | `mmap` | stdout / stderr / `std::time::Instant` |
| host module | `target/wasm32-unknown-unknown/release/rasc.wasm` | host callback `rasc_host_read` | `rasc_host_write` / `rasc_host_write_err` / `rasc_host_now_ms` / `rasc_host_progress` |

Read this page before changing anything: it records **how to prove you did not break something**
(the gates) and **the traps already stepped on**.

---

## 1. Gates

**The CLI comes first; wasm only after the CLI is green.** A change is judged by the native CLI it
ships, so `cargo test` and the native release build come first, and the wasm build and the host
suites follow. A red CLI gate is fixed before minutes are spent on a wasm build that cannot change
the failure.

**Two targets: native and wasm.** `wasm32-wasip1` (a WASI CLI) was a development aid and is gone;
the wasm product is the `wasm32-unknown-unknown` host module.

What each gate **proves**:

| Command | Proves |
|---|---|
| `cargo test` | 86 + 2 unit/integration tests (re-derived after the record surface was removed; re-run to confirm) |
| `cargo build --release` × {native, `wasm32-unknown-unknown`} | both targets build, with **zero warnings** |
| `APK=… node js/test.mjs` | Node host SDK: the 4 command digests match native + buffer source + guarded source + read-ahead window |
| `APK=… node js/cli-test.mjs` | `js/bin/rasc-wasm.mjs` has the same args / stdout / exit code as native |
| `node js/check-declared-test-count.mjs` | the number beside `cargo test` in this very table is what the command prints |
| `APK=… node js/limit-test.mjs` | the host-configurable inflation limit: a small limit is refused, a small entry still works, the default is restored |
| `node js/bomb-test.mjs 1.5` | a decompression bomb is **rejected structurally**, not trapped |
| `APK=… node js/mutation-test.mjs 40` | archive-level mutation: 200 runs with no trap / no panic |
| `APK=… node js/dex-corpus-test.mjs` | DEX-level mutation: 135 runs (real DEXes repacked as stored entries) |
| `APK=… node js/axml-corpus-test.mjs` | 24 structural AXML mutations, each compared with native |
| `APK=… node js/dex041-corpus-test.mjs` | 36 runs over 041 containers, each compared with native |
| `APK=… node js/zip-craft-corpus-test.mjs` | 60 runs of hand-crafted ZIP consistency attacks, each compared with native |
| `APK=… MODES=file,url node js/browser-test.mjs` | a **real browser** (headless Chrome): both byte sources, 4 commands each, SHA-256 identical to native |

`bench/` holds the native CLI contract (`contracts.sh`), the wasm contract (`contracts-wasm.sh`),
the reference benchmark and the corpus fetcher. It stays on disk for local runs but is **not part of
the tree** - do not commit it. The row whose number `js/check-declared-test-count.mjs` checks against
`cargo test` has load-bearing shape: keep `` | `cargo test` | <unit> + <integration> unit/integration
tests | ``.

Environment requirements:

- The wasm target must be installed: `rustup target add wasm32-unknown-unknown`
- The browser tier needs Chrome (`CHROME=` points at another path); without Chrome that tier skips
  cleanly
- **Sample archives do not travel with the repository**: the suites marked `APK=` need one of your
  own, and any corpus whose identity must not appear in the tree is referred to by alias (see
  "Desensitization" below).
- **The gates hold for any APK**: the probes (concrete `type`/`method`/`field`/`string` values, "a
  class from the latest DEX") are taken from the archive itself, not tied to a particular sample -
  see `latestDexClass()` in `js/*.mjs`.

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

The host module is what a browser drives, so it has an interface of its own beyond the CLI's
text output: the same arguments, the same stdout bytes, the same exit code, plus the imports
the host has to answer. The record modes (`--json`, `--outline`, `--count`, `--xrefs`) were
removed - a host reads the CLI's own output, so the CLI's own output is the whole contract:

| What | Why it exists | Where |
|---|---|---|
| `entries`, `strings` | the archive's entries and the DEX string table | `src/apk.rs`, `src/dex/` |
| `strings --filter`, `--limit`, `--offset` | a page of the table rather than all of it | `src/apk.rs` |
| `rasc_host_progress(done, total)` | entries walked, out of a total known before the walk; only the serial (wasm) walk reports, because only it is ordered | `src/progress.rs` |
| clap's answers | `--help`, `--version` and a usage error return clap's status and text instead of trapping the instance | `src/main.rs` |

A new host import breaks every host that does not provide it, so `js/rasc.mjs` has to grow
with it.

---

## 3. Invariants that must not be broken

1. **Native output is the baseline.** No change on the wasm side may alter native byte output. How to
   check: the `manifest` / `classes` md5s are unchanged and the contract suites pass.
   - `manifest` = `7756cac7a9cc02b74cffe9ecb1fe5eb8`
   - `classes --threads 1` = `d839067e5a8d95fafdda2e006acc3b8e`
2. **Error text is a contract too.** wasm must match native word for word — so messages are
   **generated** in the core (`zip.rs` and friends) rather than paraphrasing a backend (libdeflate and
   miniz_oxide word things differently; `incomplete_stream()` exists for exactly that).
3. **The core only sees `&[u8]` and `BytesSource`.** Host differences are allowed only in `Archive` in
   `src/apk.rs`, the two writers in `src/main.rs`, and `src/clock.rs` / `src/diag.rs` /
   `src/progress.rs`. Do not leak
   `cfg(target…)` into the parsing logic of `zip.rs` / `dex/` / `manifest.rs` (the deflate backends
   and the branches that already exist are the exceptions).
4. **`wasm32-unknown-unknown` has no threads.** `std::thread::spawn` returns `ENOTSUP` there, so
   rasc's own parallel call sites take the serial branch under `cfg!(target_family = "wasm")`, and
   **both branches must keep compiling** (`cfg!()`, not `#[cfg]`). Vendored code may lean on
   `rayon`'s own fallback instead (it runs `par_iter` sequentially when threading is unsupported);
   `crates/dexdec` does, and it never calls `rayon::spawn`, so no work is dropped.
5. **The inflation limit is host policy**: 256 MiB per entry by default, and a host may lower it; do
   not remove the "**decide before the first allocation**" semantics.
6. **`skill/SKILL.md` is the payload of `rasc skill --print`** (embedded with `include_str!`): editing
   it edits the CLI's output bytes, and native and wasm must agree — `js/cli-test.mjs` has a
   `skill --print` comparison.

---

## 4. Traps already stepped on (the kind that cost you half a day)

| Symptom | Cause / correct approach |
|---|---|
| wasm link fails with `undefined symbol: rasc_host_*` | the `extern` block must carry `#[link(wasm_import_module = "env")]`, or the linker goes looking for a definition |
| a measurement says "no improvement / a regression", but it is false | confirm first that the **build succeeded** and the product is **newer than the sources**. I once drew a wrong conclusion from an old binary whose build had failed — scripts must assert both |
| a bare `RuntimeError: unreachable` in wasm, with no information | `std::time::Instant::now()` panics outright on `wasm32-unknown-unknown` → use `src/clock.rs` (host clock); vendored dexdec carries a `crate::timing::Instant` shim for the same reason (see its PATCHES.md); `rasc_run` installs a panic hook, so a panic appears in host output as `PANIC: …` |
| `--debug` prints nothing | `eprintln!` cannot write on `wasm32-unknown-unknown` (silently dropped) → use the host channel in `src/diag.rs` |
| a large payload is truncated (especially through a pipe) | `process.exit()` does not wait for stdout to flush → use `process.exitCode` and let Node finish naturally |
| `-o` reports `operation not supported on this platform` | the host module has no filesystem → `-o` is implemented by the CLI wrapper (`js/bin/rasc-wasm.mjs`, which deletes the half-written file on failure) |
| wasm inflation reports `deflate decompression error` | `flate2::Decompress` **does not remember** input consumed across calls → split the input by `total_in()` (the current implementation uses `bufread::DeflateDecoder` instead) |
| native memory numbers jump around, by as much as 4× | macOS malloc's large-block cache → set `MallocLargeCache=0` when comparing native memory, or you are measuring the allocator instead of the program |
| a corpus is "all green" but tested nothing | every corpus needs a **control** (the unmutated input must really parse) and a count of whether the comparison actually happened. This rule has caught 5 false conclusions in this repository |
| after installing a wasm target, the build still fails with a basic error inside a dependency (e.g. `IndexMap<K, V, S>` generic count) | the target was missing on the first build and **the build script's probe result was already cached**: indexmap only declares `rerun-if-changed=build.rs`, so a changed environment does not make it re-run → `cargo clean -p <that crate>` and build again. check the exit status as well as the warnings, or a "failed build with 0 warnings" can look green |
| a custom harness reports a "mismatch" | suspect the harness first: a different error stream (wasm has no stderr), a misspelled argv, comparing bytes from different sources. Core and harness can both be wrong, and the harness is wrong more often |

---

## 5. Layout

```
src/            core (one copy for native and wasm)
                ├── apk.rs      Archive: Mapped / Ranged / Host, plus entry scheduling
                ├── zip.rs      BytesSource, ZIP parsing, inflation and its limit
                ├── dex/        scanner (mod/container/filter/mutf8/opcodes/prefix)
                ├── manifest.rs binary AndroidManifest → XML
                ├── clock.rs    ★ host clock (Instant panics on unknown-unknown)
                ├── skill.rs    `rasc skill`: installs the embedded SKILL.md for pi / Codex / Claude Code
                └── diag.rs     ★ diagnostic channel (--debug is no longer dropped in host builds)
js/             the wasm host side
                ├── rasc.mjs    shared host: 5 imports + configurable limit + diagnostics; imports nothing (loadable in a browser)
                ├── node.mjs    fs.readSync source + 4 MiB read-ahead window
                ├── browser.mjs Worker: FileReaderSync(Blob) or synchronous XHR(Range)
                ├── worker.mjs  ready-made module-worker entry
                ├── bin/rasc-wasm.mjs  usable directly as a CLI (same args / stdout / stderr / exit code)
                └── *-test.mjs      9 gate scripts + check-declared-test-count.mjs (see section 1)
vendor/         patched crates.io dependencies (axmldecoder comes in through [patch.crates-io] as a relative path; **do not delete**)
crates/         workspace members. `crates/dexdec` is the vendored Java emitter used by `getclass` on native
                (see its FORK.md for the upstream commit and the deliberate trims); `crates/rusty-dex` is the DEX
                parser it was built against. `--emitter` was removed with the last `rasc-dex` dependency: the CLI
                now has one native emitter, and wasm builds of `getclass` report that they have none.
tools/audit/    the desensitization gate only: `desensitize_check.py` and its word list. The
                rasc-dex audit harness (cohort ledger, screens, elegance counters, criteria
                checker) went away with `crates/rasc-dex`.
docs/           release notes (`docs/releases/`). The rasc-dex trail - bug records
                (`todo-bug-XXXX-*.md`), audit logs (`audit-*.md`), the elegance baseline and
                Agent-Test-Imporve-Method.md - was deleted on 2026-09-14; `git log` has it.
tests/          self_contained.rs (no process spawning, no Python/JVM bindings)
skill/          the SKILL.md that `rasc skill` writes (embedded in the binary and the payload of `--print`)
```

When adding a host, wire it up at `Archive` in `src/apk.rs` and the writers in `src/main.rs` first,
then add the adapter in `js/` — the parsing logic should not change at all.

