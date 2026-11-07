# Diana

Native Rust APK/DEX decompiler, designed to replace rasc.
The engine currently implements P0 → method reader → P1 → P2 → P3 → P4 → P5 → P6
planning → P7 independently owned method expressions → P8 class assembly and
scope-aware type-name plans → P9 text/JSON with UTF-8 source spans.
The [CLI](apps/diana/README.md) delivers these same P9 products for class and
standalone-method retrieval, as text or JSON schema 3. Readable output with
original facts and uncertainty remains the goal, not Java compilability.
Build and lint checks do not establish real-application coverage.

## Active project

```text
apps/diana/             # Diana CLI/application package
apps/diana-gui/         # Native GPUI Kit UI demo; no Engine integration
apps/diana-web-ui/      # Future web interface: documentation placeholder only
crates/diana-decomp/    # Independent native engine library
docs/diana/            # Core design, algorithms and contribution rules
# The archived rasc tree was removed at commit 5cb156c; it remains in git history only.
```

The root is a virtual Cargo workspace containing **only Diana packages**. The
rasc implementation and its crates/tests/release tooling remain in an independent
archived reference tree removed at commit 5cb156c; it is not a production dependency
and are not built, tested or released by this branch’s CI or root Cargo commands.

[Application targets](apps/README.md) share the `apps/` layout. The native GUI
is a GPUI Kit UI demo with authored display content; run it with
`cargo run -p diana-gui --locked`. It is a workspace member but not a default
member. The web UI remains a documentation placeholder with no selected framework.

## Development

Start with the [core documentation](docs/diana/README.md). Historical plans and
batch records are available in git history.
Run from the repository root (Rust 1.93 or newer):

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
```

Use `-p diana` or `-p diana-decomp` to select one package. Both share the root
`Cargo.lock`; the archived reference retains its own lockfile. The active
[Diana CI](.github/workflows/diana.yml) checks format, lint and release builds
without publishing. Test cases and fixtures have been removed by operator
decision; [AGENTS.md](AGENTS.md) prohibits adding tests. These checks do not
establish semantic correctness or real-application coverage.


## Local reference material

Use `python3 tools/fetch_references.py --list` or `--only <id> ...` to fetch
public material into ignored `.cache/references/`. Papers/slides are TXT-only;
existing text is reused. No reference checksum, builds/tests or production
dependencies. See [sources and download policy](docs/diana/SOURCE_MAP.md).

## Design

The decompiler contract exposes recovered code by default. A failed method is
shown with its available fragments, original DEX facts and explicit gaps alongside
other recovered methods. A local failure does not suppress the whole class.
Class assembly records presentation coverage, not compilability. Hard failures
remain separate from readable availability.

- [Documentation entry](docs/diana/README.md)
- [Approved architecture](docs/diana/PIPELINE_DESIGN.md)
- [Product boundary](docs/diana/spec/00-overview.md)
- [Sources and reproducibility](docs/diana/SOURCE_MAP.md)
- Archived reference scope: removed from the tree at commit 5cb156c; see
  [SOURCE_MAP](docs/diana/SOURCE_MAP.md) for the pinned upstream anchors


## License

Apache-2.0; see [LICENSE](LICENSE). Existing reference crates retain their license
files. There is no root NOTICE or additional license bundle.
