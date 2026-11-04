# Working on Diana

Diana is a native Rust APK/DEX decompiler. The workspace contains the CLI in
`apps/diana/` and the independent engine in `crates/diana-decomp/`. Read the
package-local `AGENTS.md` before working in either package.

## Product and safety

- Default retrieval MUST expose available readable code, preserve original member
  identities and DEX facts, and mark material uncertainty. A local failure MUST
  NOT suppress other members; text and JSON present the same result.
- Java legality or compilability is not a decompilation goal or output gate.
  Keep concrete recovery diagnostics, not whole-result verification badges;
  retain input safety, analysis invariants and semantics-preserving transform checks.
- Treat APK/DEX bytes as untrusted: check sizes, ranges and arithmetic before
  slicing or allocating. Do not panic, hang or allocate without bounds.
- The analysed input is the single source of decompilation facts. Do not require
  or parse external declarations, class files, android.jar, classpaths, symbol
  databases or network facts; treat absent facts as Unknown and keep the readable
  result with explicit uncertainty.
- Preserve effect, instruction, argument, exception, lock and initialization
  order, classified errors, cancellation and backpressure. Cancellation and
  bounded queues are not pagination; never silently truncate a successful result.
- Production is native Rust, forbids `unsafe`, and has no external runtime or
  process dependency. The application owns files and ZIPs; the engine borrows
  input bytes and declaration facts.
- Use one shared ordinary-method analysis chain. Migrate only to the approved
  target below; rendering consumes existing products and original facts without
  starting analysis or retrying reconstruction.
- Never bypass a missing producer or sealed boundary with a parallel analysis
  route. Follow the approved refactoring authority below for interface changes.

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
  document producer/consumer needs, retain safety and semantic-check obligations,
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
  delivered P0/P1 products. Do not add phase-specific debug binaries, renderers or
  debug examples.
- Contributors MUST NOT develop, add or restore any unit tests, including inline
  `#[test]` functions, `#[cfg(test)]` modules and private-phase test harnesses.
  Existing test cases and fixtures are removed by operator decision.
- Contributors MUST NOT add tests, fixtures or CI test jobs for non-core tooling.
  Production input checks, analysis invariants and semantics-preserving transform
  checks remain required. Build/lint success does not establish semantic correctness.
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
- [spec/00–09](docs/diana/README.md): product overview and phase chapters;
  detailed contracts are still being migrated.
- [spec/00–10](docs/diana/spec/): product, input, phases, CLI and algorithms.
- [SOURCE_MAP](docs/diana/SOURCE_MAP.md): sources, attribution and reference use.
- [CONTRIBUTING](docs/diana/CONTRIBUTING.md): detailed contribution rules.
- [WRITING](docs/diana/WRITING.md): documentation conventions.

Technical contracts are being revised toward readable output; do not interpret
old verification gates as evidence that the revised behavior is implemented.
