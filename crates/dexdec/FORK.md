# Fork notes — dexdec

Vendored from [`asLody/dexdec`](https://github.com/asLody/dexdec) at commit
`08ed478781ddddb16381c31b9cf6d8946fdf2919` (fetched 2026-09-14). Apache-2.0;
the upstream `LICENSE` is kept as `LICENSE` in this directory.

`rasc` uses this crate for one thing: `getclass`'s native Java emitter, through

```rust
dexdec::Decompiler::from_bytes(dex_entry_bytes)
    .with_options(DecompileOptions::default().with_language(SourceLanguage::Java))
    .class(descriptor)
```

The integration seam is the **DEX entry bytes**, not rasc's own parse tree:
dexdec's emitter is driven by its frontend/IR (`frontend::ClassNode`,
`ir::CFG`), so there is no adapter from `rasc-dex`'s structures and none is
planned. `src/emitter.rs` in the rasc CLI owns the seam and the smali fallback.

## What this tree is

A snapshot, not a tracking fork: no `upstream` remote, no rebase obligation, no
attempt to send anything back. `rusty-dex/` is the DEX parser dexdec was built
against (upstream [`rusty-rs/rusty-dex`](https://github.com/rusty-rs/rusty-dex)
0.2.0 plus this repository's extensions); it is vendored too, as a workspace
member, and has its own `FORK.md`.

## Deliberate differences from upstream

| Change | Why |
|---|---|
| `cli/`, `bin/`, `tests/`, `examples/` upstream files not vendored | rasc is the CLI; the fork ships a library only |
| package fields are literal, not `*.workspace = true` | upstream inherits them from its monorepo root; this tree builds as a member of the rasc workspace |
| `mimalloc` dropped | upstream's binary set a global allocator; nothing in the library uses it |
| **embedded platform symbol database dropped** (`resources/symbols/platform.dexsym`, 2 MiB, plus `codec.rs`, `crc32fast`, `zstd`, the `symbol-builder` feature and the `zip`/`quick-xml`/`abxml` deps behind it) | it is an optional precision aid (platform generics, `@Override` detection, named constants, nullability), not a control-flow input. The 2 MiB blob + zstd C code were the wasm blocker; a sample of eight real classes was byte-identical with and without it. `default_platform_symbols()` now returns `PlatformSymbolSet::empty()` |
| `zip` / APK reading moved behind `rusty-dex`'s `apk` feature (default off) | rasc finds the DEX entry itself and passes bytes; the fork must not link native compression backends |
| `[lib] test = false` | upstream's unit tests stay in-tree for reference, but should not run from rasc's `cargo test --workspace` |
| `default` features empty | the CLI features are gone |

## Re-enabling platform symbols

Restore `codec.rs`, `resources/symbols/platform.dexsym`, the `symbol-builder`
feature and its modules, and turn `default_platform_symbols()` back into the
decoder. Native-only is cheap; wasm needs a pure-Rust zstd or a
pre-decompressed blob. The upstream `DEXDEC_SYMBOLS=<file>` runtime override is
gone with the codec.
