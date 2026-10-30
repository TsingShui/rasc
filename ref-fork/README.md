# Archived reference forks

`rasc/` contains the existing rasc implementation and its in-tree dexdec,
rusty-dex and patched axmldecoder dependencies. It is retained for source review
and focused reuse, not as an actively developed/released second product on the
Diana branch.

- Root Diana Cargo workspace explicitly excludes `ref-fork/rasc`.
- The reference has its own Cargo workspace, lockfile, source/tests/skill/docs.
- No active root CI builds/tests/formats/lints it; its release workflow is nested
  under `rasc/.github/workflows/`, so GitHub does not discover or trigger it.
- rasc release scripts and MCP code-mode helper are archived with the project.
  They are historical tooling, not current root commands.
- Reference versions stay at 1.0.1; no engine behavior was changed by relocation.
- Existing crate license files stay in place; root NOTICE was removed as requested.
- Do not copy the tree into production, depend on its crates, or use its output as
  E2E truth. Review sources, rights, compatibility and semantics before local reuse.

The source baseline is the public rasc commit
`ef8a4c92c6b89e8afed9ccbc1552feae67b6982d`; the retained working tree is not
a new upstream release. Fixed upstream paths in Diana SOURCE_MAP remain relative
to that commit; in this checkout prefix those paths with `ref-fork/rasc/`.
See [Diana structure](../docs/diana/STRUCTURE.md) and
[source evidence](../docs/diana/SOURCE_MAP.md).
