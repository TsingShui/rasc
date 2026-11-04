# Working on Diana

Diana is a native Rust APK/DEX decompiler. The workspace contains the CLI in
`apps/diana/` and the independent engine in `crates/diana-decomp/`. Read the
package-local `AGENTS.md` before working in either package.

## Product and safety

- Default retrieval MUST expose available readable code, preserve original member
  identities and DEX facts, and mark material uncertainty. A local failure MUST
  NOT suppress independently recoverable fragments within the same method or
  other members; text and JSON present the same result.
- Java legality or compilability is not a decompilation goal or output gate.
  Keep concrete recovery diagnostics, not whole-result verification badges;
  retain input safety and necessary construction error handling. Independent
  translation/correspondence verification belongs to tests, not normal requests.
- Normal decompilation MUST NOT run independent output validation, symbolic replay
  or duplicate graph/dominance verification. Compile these only under `cfg(test)`;
  do not add a runtime or feature switch that enables them on ordinary requests.
- Treat APK/DEX bytes as untrusted: check sizes, ranges and arithmetic before
  slicing or allocating. Do not panic, hang or allocate without bounds.
- The analysed input is the single source of decompilation facts. Do not require
  or parse external declarations, class files, android.jar, classpaths, symbol
  databases or network facts; treat absent facts as Unknown and keep the readable
  result with explicit uncertainty.
- Preserve effect, instruction, argument, exception, lock and initialization
  order, classified errors and backpressure. Bounded queues are not pagination;
  never silently truncate a successful result. Request cancellation callbacks and
  tokens are not part of the CLI-oriented engine API; process termination belongs
  to the CLI/host.
- Production is native Rust, forbids `unsafe`, and has no external runtime or
  process dependency. The application owns files and ZIPs; the engine borrows
  input bytes and declaration facts.
- Use one shared ordinary-method analysis chain. Migrate only to the approved
  target below; rendering consumes existing products and original facts without
  starting analysis or retrying reconstruction.
- Never bypass a missing producer or sealed boundary with a parallel analysis
  route. Follow the approved refactoring authority below for interface changes.

## Error handling

