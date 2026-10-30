# Contributing to Diana

This branch develops **Diana**, a native Rust APK/DEX analysis product. The root
Cargo workspace contains only `apps/diana` and `crates/diana-decomp`. This file
is the repository-wide guide; package-local AGENTS.md files refine its scope.

## Project layout and reference isolation

- `apps/diana/`: CLI/application scaffold; future input/snapshot/ZIP/query/output.
- `apps/diana-gui/`, `apps/diana-web-ui/`: README-only future target placeholders.
  No manifests, implementation, dependencies or CI; not workspace members. Do
  not initialize/design their frameworks, browser UI or WASM bindings until
  separately requested.
- `crates/diana-decomp/`: independent native engine scaffold.
- `docs/diana/`: core design/algorithms/tasks, sources and 70 fixed E2E cases.
- `tools/audit/`: sensitive-input audit tooling.
- `tools/fetch_references.py`: explicit reference downloads to ignored
  `.cache/references/`; no checksum checks, reference builds/tests or CI hooks.
- `tests/e2e/`: planned shared self-made sources/inputs/oracles; not created yet.
- `ref-fork/rasc/`: archived rasc source, dexdec, rusty-dex, patched vendor,
  old tests, skill/docs and release tooling. Separate workspace/lockfile, excluded
  from every root Cargo gate. Do not build, test, lint, format or release it
  automatically, and do not add a Diana dependency on reference crates.

See `docs/diana/DESIGN.md` §1 for actual paths. Older root-product/src paths in
feature tasks mean the Diana application root `apps/diana/`; fixed historical
reference paths map under `ref-fork/rasc/` without changing their upstream anchors.
Do not copy the reference tree wholesale or create speculative common modules.
Use source ideas only through explicit review and focused extraction/reimplementation.

## Development workflow

Prioritize core APK/DEX analysis, decompilation and required safety. Keep non-core
tooling, registry/trace infrastructure and their documentation minimal; do not
expand them into frameworks or lengthy designs. Fallback is a simple classified
error/raw-output path, not a separate recovery subsystem or retry pipeline.

Maintain only docs/diana/{README,DESIGN,ALGORITHMS,TASKS,SOURCE_MAP}.md plus the
fixed e2e_cases.json. REPRODUCIBILITY.md is only a redirect for that frozen JSON,
not another maintained document. Do not restore the HTML board/generator or split planning
into more documents. Read `docs/diana/TASKS.md` before starting a batch; it owns
preparation, the small-batch loop, stop/done criteria and the minimal record. Create
`docs/diana/batches/<id>.md` only when an actual batch starts; do not duplicate
expected values or maintain another hand-edited progress counter.

The current baseline keeps the full B01–B04 gate below. Small implementation
slices are not a per-batch preparation exemption. DESIGN and ALGORITHMS select
the implementation strategy; TASKS is the single workflow/acceptance source.
T/H dependencies require the named producer artifact to be ready; final task
completion still requires its fixed acceptance evidence, never empty scaffolds.

## Current state and test-first gate

Only package scaffolds exist. No Engine/CLI commands/phase implementation or
executable E2E suite exists. User-authorized structure-first work does not mark
H01/T01 or B01–B04 complete. Complete `docs/diana/TASKS.md` B01–B04
(independent specifications, tool/fixture lock, input/reference certification,
comparator falsification) before production feature work.

Tests cross public libraries/modules for complex semantic, raw-integrity, malformed
input and resource/file-safety behavior. Do not add CLI black-box/help/version/
argument/column/getter/forwarding tests or empty tests to manufacture a pass.
Frozen required cases cannot be dropped, changed to Smali success or refreshed
from engine actual output. True oracle errors require independent evidence/review.
Existing legacy tests remain in the reference but are not Diana acceptance.

## Development setup

