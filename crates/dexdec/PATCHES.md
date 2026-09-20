# Patches carried by this fork

Vendored snapshot of [`asLody/dexdec`](https://github.com/asLody/dexdec) at
`08ed478781ddddb16381c31b9cf6d8946fdf2919`, plus `../rusty-dex` (upstream
[`rusty-rs/rusty-dex`](https://github.com/rusty-rs/rusty-dex) 0.2.0 with this
repository's extensions). Apache-2.0; `LICENSE` kept as shipped. `FORK.md` says
why the fork exists; this file is the delta list a re-vendor would have to
re-apply.

| # | Patch | Why |
|---|---|---|
| 1 | package fields are literal, not `*.workspace = true`; the crate builds as a member of the rasc workspace | upstream inherits them from its monorepo root |
| 2 | no upstream `cli/`, `bin/`, test suite or example suite; two rasc maintenance examples are retained | rasc is the shipped CLI; the fork is a library, while its local examples exercise the integration seam |
| 3 | `mimalloc` dropped; `default` features empty | upstream's binary set a global allocator; nothing in the library used it |
| 4 | embedded platform symbols (`codec.rs`, `resources/symbols/platform.dexsym`, symbol-builder modules and their codec dependencies) are removed; `default_platform_symbols()` returns an empty set | platform symbols are optional source-precision aids, not control-flow input; rasc does not ship the database |
| 5 | `zip`-based APK loading moved behind `rusty-dex`'s `apk` feature (wired to dexdec's `apk` feature); `from_file`'s APK branch and `from_raw_dex_file` are `#[cfg(feature = "apk")]` | rasc finds the DEX entry itself and hands over bytes; archive loading remains optional compatibility code |
| 6 | `crate::timing::Instant` is a shim that currently re-exports `std::time::Instant` | all stage instrumentation uses one import point without target-specific clocks |
| 7 | `[lib] test = false` and `[lints.rust] warnings = "allow"` | the retained upstream-style unit suite is not part of the default workspace gate, and its legacy warnings must not hide rasc warnings |
| 8 | the optional `profiling` feature, `hotpath` dependency and `profiling` module are removed; stage statistics remain on the existing `DEXDEC_*_STATS` paths | rasc does not expose the upstream profiling API and keeps no dormant instrumentation wrappers |
| 9 | lazy id pools: `types`/`protos`/`fields`/`methods` store index rows (`u32`, `ProtoRow`, `FieldRow`, `MethodRow`) and expose `descriptor`/`render` on demand; `DexFile::merge` is `apk`-gated and returns `DexError::MergeUnsupported` | rendering every id-pool name at parse dominated a 10 MiB DEX; file-local indices make the former concatenate/sort merge invalid, so the optional APK merge entry point reports `MergeUnsupported` |
| 10 | class directory stores id indices (`class_idx`, `superclass_idx: Option<TypeIdx>`, `interfaces: Vec<TypeIdx>`, `source_file_idx: Option<StringIdx>`); resolving accessors take `&DexTypes` / `&DexStrings` | descriptors decode only when requested, avoiding eager allocation for every class while preserving rendered names |

Patch 9 changes behaviour only for the optional `rusty_dex::parse` / `DexFile::merge`
entry point, which the default build never compiles. Patch 10 is representation-only:
its accessors render the same descriptor and source-file text as the eager decoder.
