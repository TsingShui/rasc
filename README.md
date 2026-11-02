# Diana

Native Rust APK/DEX decompiler, designed to replace rasc.
**Current state: checked DEX 035 input, instruction structure, original debug
snapshots and request/method resource control are implemented.** B01–B04
preparation is accepted on macOS arm64; Engine integration and analysis commands
remain pending. [TASKS](docs/diana/TASKS.md) owns acceptance evidence.

## Active project

```text
apps/diana/             # Diana CLI/application package
apps/diana-gui/         # Future GUI: documentation placeholder only
apps/diana-web-ui/      # Future web interface: documentation placeholder only
crates/diana-decomp/    # Independent native engine library
docs/diana/            # Core design, algorithms, tasks and E2E specifications
# The archived rasc tree was removed at commit 5cb156c; it remains in git history only.
```

The root is a virtual Cargo workspace containing **only Diana packages**. The
rasc implementation and its crates/tests/release tooling remain in an independent
archived reference tree removed at commit 5cb156c; it is not a production dependency
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
cargo nextest run --workspace --all-features --locked
```

Use `-p diana` or `-p diana-decomp` to select one package. Both share the root
`Cargo.lock`; the archived reference retains its own lockfile. The active
[Diana CI](.github/workflows/diana.yml) checks only scaffold format/lint/build,
without publishing or pretending a zero-test run establishes functionality.
Core tests run through public library interfaces with cargo-nextest. Their input,
structure, debug and resource evidence is distinct from fixed Engine E2E acceptance.

The current `diana` entry point reports “not implemented” to stderr and returns
failure. It does not delegate to rasc or dexdec. Do not install or release the
scaffold as a usable decompiler.

## Local reference material

Use `python3 tools/fetch_references.py --list` or `--only <id> ...` to fetch
public material into ignored `.cache/references/`. Papers/slides are TXT-only;
existing text is reused. No reference checksum, builds/tests or production
dependencies. See [sources and download policy](docs/diana/SOURCE_MAP.md).

## Design and test preparation

The decompiler contract exposes recovered code by default. A failed method is
shown as original DEX debug information or an explicit unavailable result alongside other recovered
methods; Mixed is not a gate that suppresses the whole class. Complete Java-unit
qualification and hard failures remain separate from readable output.

- [Documentation entry](docs/diana/README.md)
- [Product / CLI / engine design](docs/diana/DESIGN.md)
- [Implementation algorithms](docs/diana/ALGORITHMS.md)
- [Tasks, preparation and acceptance](docs/diana/TASKS.md)
- [Revisioned case specifications and independent expectations](docs/diana/e2e_cases.json)
- [Sources and reproducibility](docs/diana/SOURCE_MAP.md)
- Archived reference scope: removed from the tree at commit 5cb156c; see
  [SOURCE_MAP](docs/diana/SOURCE_MAP.md) for the pinned upstream anchors

The requested layout work does not mark H01/T01 or B01–B04 complete. Independently
certify fixtures, expected behavior and the comparator before production feature
work. MCP is deferred; its old implementation is preserved only in the reference.

## License

Apache-2.0; see [LICENSE](LICENSE). Existing reference crates retain their license
files. There is no root NOTICE or additional license bundle.
