# diana-decomp — native engine package scaffold

This is Diana's internal Rust decompiler library, imported as `diana_decomp`.
**It has no Engine interface or decompilation implementation yet.** An empty
build or a test run with zero cases is not engine or E2E acceptance.

From the repository root:

```sh
cargo build --locked -p diana-decomp
cargo clippy --locked -p diana-decomp --all-targets -- -D warnings
```

The crate currently has no dependencies. It must remain independent of rasc,
dexdec, rusty-dex, Kuna and radguard implementations. Future inputs are borrowed
DEX bytes and declaration facts provided by `apps/diana/`; file/ZIP loading and
CLI dispatch do not belong here. Production code stays native Rust, with unsafe
code forbidden and no external runtime/process dependency.

Only create modules when an implemented capability needs them. Do not pre-create
P0–P9 directories, fake Engine results, pass registries or tests for placeholder
getters. See [interfaces / data](../../docs/diana/DESIGN.md),
[algorithms](../../docs/diana/ALGORITHMS.md) and
[preparation / tasks](../../docs/diana/TASKS.md).

The `0.0.0` version is an unpublished scaffold marker. rasc and its release
tooling are isolated under `ref-fork/rasc/`, outside the active workspace/CI. Shared test inputs/oracles will live in root
`tests/e2e/`; crate tests will only add the public-library adapter and semantic
assertions needed by actual capabilities.
