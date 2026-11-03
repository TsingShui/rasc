# Diana application targets

`apps/` groups product entry points, separate from the reusable engine under
`crates/diana-decomp/`. It remains useful as Diana grows beyond a single CLI.

| Directory | Target | Current state |
|---|---|---|
| [diana/](diana/README.md) | Native CLI | Native analysis and decompilation CLI |
| [diana-gui/](diana-gui/README.md) | Future GUI | Documentation placeholder only |
| [diana-web-ui/](diana-web-ui/README.md) | Future web interface (WASM-based or otherwise undecided) | Documentation placeholder only |

The GUI and web-UI directories have no manifest, source, dependency, build target
or CI job. They are not Cargo workspace members. No GUI framework, browser frontend,
WASM binding or engine portability contract is selected by creating them.
Implement nothing there until explicitly requested and separately designed.

These directory reservations do not mark any capability complete. The archived
rasc implementation was removed from the tree at commit 5cb156c and stays outside
active builds and tests; only git history retains it.

See the [specification chapters](../docs/diana/spec/00-overview.md).
