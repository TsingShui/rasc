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
MUST NOT discard independently recoverable fragments within that method or other
members. All error handling follows [failure locality](PIPELINE_DESIGN.md#9-局部失败与方法退出).
Contributors MUST identify the owning producer, affected facts, actual dependents
and retained output when adding or changing an error path. Any wider stop requires
a concrete dependency/resource reason, not only an error kind or an Engine-level
catch-and-continue patch. The [P1–P9 contract](PIPELINE_DESIGN.md#92-p1p9-统一消费局部结果)
applies to each stage's input, output and stop behavior, not only P1–P3 or outer
scheduling. Contributors MUST update the owning producer and real consumers
together; a later stage MUST NOT erase local results by reintroducing an all-or-nothing
gate. Text/JSON are presentations of the same result,
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
are available in git history. Do not maintain a separate task or batch-record tree.

## Architecture guardrails

Before changing an engine capability, contributors MUST read
[phases](phases.md) and the owning stage chapter. Contributors MUST identify the owning phase, actual producer/consumer, invariant
and existing independent evidence before implementation. Before
implementation, contributors MUST also identify the consumed facts, their full or
partial guarantees, and the classified failure/resource behavior.
For the pipeline refactoring, contributors follow the root
[refactoring authority](../../AGENTS.md#pipeline-refactoring-authority). Technical
details within the approved target have default approval; the listed prohibited
changes do not. Stop and report concrete conflicts without weakening the design
or adding a bypass.

- Contributors MUST keep one shared ordinary-method chain: P2 CFG/effects →
  P3 SSA → P4 declaration constraints/evidence → P4 types/use conversions →
  P5 storage/copies → P6 regions → P7 method expression. Under the approved
  target, P8 assembles the class and plans names; P9 renders existing products
  and original facts without further analysis.
- Contributors MUST follow the stage-authority rules in [phases](phases.md)
  when exposing stage entries, implementation modules and product views. A
  read-only product or a passing behavior test does not establish phase ownership.
- Contributors MUST document actual producer/consumer needs before implementing
  the approved product interfaces. Root refactoring authority permits technical
  detail decisions, not wider stage access, exports or product-construction
  authority. Contributors MUST NOT change ownership or scheduling beyond that
  target or treat default approval as permission for its prohibited changes.
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
  typed uses and P7 method-expression checks. P6 MUST read P5 storage before
  lowering, not derive its graph from a P7 AST. Under the approved target, P8
  plans display names without reselecting method types or recovery semantics;
  P9 only renders existing products.
- Natural regions and explicit Java-like graph transfers MUST consume the same
  operations, storage and CopyPlans. Contributors MUST NOT add a generated
  dispatcher or a separate method analysis for either representation.
- Reviewers MUST reject parallel recovery implementations with their own
  complete-method decoding, SSA/type analysis or register/alias tracking.
  Operation-specific helpers and phase-owned proofs do not authorize another
  method entry path. Independent replay remains a test-only verifier obligation.
- A migration MUST preserve available members, original identities, classified
  failures, raw completeness, effect order and resource behavior.
  Contributors MUST remove replaced paths before reporting the migration
  complete. Passing behavior cases alone does not establish architecture
  compliance.

## Preparation and acceptance gates

Contributors MUST check capabilities against active code and independent evidence,
not infer them from scaffold markers or historical completion labels. Build and
lint success does not establish semantic correctness or real-application coverage.

Run from the repository root with Rust 1.93 or newer:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
```

Bare Cargo commands select Diana only. Do not build or release archived reference
projects to validate this branch. Report failed checks and missing evidence
accurately. Real APK/ART validation requires explicit opt-in and authorized
reproducible inputs; no private APKs are committed.

## Testing policy

The root [AGENTS.md](../../AGENTS.md) owns the prohibition on unit-test development.
Existing unit and integration cases, fixtures and test-only helpers are removed
by operator decision. The root policy defines one operator-approved core
unit-test exception for P2/P3 semantic cases through public Engine interfaces.
CI runs format, lint and build checks without test jobs.
Contributors MUST NOT restore the removed cases or test-only construction paths.

Contributors MUST NOT write tests for non-core code, including maintenance
scripts, downloaders, documentation generators and build/release helpers.
Contributors MUST NOT bypass this prohibition with self-tests or automated smoke
checks, and MUST NOT request an exception.

Production retains safe parsing, range/arithmetic checks, fallible allocation and
necessary construction error handling. Independent output audits, symbolic replay,
duplicate graph/dominance checks and transform-correspondence verification MUST
compile only under `cfg(test)`. Ordinary debug/release requests MUST NOT execute
them, and no runtime or feature switch enables them. Tests exercise the same
Engine algorithms with those verifiers enabled; a successful production product
means construction completed, not that an independent verifier accepted it.

Regression review checks original member identities, readable availability and
classified failures. Respect unrelated uncommitted work and obtain operator
approval before withdrawing or committing it.

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
- Keep backpressure, checked ranges and allocation failure handling. Request
  cancellation callbacks/tokens are not an engine API; the CLI/host owns process
  termination.
  The engine keeps no work or allocation ledger and no caller-configured numeric
  quota. Use try_reserve and report an actual allocation failure explicitly; do
  not introduce speculative JVM-size rejection formulas. Real format limits, input
  admission and bounded queues still apply.
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
  Ordinary loop counters do not require wrappers. Raw input, partial construction
  snapshots and complete analysis products MUST remain distinct where consumers
  depend on those guarantees; only the owning stage constructs its sealed product.
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
  assumption that input is valid.
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
  once. Lint exceptions MUST be local and justified.

## Code comments

Contributors MUST add comments only to explain non-obvious invariants, algorithm
choices, format constraints, units or failure behavior that the code does not
express clearly. Contributors MUST NOT add comments that restate code, narrate
edits, decorate sections or describe speculative future work. Prefer clearer
names and smaller functions over explanatory comments for avoidable complexity.

Public API documentation MUST describe the contract needed by callers, including
relevant errors, any panic preconditions, safety requirements, and resource or
failure behavior, without duplicating the design documents. Contributors MUST preserve comments that carry still-valid
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

Check existing design first and follow the root testing policy. Run applicable
active-only format, lint and build gates and report blocked or not-run evidence.
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
