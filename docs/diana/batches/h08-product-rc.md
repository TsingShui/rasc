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

`python3 tools/e2e.py run` selects the full required set from the JSON: 86 cases,
177 invocations, 181 seams. Result: **181/181 seams and 86/86 whole cases
accepted**, with no failing seam and no member regression.

The last seams to land were the 17 `extract-entry` invocations. The adapter now
builds the D068–D070 archive, target and observer fixtures, drives
`analysis::extract` through a development caller, and records the observations the
comparator asserts: exact payload bytes for stored/deflated/duplicate records, the
ambiguous duplicate, CRC/budget/compression/encryption rejections, existing target
without force, symlink, input path, input hardlink and directory targets, the
traversal-shaped entry name with an explicit safe output, two concurrent publishers
where exactly one wins with `target_exists` for the other, and the atomic
replacement sequence that only observes the old and the complete new payload.

Gates: formatting, strict workspace Clippy and the locked release build pass;
`cargo nextest run --workspace --all-features --locked` passes 234 tests with zero
skips. The E2E adapter change adds no product test and no CLI black-box check.

## Retirement review

Reviewed by reading the workspace manifests, the source tree and the CI file:

- `Cargo.toml` lists only `apps/diana` and `crates/diana-decomp` as members and
  default members. The archived rasc tree was removed from the working tree after
  this review and survives only in git history.
- Both packages keep `publish = false` and the `0.0.0` scaffold marker; no tag,
  release workflow or publication step runs for this branch.
- No active target, module or test references the archived tree, dexdec, rusty-dex
  or the old MCP surface; those names appear only in documentation statements about
  independence. The earlier `skill`/MCP command was not restored, and no adapter for
  MCP was designed.
- `.github/workflows/diana.yml` runs the Diana-only gates.
- The archived tree is outside every root gate and now exists only in git history;
  nothing in it was rewritten by this branch before removal.

## Remaining obligations

The Windows binary target, ART execution and representative-input performance
measurement are not run and are not claimed. MCP stays deferred.
