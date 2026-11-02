# H08 CLIProductRC progress: application seams

Baseline: c60af3d. Contracts: DESIGN §6 and TASKS H08. This record tracks the
application side of the required JSON set; `ALGORITHMS.md` is unchanged.

## What the product now delivers through the application path

New development callers under `apps/diana/examples/` drive the application modules
for the E2E adapter:

- `app_query` exercises `analysis::query` (`defined-methods`) and `analysis::xref`
  (`direct-method-xref`) and returns original member identities and site offsets.
- `app_decompile` exercises `analysis::decompile` with the same engine chain the CLI
  uses, returning the class report, member sources, diagnostics, spans and the
  selected image's header offset.

The adapter now runs those seams instead of reporting them unimplemented:

| Seam | Evidence produced |
|---|---|
| `query-defined-symbols` (D066) | the two real definitions of the fixture, with original `method_id_idx` |
| `query-reference-sites` (D067) | both invoke sites in one method with their original PCs |
| `application-default-text` (D085) | the default text/report of the class and of the failing method |
| `application-original-view` (D065) | the class of each 041 logical member; the tracked Driver returns 11 and 22 |

Locators are built from the certified binding, including the logical ordinal, so a
041 input selects `e0:l0` or `e0:l1` instead of an ambiguous short name. A method
context uses the tracked source, or the certified D065 mirror with its namespace
rewritten, and never an engine-generated fragment.

## Current ledger

`python3 tools/e2e.py run` with this working tree selects 86 cases and 181 seams.
Result: 164 seams and 83 whole cases accepted. The only remaining seam is
`extract-entry`: D068 (5 invocations), D069 (4) and D070 (8), 17 in total. Earlier
records listed `application-original-view`, `application-default-text`,
`query-defined-symbols` and `query-reference-sites` as unimplemented; they now pass
against the existing independent expectations.

Gates: formatting, strict workspace Clippy and the locked release build pass;
`cargo nextest run --workspace --all-features --locked` passes 234 tests with zero
skips. The E2E adapter change adds no product test and no CLI black-box check.

## Remaining work

The 17 `extract-entry` invocations need an adapter path that builds the archive,
target and observer fixtures for D068–D070, drives `analysis::extract`, and records
the file-safety observations the comparator already asserts. H08 stays open until
those seams are accepted and the retirement review below is recorded.

## Retirement review (pending completion of the seams)

- `Cargo.toml` excludes `ref-fork/rasc` and lists only `apps/diana` and
  `crates/diana-decomp`; both packages keep `publish = false` and the `0.0.0`
  scaffold marker, and no active target or script references rasc, dexdec or MCP.
- The archived reference stays complete and outside every root gate.
