# Working on the Diana engine

The root `AGENTS.md` and `docs/diana/CONTRIBUTING.md` apply. Read [phases](../../docs/diana/phases.md) and the owning stage chapter before
changing an engine capability.

- The native library borrows DEX bytes and caller-supplied declaration facts. It
  does not own filesystem, ZIP, CLI, network, process or MCP operations.
- Keep sealed products and one shared ordinary-method chain. Migrate stage
  ownership only to the approved target in the root AGENTS.md; do not widen input
  access, exports or construction authority beyond its product contracts. Preserve
  raw identities, classified failures, available members and effect order.
- Use `thiserror` major version 2 for typed errors and `rayon` major version 1
  for parallel compute scheduling. Obtain operator approval before introducing
  alternatives. Preserve concrete public errors and allocation
  failure handling.
- Remain independent of rasc, dexdec, rusty-dex, Kuna and radguard code. Consult
  `docs/diana/SOURCE_MAP.md` before reviewing or reusing reference material.
- Contributors MUST NOT develop, add or restore tests. Preserve production input
  and necessary construction errors. Existing independent correspondence
  verifiers compile only under `cfg(test)` and MUST NOT run on ordinary Engine
  requests.
- Engine development verification uses the `dbg_shell` example:
  `cargo run -p diana-decomp --example dbg_shell --locked -- --dex <classes.dex> --exec '<json>'`.
  It is the single P0–P9 inspection entry and accepts a bare DEX. `class` is the
  `class_def` index of that image; `method` is the engine-wide `MethodId`, not an
  index inside the class. Do not add phase-specific debug renderers, binaries or
  examples.
- Build from the repository root with `cargo build --locked -p diana-decomp`.