Use Rust 1.93 or newer, as declared by the active package manifests. Build from
the repository root. Root `Cargo.lock` covers only Diana; the reference lockfile
is independent and must not be updated by active work.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
# Applicable after real Diana tests exist; currently zero tests is not acceptance
cargo test --workspace --all-features --locked
```

Bare Cargo commands now select Diana only. Do not run old rasc/dexdec tests or
release preparation to validate this branch. Missing fixture/tools, skipped tests
or an empty selector are not passes. Real APK/ART validation requires explicit
opt-in and authorized reproducible inputs; no private APKs are committed.

## Test scope: core project code only

Do not write tests for ANY non-core project code unless the user explicitly
authorizes that specific testing work. This applies repository-wide, not just
to reference downloads. Non-core code includes development/maintenance scripts,
downloaders, documentation/board generators, build/release helpers and other
auxiliary tooling. A request to implement or improve such code is not permission
to add tests.

Do not add unit, integration or CLI tests, test fixtures/harnesses, or CI test
jobs for non-core code. Do not bypass this rule by calling them self-tests,
validation scripts or automated smoke checks. Inspect the code and, when useful,
manually exercise the requested operation; report limitations/failures honestly.
Existing plans mentioning auxiliary-tool or comparator/harness self-tests do not
supply authorization for writing those tests; ask before implementing them.

The previously agreed tests of Diana core behavior remain in scope: native
DEX/decompiler semantics, input/query/extraction correctness and resource/file
safety through public interfaces. Test tooling may support those core E2E cases,
but testing the tooling itself requires separate explicit authorization. Basic
CLI/help/getter/forwarding tests remain excluded as described above.

Reference downloads do not require checksum verification. Keep source URLs and
fixed revisions for attribution/lookup, preserve local changes, and never build
or test reference projects as part of downloading. Keep PDF-origin papers/slides
as same-name UTF-8 TXT only: reuse existing text, otherwise convert with Poppler
(and OCR when needed), and do not retain PDFs after successful conversion. Keep
the original public URLs for online consultation. This does not change the
separate core E2E fixture/toolchain integrity requirements.

## Engineering constraints

- Treat APK/DEX input as untrusted. Check lengths/offsets/counts before slicing or
  allocation; malformed input must not panic/hang or allocate without bounds.
- Production stays native Rust without external runtime/process dependencies.
  Active crates forbid unsafe code. File/ZIP ownership belongs to the application;
  the engine will borrow input bytes and declaration facts.
- Preserve raw identity and program instruction/argument/effect/catch/lock/
  initialization order. Independent-result order, generated source or synthetic
  names are not stable; no output sorting/reordering buffer or pagination.
- Keep cancellation, backpressure and safety budgets. Exceeding a budget fails
  explicitly, not as silent successful truncation.
- Source semantics beat beauty; Java-unrepresentable methods use honest raw Smali
  or Unavailable according to the design, never fake successful source.
- Measure performance on release builds and authorized representative inputs.
- Keep changes focused. Read design/source/fork notes before reuse. Preserve
  reference byte content except explicitly reviewed path/provenance edits.
- MCP is deferred; old MCP is reference material, not a current interface to migrate.

## Corpus data and sensitive inputs

Do not commit private/unauthorized APKs, real-corpus identifiers or identity-revealing
mappings. Keep local corpus/cache under untracked `.cache/`. Consult
`tools/audit/desensitize_words.txt` and run
`python3 tools/audit/desensitize_check.py --range HEAD`. Relocation must not leak
previously ignored artifacts or reintroduce sensitive words from legacy notes.
Prefer small self-made fixtures with documented source/license/recipe.

## Contribution and releases

Check existing design first; include independent semantic tests for core behavior
changes within the agreed test scope. Do not write tests for non-core code without
explicit user authorization. Run applicable active-only gates and report blocked
or not-run evidence.
Use Conventional Commit messages. Do not commit, push, publish or create tags
unless the user asks. Root NOTICE is removed as requested; retain existing crate
license files without adding a license bundle.

Diana packages remain `publish=false`, version `0.0.0` scaffold markers. No release
workflow is active. The old rasc workflow/scripts are archived under ref-fork,
not triggered by GitHub or part of Diana CI. Do not reactivate them or couple
Diana versions to rasc. Future release policy requires explicit approval.
