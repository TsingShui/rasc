# rusty-dex

DEX parser used by the in-tree `dexdec` decompiler.

It parses the file format and exposes the class, method, field, string and instruction
tables; control-flow recovery and source generation live in `dexdec`.

## `apk` feature

Off by default. It adds the zip-based archive reader; rasc and dexdec hand over DEX bytes
instead, so the default build avoids native compression backends.

## License

Apache-2.0; derived from [rusty-rs/rusty-dex](https://github.com/rusty-rs/rusty-dex) 0.2.0
plus the extensions `asLody/dexdec` carries. See [`LICENSE`](LICENSE).
