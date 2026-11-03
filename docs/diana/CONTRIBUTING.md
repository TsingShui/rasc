# Diana contribution details

This document supplements the root `AGENTS.md` with detailed rules for the
relevant work. Package-local instructions refine their respective scopes.

## Project layout and reference isolation

- `apps/diana/`: CLI, input, snapshot, ZIP, query and output.
- `apps/diana-gui/`, `apps/diana-web-ui/`: README-only future target placeholders.
  No manifests, implementation, dependencies or CI; not workspace members. Do
  not initialize/design their frameworks, browser UI or WASM bindings until
  separately requested.
- `crates/diana-decomp/`: independent native engine and public library.
- `docs/diana/`: design, algorithms, sources and contribution rules.
- `tools/audit/`: sensitive-input audit tooling.
- `tools/fetch_references.py`: explicit reference downloads to ignored
  `.cache/references/`; no checksum checks, reference builds/tests or CI hooks.
- The archived rasc tree (rasc source, dexdec, rusty-dex, patched vendor, old
  tests, skill/docs and release tooling) was removed from the working tree at
  commit 5cb156c. It remains reachable in git history only: do not restore it,
  build, test, lint, format or release it, and do not add a Diana dependency on
  reference crates.

See [00-overview](spec/00-overview.md) for actual paths. Older root-product/src paths in
feature tasks mean the Diana application root `apps/diana/`; historical reference
paths are resolved against their upstream URL and pinned commit in SOURCE_MAP, not
by a local copy.
Do not copy the reference tree wholesale or create speculative common modules.
Use source ideas only through explicit review and focused extraction/reimplementation.

## Development workflow

Diana is a decompiler: default class/method retrieval MUST expose available
code, not require an entirely verified Java unit before showing anything.
Keep partial Java, original DEX debug information and unavailable reasons explicit; a local failure
MUST NOT discard other members. Text/JSON are presentations of the same result,
not capability gates. Preserve hard errors and distinguish observed readable output
from independently established semantic behavior. Product acceptance evaluates
readability, original facts and uncertainty, not a verified-Java status.

Prioritize core APK/DEX analysis, decompilation and required safety. Keep non-core
tooling, registry/trace infrastructure and their documentation minimal; do not
expand them into frameworks or lengthy designs. The shared analysis chain MUST retain available readable output with concrete
recovery diagnostics. Contributors MUST NOT build a whole-result Java-legality
qualification path. Fallback retains original facts when reconstruction is unavailable,
not a separate recovery subsystem or retry pipeline.

Every authored Diana document, including this file and the package README/AGENTS
files, MUST follow `docs/diana/WRITING.md`. Read it before editing documentation.
WRITING owns normative keyword strength, sentence roles, terminology, numbers,
document ownership and the rules against duplicated narration and speculative
roadmaps; those rules are not restated here. A style edit MUST NOT change
requirement strength or technical meaning.

Technical documents specify supported inputs, algorithms, invariants and failure
behavior. Current status belongs in README; historical plans and execution records
are available in git history. Core tests own their fixtures and independent
expectations. Do not maintain a separate task or batch-record tree.

## Architecture guardrails

Before changing an engine capability, contributors MUST read
[phases](phases.md) and the owning stage chapter. Contributors MUST identify the owning phase, actual producer/consumer, invariant
and existing independent evidence before implementation. Before
implementation, contributors MUST also identify the consumed facts, their full or
partial guarantees, and the classified failure/resource/cancellation behavior.
When an implementation would contradict those contracts, contributors MUST stop the
affected change and request a decision rather than weaken the design or add a
bypass.

- Contributors MUST extend the shared ordinary-method chain: P2 CFG/effects →
  P3 SSA → P4 declaration constraints/evidence → P4 types/use conversions →
  P5 storage/copies → P6 regions → P7 Java recovery. P8 MUST render existing
  products and original facts without further analysis.
- Contributors MUST follow the stage-authority rules in [phases](phases.md)
  when exposing stage entries, implementation modules and product views. A
  read-only product or a passing behavior test does not establish phase ownership.
- Before widening a phase's input access, module exports or product-construction
  authority, contributors MUST obtain operator approval. The same approval is
  required before adding an analysis-construction entry callable from outside its
  owning phase or changing the phase-ownership or scheduling contract. Contributors MUST present
  the producer/consumer need and the proposed interface change before editing it.
- Contributors MUST NOT work around a missing producer or an access restriction
  by widening visibility, forwarding internal implementations through another
  module, adding an alternate analysis route or weakening a sealed product.
  Contributors MUST stop the affected change and request a decision when the
  capability cannot be implemented within the existing contract.
