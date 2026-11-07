# Diana application targets

`apps/` groups product entry points, separate from the reusable engine under
`crates/diana-decomp/`. It remains useful as Diana grows beyond a single CLI.

| Directory | Target | Current state |
|---|---|---|
| [diana/](diana/README.md) | Native CLI | Class/method retrieval delivers Engine/P9 text and JSON; input/query/extraction commands remain native. |
| [diana-gui/](diana-gui/README.md) | Native GUI | GPUI Kit UI demo; authored content, no Engine integration |
| [diana-web-ui/](diana-web-ui/README.md) | Future web interface (WASM-based or otherwise undecided) | Documentation placeholder only |

The GUI is a Cargo workspace member selected explicitly with `-p diana-gui`.
The web-UI directory remains a README-only placeholder, outside the workspace;
its browser framework, WASM bindings and Engine portability are not selected.
Do not initialize the web target without a separate user request.

These directory reservations do not mark any capability complete. The archived
rasc implementation was removed from the tree at commit 5cb156c and stays outside
active builds and tests; only git history retains it.

See the [documentation entry](../docs/diana/README.md) and
[approved pipeline architecture](../docs/diana/PIPELINE_DESIGN.md).
