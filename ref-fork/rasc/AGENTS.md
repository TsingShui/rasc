# Archived rasc reference

Root AGENTS.md applies. This is not an active product on the Diana branch. The
rasc CLI/MCP/source, dexdec/rusty-dex, patched vendor, tests and release tools
are retained only for reference and explicitly excluded from the root workspace.

- Do not automatically build, test, lint, format or release this project as a
  Diana development gate, even when using cargo --workspace at the repository root.
- Do not update this independent Cargo.lock or version group for Diana changes.
- Preserve legacy source/test behavior and licenses. Changes here require an
  explicit reference-maintenance/reuse request, not a lint cleanup opportunism.
- Nested .github/workflows files are archived, not discovered by root GitHub CI.
  Do not reactivate them or run their scripts to push/release on this branch.
- Use public source/commit anchors in docs/diana/SOURCE_MAP.md for understanding,
  never reference decompiler output as a test oracle. Diana must not depend on
  these crates or forward to their CLI/engine.
- Retain existing tests as material, not part of active test counts. Missing
  private APKs or ignored benchmarks are not evidence of real-input correctness.
- Keep root sensitive-input policy during relocation; use neutral descriptions
  instead of disallowed corpus names. Do not add ignored binaries/cache material.

See ../README.md and ../../docs/diana/STRUCTURE.md for current scope. Historical
rasc docs may describe old build/install/release commands, not current workflow.
