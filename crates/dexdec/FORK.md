# Fork notes — dexdec

Vendored from [`asLody/dexdec`](https://github.com/asLody/dexdec) at commit
`08ed478781ddddb16381c31b9cf6d8946fdf2919` (fetched 2026-09-14). Apache-2.0;
the upstream `LICENSE` is kept as `LICENSE` in this directory.

`rasc` uses this crate for one thing: `getclass`'s native Java emitter, through

```rust
dexdec::Decompiler::from_bytes(dex_entry_bytes)
    .with_options(DecompileOptions::default())
    .class(descriptor)
```

The integration seam is the **DEX entry bytes**, not rasc's own parse tree:
dexdec's emitter is driven by its frontend/IR (`frontend::ClassNode`,
`ir::CFG`), so there is no adapter from `rasc-dex`'s structures and none is
planned. `src/analysis/emitter.rs` in rasc owns that seam.

## What this tree is

A snapshot, not a tracking fork: no `upstream` remote, no rebase obligation, no
attempt to send anything back. `rusty-dex/` is the DEX parser dexdec was built
against (upstream [`rusty-rs/rusty-dex`](https://github.com/rusty-rs/rusty-dex)
0.2.0 plus this repository's extensions); it is vendored too, as a workspace
member, and has its own `FORK.md`.

## Deliberate differences from upstream

| Change | Why |
|---|---|
| upstream `cli/`, `bin/` and test/example suites not vendored; two rasc maintenance examples are retained locally | rasc is the shipped CLI; the fork itself is a library, while the examples exercise its byte-entry seam |
| package fields are literal, not `*.workspace = true` | upstream inherits them from its monorepo root; this tree builds as a member of the rasc workspace |
| `mimalloc` dropped | upstream's binary set a global allocator; nothing in the library uses it |
| embedded platform-symbol codec, database and builder removed; the default set is empty | rasc does not ship that optional source-precision database |
| `zip` / APK reading removed (`apk` feature, `from_file`, `from_raw_dex_file`, `Decompiler::open`, `load_dex`) | rasc finds the DEX entry itself and passes bytes; an archive-loading path in the library was dead compatibility code |
| all stage timing goes through `crate::timing::Instant`, currently a `std::time::Instant` re-export | one import point keeps the instrumentation implementation explicit |
| optional `profiling` / `hotpath` integration removed | rasc does not expose that upstream profiling interface; existing `DEXDEC_*_STATS` diagnostics remain |
| `[lib] test = false` | upstream's unit tests stay in-tree for reference, but should not run from rasc's `cargo test --workspace` |
| Kotlin source output removed (backend, printer, `@kotlin.Metadata` decoder, `SourceLanguage`, batch language selection) | rasc only ever requests Java; keeping the Kotlin half alive left a language-detection API, a Kotlin-defaulted `DecompileOptions`, and 27% dead code. DEX-level Kotlin facts that shape Java output stay in the shared IR and Java backend |
| platform symbol database and its consumers removed (`platform_symbols/`, `PlatformClassSet`, `preload_platform_symbols`, `platform_exception_contracts`, `platform_hierarchy_index`, `TypedConstantNameInference`, the platform fallbacks in `JavaSourceAbi`) | the dexsym database was already gone, so `default_platform_symbols()` returned an empty set and every platform lookup returned `None`; the code could not affect Java output |
| CFG/IR visualizer removed (`visualizer/`, `method_to_dot`, `method_to_text`) | rasc has no caller |
| batch/archive, catalog, reference-search and method-level API removed (`ClassBatch`, `generate_classes`, `ClassSelector`, `BatchSummary`, `ClassFailure`, `catalog`/`member_catalog`/`visit_members`, `references`, `method`/`MethodRequest`/`MethodOutput`, `source_path`, `load_archive_selection`, `prefetch_decoded_methods`, `apply_archive_termination`, `DexFileReader::find_references`/`visit_member_declarations`) | rasc calls exactly `Decompiler::from_bytes(..).with_options(..).class(..)`; everything else served upstream's GUI/full-archive export |
| `default` features empty | the CLI features are gone |
| crate-root warning/Clippy allowances | this large upstream snapshot is validated by build/tests; focused `rasc --no-deps` Clippy covers MCP-owned code, while the workspace-wide Rust 1.93 `-D warnings` invocation remains executable without rewriting unrelated upstream style/dead-code paths |

## Platform symbols

The platform-symbol layer is gone, not merely disabled: `platform_symbols/`, the
symbol-driven name recovery, and the platform parents/exception contracts that
`JavaSourceAbi` consulted are deleted. Re-enabling it means re-vendoring those
modules from upstream and re-attaching the fallbacks; there is no runtime switch.
