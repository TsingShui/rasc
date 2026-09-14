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
| 2 | no `cli/`, `bin/`, upstream `tests/` or `examples/` | rasc is the CLI; the fork ships a library, and upstream's 626 tests stay in-tree but do not run |
| 3 | `mimalloc` dropped; `default` features empty | upstream's binary set a global allocator; nothing in the library used it |
| 4 | platform symbols: `codec.rs`, `resources/symbols/platform.dexsym` (2 MiB), the `symbol-builder` modules and the `zstd`/`crc32fast`/`zip`/`quick-xml`/`abxml` deps are gone; `default_platform_symbols()` returns `PlatformSymbolSet::empty()` | it is a type/name precision aid, not a control-flow input; a sample of real classes was byte-identical without it, and the blob + zstd were a wasm blocker |
| 5 | `zip`-based APK loading moved behind `rusty-dex`'s `apk` feature (wired to dexdec's `apk` feature); `from_file`'s APK branch and `from_raw_dex_file` are `#[cfg(feature = "apk")]` | rasc finds the DEX entry itself and hands over bytes; native compression backends must not be linked |
| 6 | `src/timing.rs`: a `crate::timing::Instant` shim, used by every former `std::time::Instant::now()` call site; on `wasm32-unknown-unknown` it returns a zero duration | `Instant::now()` panics on that target (no WASI, no clock) and took the whole browser host module down; every use is instrumentation (`mark(&mut stages, …)`), never behaviour |
| 7 | `[lib] test = false` and `[lints.rust] warnings = "allow"` | third-party tests must not run from `cargo test --workspace`, and upstream's ~90 warnings must not fail rasc's zero-warning build gate |
| 8 | lazy id pools: `types`/`protos`/`fields`/`methods` store index rows (`u32`, `ProtoRow`, `FieldRow`, `MethodRow`) and expose `descriptor`/`render` on demand; `DexFile::merge` is `apk`-gated and returns `DexError::MergeUnsupported` | rendering every id-pool name at parse dominated a 10 MiB DEX (types 3.2 ms + protos 0.7 ms + fields 5.0 ms + methods 7.0 ms); once the pools are lazy and store file-local string/type indices, concatenating and sorting rendered names across DEX files cannot be expressed |
| 9 | class directory stores id indices (`class_idx`, `superclass_idx: Option<TypeIdx>`, `interfaces: Vec<TypeIdx>`, `source_file_idx: Option<StringIdx>`); `ClassDefItem::class_name`/`superclass`/`interfaces`/`source_file` resolve through `&DexTypes`/`&DexStrings` on demand, and `DexFile::get_classes_names`/`get_class_def` resolve the same way | storing `String` descriptors for all 24,436 classes of a 10 MiB DEX cost ~4 ms of the class-directory parse even when the decompile never read a class; the descriptors now decode only for callers that ask |

Patch 6 is the only wasm-behaviour patch. Patch 8 changes behaviour only for the
`apk` merge entry point (`rusty_dex::parse` / `DexFile::merge`), which the default
build never compiles; every other parse path renders the same text as before.
Patch 9 is representation-only: the resolving accessors render the same
descriptor and source-file text the eager decoder stored. Patch 4 is the one to
revisit if rasc ever wants platform generics, `@Override` detection or named
platform constants back.
