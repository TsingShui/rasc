# rasc v1.0.2

## CLI inheritance queries

Adds the missing `findrefs extends` and `findrefs impl` commands:

```sh
rasc findrefs app.apk extends android.app.Activity -o subclasses.txt
rasc findrefs app.apk impl android.os.IInterface -o impls.txt
```

- Accept exact dotted Java names or Dalvik descriptors.
- Inspect direct superclass and declared interface relations only, not transitive ancestry.
- Preserve upstream text output: `dex | descriptor extends target` or `dex | descriptor implements target`.
- Support APK/multi-DEX archives, bare DEX, and DEX 041 logical containers.
- Support deterministic output, `-o`, thread limits, and stderr debug diagnostics.
- Validate malformed superclass indices and interface list ranges without panicking.

These two query kinds are CLI-only for now; MCP still exposes string/type/method/field queries.

## Release assets

Adds Windows x86_64 binaries alongside macOS arm64/x86_64 and Linux x86_64 musl packages. Each package includes a SHA-256 checksum.

## Validation

Formatting, workspace tests, CLI-only strict Clippy, and a local release build passed. Self-contained fixtures cover inheritance queries and malformed inputs. Real-APK tests were not run. Workspace strict Clippy remains blocked by existing warnings in `rusty-dex`.
