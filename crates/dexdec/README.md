# dexdec (rasc fork)

`dexdec` is the vendored decompiler library used by `rasc getclass`. It is not a separately
shipped application or command in this repository; the supported product is the native `rasc` CLI.

## Role in rasc

rasc passes one DEX entry and a class descriptor to the library:

```rust
use dexdec::{DecompileOptions, Decompiler};

let options = DecompileOptions::default();
let mut decompiler = Decompiler::from_bytes(dex_bytes)?.with_options(options);
let unit = decompiler.class("Lcom/example/App;".to_owned())?;
println!("{}", unit.source);
# Ok::<(), dexdec::DecompileError>(())
```

The integration seam is the DEX entry bytes. dexdec owns its frontend, CFG/SSA, semantic IR,
source recovery, and Java printer; rasc does not adapt its own scanner model into that
pipeline. The Kotlin output backend was removed with the Java-only CLI; DEX-level Kotlin facts
that shape Java output (Intrinsics guards, inline-finally markers, default-argument masks)
remain.

## Local examples

Two small examples are retained for fork maintenance rather than as shipped products:

```sh
cargo run --release -p dexdec --example decompile_class -- classes.dex Lcom/example/App;
cargo run --release -p dexdec --example phase_timings -- classes.dex \
  Lcom/example/App; Lcom/example/Other;
```

## Fork maintenance

- [`FORK.md`](FORK.md) records the snapshot and repository role.
- [`PATCHES.md`](PATCHES.md) is the ordered re-vendor ledger.
- The default feature set is empty; the optional `apk` feature retains upstream-style archive
  loading for compatibility, while rasc normally supplies DEX bytes directly.

## License

Apache-2.0; see [LICENSE](LICENSE).
