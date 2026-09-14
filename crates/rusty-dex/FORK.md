# Fork notes — rusty-dex

Vendored as part of the `dexdec` snapshot (see `crates/dexdec/FORK.md`):
upstream [`rusty-rs/rusty-dex`](https://github.com/rusty-rs/rusty-dex) 0.2.0
plus the extensions `asLody/dexdec` carries (debug info, declarations, encoded
values, references). Apache-2.0; `LICENSE` kept as shipped.

It is the DEX **parser** only — no CFG, SSA, structuring or emission. In rasc it
exists because `dexdec` depends on it; nothing else in the repository uses it
yet.

## Deliberate differences from upstream

| Change | Why |
|---|---|
| `zip` is optional, behind the `apk` feature (off by default) | the zip backends (`deflate-zlib`, `bzip2`, `zstd`) are C code and were the wasm blocker; rasc and dexdec both consume DEX bytes |
| `DexArchive`, `DexReader::build_from_file`, `parse()` and the `InvalidArchive` error variant are `#[cfg(feature = "apk")]` | same reason; without the feature `from_file` rejects non-DEX input with a typed error |
| unused imports guarded by the same cfg | keep the default build warning-clean |

## Possible next use

If the last `rasc-dex` dependency is ever removed, this crate is the natural
home for a DEX→smali fallback printer (decoded instructions are already here),
which is the reason it is kept as a workspace member rather than a private
dependency of `dexdec`.