- All error paths MUST follow [failure locality](docs/diana/PIPELINE_DESIGN.md#9-局部失败与方法退出):
  contain failure to the smallest reliably identified scope and propagate missing
  facts only to their actual dependents. Partial method recovery is a normal result.
- Failure kind MUST NOT alone determine stop scope. A method-private invariant or
  allocation failure MUST NOT automatically terminate sibling methods, classes or
  DEX images; widening a stop requires a concrete shared-input/resource reason.
- Preserve recovered code, original facts, identities and precise unavailable
  ranges/reasons. Do not erase effects, fabricate values or promote incomplete
  analysis snapshots to complete products to appear tolerant.

## Pipeline refactoring authority

The target in [PIPELINE_DESIGN](docs/diana/PIPELINE_DESIGN.md) is approved: P0 owns
class facts; an independent method reader prepares input; P1 disassembles; P2–P7
process one method; P8 assembles the class and plans names; P9 renders. Engine
holds and delivers products and schedules method-level tasks. Current phase
chapters describe the existing implementation until migrated to this target.

- Contributors MAY decide technical details within this target without repeated
  approval: private product layout, lifetimes, algorithm invariants, classified
  outcomes, check organization and implementation order. Prefer implementing the
  new product structure; existing algorithms can be reused or rewritten without
  preserving old private IR layouts or moving files one-to-one. Contributors MUST
  document producer/consumer needs, retain input safety and test-owned semantic
  verification obligations,
  and remove replaced paths when the new chain enters the shared formal entry.
  Rewriting MUST NOT introduce a second method-analysis or retry route.
- Contributors MUST NOT change agreed output behavior, hide available members or
  introduce a generated dispatcher. Natural and explicit Java-like expressions
  remain readable results, not whole-method qualification gates.
- Contributors MUST NOT introduce external declarations, new dependencies or
  numeric resource quotas for this refactoring.
- Contributors MUST NOT change phase ownership or scheduling beyond the approved
  target. Stage implementations MUST NOT invoke another stage's analysis.
- Contributors MUST NOT widen stage input access, exports or product-construction
  authority beyond the approved producer/consumer contracts. Private fact views
  and owning-stage constructors MUST NOT become general upstream access.
- Default approval does not authorize these prohibited changes. If the target
  cannot be implemented within these constraints, stop the affected work and
  report the concrete conflict; do not add a bypass or treat silence as an exception.

## Workflow

- Contributors MUST NOT develop by accumulating local patches that mask symptoms,
  special-case observed inputs or bypass missing producer facts. Contributors MUST
  trace each problem to its architectural cause and implement a general solution
  in the owning module, with explicit input guarantees and failure behavior.
  Contributors MUST update affected producers, consumers and invariants together
  and remove superseded paths. Small, focused changes remain valid when they fix
  the shared cause; this rule does not require speculative frameworks or broad rewrites.
- Read the owning [stage chapter](docs/diana/README.md) before capability
  changes.
  Identify consumed facts, guarantees, invariants and classified failure behavior.
  If contracts conflict with the change, stop and request a decision.
- For detailed architecture, Rust conventions, dependency choices, acceptance,
  file verification, corpus handling and reference isolation, read
  [CONTRIBUTING](docs/diana/CONTRIBUTING.md) for the relevant work. Read
  [WRITING](docs/diana/WRITING.md) before editing Diana documentation.
- For development verification of the engine, use the `dbg_shell` Cargo example
  as the single inspection entry:
  `cargo run -p diana-decomp --example dbg_shell --locked -- --dex <classes.dex> --exec '<json>'`.
  It accepts a bare DEX, emits one JSON response per command, and exposes the
  delivered P0/P1/P2/P3 products. Do not add phase-specific debug binaries, renderers or
  debug examples.
- Contributors MUST NOT develop, add or restore any unit tests, including inline
  `#[test]` functions, `#[cfg(test)]` modules and private-phase test harnesses.
  Existing test cases and fixtures are removed by operator decision.
  The sole operator-approved exception is the one core test in
  `crates/diana-decomp/src/dynamic_instructions_test.rs`, exercised through public
  Engine interfaces with synthetic DEX input, including P2/P3 correspondence cases. Contributors MAY maintain this
  scoped test and run it with `cargo nextest run`; this does not authorize other
  tests, fixture suites or private-stage harnesses.
- Contributors MUST NOT add tests, fixtures or CI test jobs for non-core tooling.
  Production input safety and necessary construction errors remain enforced;
  independent transform verification remains test-only. Build/lint success does not establish semantic correctness.
- Respect uncommitted work; obtain approval before withdrawing or committing
  unrelated changes. Do not commit, push, publish or tag unless requested.
  Authorized commit subjects start with `[AUTOMATED] ` and a Conventional type.
- Keep private/unauthorized APKs and identifying corpus data out of commits;
  use `.cache/` locally. Consult `tools/audit/desensitize_words.txt` and run
  `python3 tools/audit/desensitize_check.py --range HEAD`.

## Common checks

Run from the repository root with Rust 1.93 or newer:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
```

Run applicable format, lint and build checks. Report failures and unrun evidence
accurately; do not report removed tests or missing semantic evidence as a pass.

## Documentation map

- [phases](docs/diana/phases.md): phase map and current scheduling entry.
- [documentation entry](docs/diana/README.md): active contracts and package docs.
- [PIPELINE_DESIGN](docs/diana/PIPELINE_DESIGN.md): approved P0–P9 architecture.
- [spec](docs/diana/spec/): product boundary and implemented P0/input/P1/P2/P3 contracts.
- [SOURCE_MAP](docs/diana/SOURCE_MAP.md): sources, attribution and reference use.
- [CONTRIBUTING](docs/diana/CONTRIBUTING.md): detailed contribution rules.
- [WRITING](docs/diana/WRITING.md): documentation conventions.

PIPELINE_DESIGN is the approved architecture, not an implementation-status claim.
Use phases and package READMEs for active capabilities; archived specifications
and verification gates are not current contracts or acceptance evidence.
