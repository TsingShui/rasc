# dexdec

`dexdec` is the DEX to Java decompiler behind `rasc getclass`. It is a library, not a
separately shipped command.

## Entry point

rasc passes one DEX entry and a class descriptor:

```rust
use dexdec::{DecompileOptions, Decompiler};

let mut decompiler =
    Decompiler::from_bytes(dex_bytes)?.with_options(DecompileOptions::default());
let unit = decompiler.class("Lcom/example/App;".to_owned())?;
println!("{}", unit.source);
# Ok::<(), dexdec::DecompileError>(())
```

The seam is the DEX entry bytes. dexdec owns its frontend, CFG/SSA, semantic IR, value
recovery and Java printer; rasc does not adapt its own scanner model into that pipeline.
The tree is Java-only and single-class: there is no Kotlin backend, archive loader or
platform-symbol database.

## Local example

```sh
cargo run --release -p dexdec --example decompile_class -- classes.dex Lcom/example/App;
```

## License

Apache-2.0; derived from [asLody/dexdec](https://github.com/asLody/dexdec). See
[`LICENSE`](LICENSE).
