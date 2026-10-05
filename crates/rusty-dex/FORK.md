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
| `DexArchive`, `DexReader::build_from_file`, `parse()` and the `InvalidArchive` error variant are `#[cfg(feature = "apk")]` | same reason; without the feature `from_file` rejects non-DEX input with a typed error |
| unused imports guarded by the same cfg | keep the default build warning-clean |
| narrow Clippy cleanups (`&str` lookup inputs, direct `Option` chaining, range/alignment expressions, checked string offsets) | keep the Rust 1.93 workspace-wide `-D warnings` gate green without changing parser behavior |
| `too_many_arguments` allowed on DEX class-table builders | the parameters correspond to independent DEX tables/offsets; bundling them would obscure the file-format boundary |

## Workspace role

The crate remains a workspace member because its lazy DEX model and instruction decoder are
maintained directly alongside `dexdec`, rather than hidden as an unreviewed private dependency.
