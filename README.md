# Diana

Native Rust APK/DEX analysis and decompilation, designed to replace rasc.
**Current state: package scaffolds and E2E case specifications only.** No Engine,
analysis commands or executable E2E suite have been implemented yet.

## Active project

```text
apps/diana/             # Diana CLI/application package
apps/diana-gui/         # Future GUI: documentation placeholder only
apps/diana-web-ui/      # Future web interface: documentation placeholder only
crates/diana-decomp/    # Independent native engine library
docs/diana/            # Core design, algorithms, tasks and 70 fixed cases
ref-fork/rasc/         # Archived rasc/dexdec/rusty-dex/vendor/tests, reference only
```

The root is a virtual Cargo workspace containing **only Diana packages**. The
rasc implementation and its crates/tests/release tooling remain in an independent
reference workspace under `ref-fork/rasc/`; they are not production dependencies
and are not built, tested or released by this branch’s CI or root Cargo commands.

[Application targets](apps/README.md) share the `apps/` layout. GUI and web UI
are reserved directories only, with no manifests, implementation, dependencies
or CI jobs; they are not workspace members and no framework or WASM strategy
has been selected.

## Development

Start with the [core documentation](docs/diana/README.md). The unified
[TASKS.md](docs/diana/TASKS.md) owns preparation, bounded implementation batches
and acceptance. The full B01–B04 prerequisite remains in force.

Run from the repository root (Rust 1.93 or newer):

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
```

Use `-p diana` or `-p diana-decomp` to select one package. Both share the root
`Cargo.lock`; the archived reference retains its own lockfile. The active
[Diana CI](.github/workflows/diana.yml) checks only scaffold format/lint/build,
without publishing or pretending a zero-test run establishes functionality.
`cargo test --workspace` will only select Diana; there are currently no actual
Diana tests, and that is **not** an E2E pass.

The current `diana` entry point reports “not implemented” to stderr and returns
failure. It does not delegate to rasc or dexdec. Do not install or release the
scaffold as a usable analysis tool.

## Local reference material

Use `python3 tools/fetch_references.py --list` or `--only <id> ...` to fetch
public material into ignored `.cache/references/`. Papers/slides are TXT-only;
existing text is reused. No reference checksum, builds/tests or production
dependencies. See [sources and download policy](docs/diana/SOURCE_MAP.md).

## Design and test preparation

- [Documentation entry](docs/diana/README.md)
- [Product / CLI / engine design](docs/diana/DESIGN.md)
- [Implementation algorithms](docs/diana/ALGORITHMS.md)
- [Tasks, preparation and acceptance](docs/diana/TASKS.md)
- [70 fixed case specifications](docs/diana/e2e_cases.json)
- [Sources and reproducibility](docs/diana/SOURCE_MAP.md)
- [Archived reference scope](ref-fork/README.md)

The requested layout work does not mark H01/T01 or B01–B04 complete. Independently
certify fixtures, expected behavior and the comparator before production feature
work. MCP is deferred; its old implementation is preserved only in the reference.

## License

Apache-2.0; see [LICENSE](LICENSE). Existing reference crates retain their license
files. There is no root NOTICE or additional license bundle.
