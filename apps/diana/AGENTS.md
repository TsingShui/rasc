# Working on the Diana application

Repository-wide AGENTS.md applies. This directory is the Diana application root,
inside the root Diana-only virtual workspace. See ../../docs/diana/DESIGN.md §1 for the
current layout and the interpretation of older design paths.

- The current package is only a scaffold. Implement no CLI/query/input behavior
  before B01–B04 E2E preparation is independently accepted. Structure-first does
  not mark H01, T01 or E2E complete.
- Keep CLI/input/snapshot/ZIP/query/result adaptation here. Do not depend on rasc,
  dexdec or rusty-dex, copy their whole tree, or delegate to their CLI/engine.
- Use ../../crates/diana-decomp for the native engine. Reuse legacy input/ZIP
  code only through an explicitly reviewed extraction with compatibility and
  security evidence; do not create speculative shared forwarding modules.
- Add product tests through public module interfaces when behavior exists; no
  new CLI black-box/help/version/argument/getter tests or empty tests. Shared
  inputs and independent oracles belong to repository tests/e2e/.
- Production stays native Rust; no external runtime/process dependency, unsafe
  code or MCP design. Keep publish=false until an explicit release decision.
- Build from the repository root with cargo build --locked -p diana. Preserve
  reference source/version content without building, testing or releasing it.
  Root Cargo/CI select only Diana; do not publish or commit unless the user asks.