- Contributors MUST put calls, fields, arrays, casts, allocation and fill into
  common operation, definition/use and effect products. Initialization and
  FreshArray proofs MUST consume those products in their owning phases, not
  maintain another complete method register/alias tracker.
- Contributors MUST NOT select recovery paths by hard-coded class/test names or
  whole-method instruction templates. Specification-defined opcode/format checks, local
  proofs, P6 region patterns and Generated-member rules remain in their owning
  phases.
- Contributors MUST NOT organize `engine.rs` as successive whole-method
  builders/verifiers that emit on the first match. Ordinary methods MUST use one
  analysis chain. P7 MUST retain available readable reconstruction from
  shared products; the classified read-only raw exit MUST NOT retry analysis or
  reconstruct values.
- Contributors MUST keep P4 constraints/binding obligations distinct from P4
  typed uses and final P7 source binding. P6 MUST read P5 storage before Java
  lowering, not derive its graph from a P7 AST. P8 MUST NOT select types, bindings
  or recovery semantics.
- Natural structure and dispatcher MUST consume the same checked operations,
  storage and CopyPlans. Contributors MUST NOT give either representation a
  separate capability implementation.
- Reviewers MUST reject parallel recovery implementations with their own
  complete-method decoding, SSA/type analysis or register/alias tracking.
  Operation-specific helpers and phase-owned proofs do not authorize another
  method entry path. Independent replay remains a verifier obligation.
- A migration MUST preserve available members, original identities, classified
  failures, raw completeness, effect order and resource/cancellation behavior.
  Contributors MUST remove replaced paths before reporting the migration
  complete. Passing behavior cases alone does not establish architecture
  compliance.

## Preparation and acceptance gates

Structure-only work never substitutes for input/reference certification or
independent comparison evidence. Contributors MUST check current capability and acceptance against
active code and recorded evidence, not infer them from scaffold version markers
or historical completion labels.

Tests cross public libraries/modules for complex semantic, raw-integrity, malformed
input and resource/file-safety behavior. Do not add CLI black-box/help/version/
argument/column/getter/forwarding tests or empty tests to manufacture a pass.
Contributors MUST retain independent expectations rather than refresh truth from
engine output. A test pass does not establish real-application coverage or authorize
an implementation change. Existing legacy tests remain in git history.

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
safety through public interfaces. Test tooling MAY support those core cases,
but contributors MUST NOT test the tooling itself. Basic
CLI/help/getter/forwarding tests remain excluded as described above.

Contributors MUST NOT introduce algorithmically complex test or validation code
for a capability change. Reuse existing fixtures, Drivers and independent
expectations; prefer small fixed counterexamples over new interpreters, solvers,
Cartesian-product generators or comparison frameworks.

Before adding a core test, contributors MUST identify its invariant or regression
and inspect existing coverage through the relevant public interface. Contributors
SHOULD extend an existing test with table-driven inputs when the inputs exercise
the same contract and failure path. A separate test requires a distinct semantic
obligation, failure path or regression. Contributors MUST NOT duplicate generic
budget, cancellation, reference-bound or serialization checks unless a different
producer or control path requires independent coverage. Test consolidation MUST
preserve unique counterexamples, independent expectations and classified failures;
reducing the number of test functions alone does not establish deduplication.

Regression review MUST check affected original member identities, Java
availability and classified failures, not only whole-case totals. Contributors
MUST distinguish unverified drafts from the accepted baseline and obtain operator
approval before withdrawing or committing unrelated uncommitted work.

Contributors SHOULD run relevant existing core cases for coherent changes and
the full required set at integration/final-acceptance boundaries. Cross-cutting
changes or observed regressions can require another full run; editing another
file alone is not a reason to repeat it. These scheduling choices MUST NOT turn
unexecuted required evidence into a pass.

Reference downloads do not require checksum verification. Keep source URLs and
fixed revisions for attribution/lookup, preserve local changes, and never build
or test reference projects as part of downloading. Keep PDF-origin papers/slides
as same-name UTF-8 TXT only: reuse existing text, otherwise convert with Poppler
(and OCR when needed), and do not retain PDFs after successful conversion. Keep
the original public URLs for online consultation.

## File verification

Diana's download, preparation and cache tooling MUST NOT perform SHA-256
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
  remain separate as defined by [00-overview](spec/00-overview.md). Independent declaration/code results
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
and follow [the engine instructions](../../crates/diana-decomp/AGENTS.md) in addition
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
workflow is active. The old rasc workflow/scripts were removed with the archived
tree and exist only in git history; do not reactivate them or couple Diana versions
to rasc. Future release policy requires explicit approval.
