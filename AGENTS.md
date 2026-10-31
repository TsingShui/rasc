# Contributing to Diana

This branch develops **Diana**, a native Rust APK/DEX decompiler. The root
Cargo workspace contains only `apps/diana` and `crates/diana-decomp`. This file
is the repository-wide guide; package-local AGENTS.md files refine its scope.

## Project layout and reference isolation

- `apps/diana/`: CLI/application scaffold; future input/snapshot/ZIP/query/output.
- `apps/diana-gui/`, `apps/diana-web-ui/`: README-only future target placeholders.
  No manifests, implementation, dependencies or CI; not workspace members. Do
  not initialize/design their frameworks, browser UI or WASM bindings until
  separately requested.
- `crates/diana-decomp/`: independent native engine scaffold.
- `docs/diana/`: core design/algorithms/tasks, sources and revisioned E2E specifications.
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

Diana is a decompiler: default class/method retrieval MUST expose available
code, not require an entirely verified Java unit before showing anything.
Keep partial Java, original DEX debug information and unavailable reasons explicit; a local failure
MUST NOT discard other members. Text/JSON are presentations of the same result,
not capability gates. Preserve hard errors and strict Java/JavaUnit acceptance;
a readable Mixed result is not permission to count required Java as passed.

Prioritize core APK/DEX analysis, decompilation and required safety. Keep non-core
tooling, registry/trace infrastructure and their documentation minimal; do not
expand them into frameworks or lengthy designs. Fallback is a simple classified
error/raw-output path, not a separate recovery subsystem or retry pipeline.

Every authored Diana document, including this file and the package README/AGENTS
files, MUST follow `docs/diana/WRITING.md`. Read it before editing documentation.
WRITING owns normative keyword strength, sentence roles, terminology, numbers,
document ownership and the rules against duplicated narration and speculative
roadmaps; those rules are not restated here. A style edit MUST NOT change
requirement strength or technical meaning.

Technical documents specify one implementation contract: supported/rejected inputs,
algorithms, invariants and failure behavior. Progress and execution status live in
README/TASKS, not in a technical specification.

Maintain only docs/diana/{README,DESIGN,ALGORITHMS,TASKS,SOURCE_MAP,WRITING}.md plus
e2e_cases.json, the single case/input/expected specification. Do not restore
the HTML board/generator, obsolete redirect documents or split planning
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

The engine implements request work/allocation accounting, method normal/raw
reservations, cancellation, checked DEX 035 parsing and borrowed declaration/code
range views with method-local failures. Input boundaries use typed coordinates
and IDs; the shared instruction-layout helper checks format and range facts.
Engine/CLI commands and decompilation compute phases remain unimplemented. B01–B04 preparation is accepted for the executed profile in
`docs/diana/TASKS.md` §3. The prepared runner reports the absent engine as
not-implemented, not passing. H01/T01 remain unaccepted. Keep the full B01–B04
requirements for any new or revised preparation evidence; structure-only work
never substitutes for input/reference certification or comparator falsification.

Tests cross public libraries/modules for complex semantic, raw-integrity, malformed
input and resource/file-safety behavior. Do not add CLI black-box/help/version/
argument/column/getter/forwarding tests or empty tests to manufacture a pass.
B01 freezes a reviewed revision, not an arbitrary case count or faulty format.
Correct faulty recipes/oracles with independent evidence and recorded review;
preserve problem IDs and valid semantics, append new counterexamples, and derive
case/invocation denominators from the specification. Never drop required cases,
change required Java to debug output to manufacture success or refresh truth
from engine actual output.
A specification revision is not B01 approval, fixture certification, production
implementation permission or authorization for auxiliary-tool self-tests.
Existing legacy tests remain in the reference but are not Diana acceptance.

## Development setup

