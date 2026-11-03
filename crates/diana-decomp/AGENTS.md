# Working on the Diana engine

The root `AGENTS.md` and `docs/diana/CONTRIBUTING.md` apply. Read DESIGN §4 and
relevant ALGORITHMS sections before changing an engine capability.

- The native library borrows DEX bytes and caller-supplied declaration facts. It
  does not own filesystem, ZIP, CLI, network, process or MCP operations.
- Keep P1–P9 ownership, sealed products and the shared ordinary-method chain;
  stage visibility and construction authority follow DESIGN §4.1. Preserve raw
  identities, classified failures, available members and effect order.
- Use `thiserror` major version 2 for typed errors and `rayon` major version 1
  for parallel compute scheduling. Obtain operator approval before introducing
  alternatives. Preserve concrete public errors and resource/cancellation control.
- Remain independent of rasc, dexdec, rusty-dex, Kuna and radguard code. Consult
  `docs/diana/SOURCE_MAP.md` before reviewing or reusing reference material.
- Build from the repository root with `cargo build --locked -p diana-decomp`.
