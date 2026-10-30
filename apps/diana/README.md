# Diana CLI — development scaffold

Diana is the CLI package in the active Diana-only Cargo workspace. Its target
binary is `diana` (`diana.exe` on Windows). **It is not usable for analysis yet:**
the current entry point reports that it is a scaffold to stderr and exits with
failure. It never delegates to rasc or dexdec.

From the repository root:

```sh
cargo build --locked -p diana
cargo clippy --locked -p diana --all-targets -- -D warnings
```

- This package will own CLI dispatch, input/snapshot/ZIP handling, lightweight
  queries and result adaptation.
- [diana-decomp](../../crates/diana-decomp/README.md) will own native decompilation
  and take borrowed DEX bytes, not paths or archives.
- Old rasc `src/`, `tests/`, `skill/`, dexdec/rusty-dex/vendor and release
  tooling are archived under `ref-fork/rasc/`, excluded from all root gates.
  Do not copy its whole tree, build/test it automatically or depend on its crates.
- Diana product interface tests will live under `apps/diana/tests/`; shared E2E
  sources/oracles/inputs belong to root `tests/e2e/`, not a second fixture set.

See [product / workspace design](../../docs/diana/DESIGN.md) and
[preparation / tasks](../../docs/diana/TASKS.md). The user requested package
structure first; B01–B04 still precede production behavior and engine work.
The `0.0.0` version is an unpublished scaffold marker, not a release.