Use Rust 1.93 or newer, as declared by the active package manifests. Build from
the repository root. Root `Cargo.lock` covers only Diana; the reference lockfile
is independent and MUST NOT be updated by active work.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
# Run real Diana core tests; an empty selector is not acceptance
cargo nextest run --workspace --all-features --locked
```

Contributors MUST use cargo-nextest (`cargo nextest run`) for Rust unit and
integration test execution, not `cargo test`. If cargo-nextest is unavailable,
report the test run as blocked rather than silently switching runners. Rustdoc
doctests require a separate `cargo test --doc` command; obtain operator approval
before running that exception.

Bare Cargo commands now select Diana only. Do not run old rasc/dexdec tests or
release preparation to validate this branch. Missing fixture/tools, skipped tests
or an empty selector are not passes. Real APK/ART validation requires explicit
opt-in and authorized reproducible inputs; no private APKs are committed.

## Test scope: core project code only

Contributors MUST NOT write tests for any non-core project code. This prohibition
applies repository-wide, not just to reference downloads, and has no authorization
exception. Non-core code includes development/maintenance scripts, downloaders,
documentation/board generators, build/release helpers and other auxiliary tooling.
A request to implement or improve such code does not permit tests.

Do not add unit, integration or CLI tests, test fixtures/harnesses, or CI test
jobs for non-core code. Do not bypass this rule by calling them self-tests,
validation scripts or automated smoke checks. Inspect the code and, when useful,
manually exercise the requested operation; report limitations/failures honestly.
Existing plans, task cards and historical authorizations for auxiliary-tool or
comparator/harness self-tests do not override this prohibition. Contributors MUST
NOT request an exception.

The previously agreed tests of Diana core behavior remain in scope: native
DEX/decompiler semantics, input/query/extraction correctness and resource/file
safety through public interfaces. Test tooling MAY support those core E2E cases,
but contributors MUST NOT test the tooling itself. Basic
CLI/help/getter/forwarding tests remain excluded as described above.

Before adding a core test, contributors MUST identify its invariant or regression
and inspect existing coverage through the relevant public interface. Contributors
SHOULD extend an existing test with table-driven inputs when the inputs exercise
the same contract and failure path. A separate test requires a distinct semantic
obligation, failure path or regression. Contributors MUST NOT duplicate generic
budget, cancellation, reference-bound or serialization checks unless a different
producer or control path requires independent coverage. Test consolidation MUST
preserve unique counterexamples, independent expectations and classified failures;
reducing the number of test functions alone does not establish deduplication.

Reference downloads do not require checksum verification. Keep source URLs and
fixed revisions for attribution/lookup, preserve local changes, and never build
or test reference projects as part of downloading. Keep PDF-origin papers/slides
as same-name UTF-8 TXT only: reuse existing text, otherwise convert with Poppler
(and OCR when needed), and do not retain PDFs after successful conversion. Keep
the original public URLs for online consultation. This does not change the
separate core E2E fixture/toolchain integrity requirements.

## File verification

Diana's download, preparation, cache and E2E tooling MUST NOT perform SHA-256
verification or require it for acceptance. Existing digest records are
informational provenance only, not gates. Agents MUST NOT restore those checks
or substitute another content fingerprint without explicit user authorization.
Keep explicit tool versions, public URLs, dependency locks, path/range/size
checks and independent semantic evidence. Format-defined ZIP CRC and original
DEX header fields retain their own contracts. Reuse existing evidence for
unchanged artifacts; do not add whole-JDK, dependency-tree or cache scans.

## Engineering constraints

- Treat APK/DEX input as untrusted. Check lengths/offsets/counts before slicing or
  allocation; malformed input MUST NOT panic/hang or allocate without bounds.
- Production stays native Rust without external runtime/process dependencies.
  Active crates forbid unsafe code. File/ZIP ownership belongs to the application;
  the engine will borrow input bytes and declaration facts.
- Preserve raw identity and program instruction/argument/effect/catch/lock/
  initialization order. Independent-result order, generated source or synthetic
  names are not stable; no output sorting/reordering buffer or pagination.
- Keep cancellation, backpressure, checked ranges and allocation failure handling.
  Resource quotas are opt-in and default to no numeric caps. Do not introduce
  unmeasured default budgets or speculative JVM-size rejection formulas. Enforce
  explicitly configured quotas without silent successful truncation; real format
  limits still apply.
- Source semantics beat beauty; Java-unrepresentable methods retain their reason
  and available original DEX facts/debug information, or an explicit Unavailable
  with a safe excerpt. No standard-Smali/reassembly guarantee or fake successful
  source is implied.
- Measure performance on release builds and authorized representative inputs.
- Keep changes focused. Read design/source/fork notes before reuse. Preserve
  reference byte content except explicitly reviewed path/provenance edits.
- MCP is deferred; old MCP is reference material, not a current interface to migrate.

## Communication

Assistant responses MUST state a clear decision or concrete next action.
Assistants MUST NOT narrate scope exclusions or unsolicited statements about
what the work does not accomplish. When a real blocker, safety constraint or
acceptance limitation affects the decision, assistants MUST report it accurately
and state the required next action. This communication rule does not change
implementation scope or acceptance requirements.

## Rust coding conventions

Diana favors plain, explicit, type-driven Rust. Reviewability and correctness
invariants take precedence over brevity or clever abstractions. Use the
[Rust Style Guide](https://doc.rust-lang.org/style-guide/) for formatting and
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) for public
interfaces, naming, conventional traits and documentation. Project contracts
remain authoritative.

- Contributors MUST use newtypes for confusable coordinates and identities across
  module boundaries, such as byte offsets, code-unit offsets and method IDs.
  Ordinary loop counters do not require wrappers. Raw, range-checked and
  semantically validated products MUST have distinct types where later analysis
  depends on those guarantees; only the owning validator constructs its sealed
  validated product.
- Contributors MUST model mutually exclusive states with enums rather than
  independent flags that permit contradictions. Output states and failure kinds
  remain separate as defined by DESIGN §2. Independent declaration/code results
  and ordinary boolean predicates remain valid when their combinations have
  explicit meanings.
- Core interfaces MUST propagate classified errors. Contributors MUST NOT turn
  parse failures into empty lists, zero, missing results or defaults. Application
  boundaries MAY add human-readable context while preserving the failure kind.
  A default representing an explicitly absent option is not error suppression.
- Contributors MUST use checked arithmetic and checked conversions for untrusted
  ranges and counts. A narrowing `as` conversion or production `unwrap`/`expect`
  MUST have a locally evident invariant proving it safe, independent of an
  assumption that input is valid. Tests MAY use `unwrap` for required outcomes.
- Contributors SHOULD use ordinary loops and `match` for stateful algorithms,
  control-flow analysis and multiple failure exits. Use iterator chains for
  simple transformations, not to hide mutation or error propagation.
- Contributors MUST start with concrete implementations and introduce traits or
  generic abstractions only for actual multiple implementations or a concrete
  isolation boundary. Macros SHOULD express mechanical repetition rather than
  hide control flow. The existing direct-scheduling constraints still apply.
- Contributors SHOULD borrow immutable input and copy only when ownership requires
  it. Contributors MUST NOT use pervasive `clone` or `Arc<Mutex<_>>` merely to evade
  ownership design. A small justified copy is preferable to unnecessarily complex
  lifetimes.
- Contributors MUST keep `mod.rs` focused on module declarations, re-exports and
  minimal module-local wiring. Substantial algorithms, data types and error handling
  MUST live in named implementation files rather than accumulate in `mod.rs`.
  Create files only for actual producers or consumers; do not split empty scaffolds.
- Contributors MUST keep parsing, analysis, recovery and rendering responsibilities
  separate. Parsers do not emit Java, analysis does not choose CLI exit codes,
  and writers do not repair semantics. Each boundary MUST state its input
  guarantees, output guarantees and failure behavior.
- Contributors MUST use default rustfmt formatting and the established Clippy
  baseline. Broader lint groups SHOULD be enabled selectively rather than all at
  once. Lint exceptions MUST be local and justified. Core tests MUST exercise
  meaningful invariants through public interfaces within the existing test scope.

## Code comments

Contributors MUST add comments only to explain non-obvious invariants, algorithm
choices, format constraints, units or failure behavior that the code does not
express clearly. Contributors MUST NOT add comments that restate code, narrate
edits, decorate sections or describe speculative future work. Prefer clearer
names and smaller functions over explanatory comments for avoidable complexity.

Public API documentation MUST describe the contract needed by callers, including
relevant errors, any panic preconditions, safety requirements, and resource or
cancellation behavior, without duplicating the design documents. Contributors MUST preserve comments that carry still-valid
correctness or safety reasoning and MUST update or remove comments made stale by
a code change. Comment edits MUST NOT weaken technical requirements.

## Dependency conventions

Diana production packages MUST use `serde_json` major version 1 for JSON
encoding and decoding, with `serde` major version 1 for serialization traits
and derives where needed. Contributors MUST obtain operator approval before
introducing an alternative JSON library. Add dependencies only when an actual
capability consumes them; this choice does not waive resource-control or
original-data preservation requirements.

Performance timeline instrumentation MUST use `fastrace` major/minor version
0.7 behind the opt-in `perf` Cargo feature, disabled by default. The application
owns reporter initialization and flushing; the engine MUST NOT initialize a
global reporter. Contributors MUST keep instrumentation limited to actual
compute stages and MUST NOT record input contents or sensitive identities.
Timeline spans do not replace CPU sampling, benchmarks or resource accounting.

## diana-decomp

Before working on files under `crates/diana-decomp/`, contributors MUST read
and follow [the engine instructions](crates/diana-decomp/AGENTS.md) in addition
to this file. A link does not automatically load the referenced instructions.

## Corpus data and sensitive inputs

Do not commit private/unauthorized APKs, real-corpus identifiers or identity-revealing
mappings. Keep local corpus/cache under untracked `.cache/`. Consult
`tools/audit/desensitize_words.txt` and run
`python3 tools/audit/desensitize_check.py --range HEAD`. Relocation MUST NOT leak
previously ignored artifacts or reintroduce sensitive words from legacy notes.
Prefer small self-made fixtures with documented source/license/recipe.

## Contribution and releases

Check existing design first; include independent semantic tests for core behavior
changes within the agreed test scope. Contributors MUST NOT write tests for
non-core code. Run applicable active-only gates and report blocked
or not-run evidence.
Use Conventional Commit messages. Agents MUST prefix every commit subject with
`[AUTOMATED] ` before the Conventional Commit type, for example
`[AUTOMATED] feat: decode original debug programs`. When commits are authorized,
agents MUST commit completed, verified batches promptly.
Do not commit, push, publish or create tags
unless the user asks. Root NOTICE is removed as requested; retain existing crate
license files without adding a license bundle.

Diana packages remain `publish=false`, version `0.0.0` scaffold markers. No release
workflow is active. The old rasc workflow/scripts are archived under ref-fork,
not triggered by GitHub or part of Diana CI. Do not reactivate them or couple
Diana versions to rasc. Future release policy requires explicit approval.
