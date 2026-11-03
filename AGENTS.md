# Working on Diana

Diana is a native Rust APK/DEX decompiler. The workspace contains the CLI in
`apps/diana/` and the independent engine in `crates/diana-decomp/`. Read the
package-local `AGENTS.md` before working in either package.

## Product and safety

- Default retrieval MUST expose available readable code, preserve original member
  identities and DEX facts, and mark material uncertainty. A local failure MUST
  NOT suppress other members; text and JSON present the same result.
- Treat APK/DEX bytes as untrusted: check sizes, ranges and arithmetic before
  slicing or allocating. Do not panic, hang or allocate without bounds.
- The analysed input is the single source of decompilation facts. Do not require
  or parse external declarations, class files, android.jar, classpaths, symbol
  databases or network facts; treat absent facts as Unknown and keep the readable
  result with explicit uncertainty.
- Preserve effect, instruction, argument, exception, lock and initialization
  order, classified errors, cancellation and backpressure. Numeric quotas are
  opt-in; never silently truncate a successful result.
- Production is native Rust, forbids `unsafe`, and has no external runtime or
  process dependency. The application owns files and ZIPs; the engine borrows
  input bytes and declaration facts.
- Use the shared P2→P8 method analysis chain. P9 renders existing products and
  original facts; it does not perform analysis or retry reconstruction.
- Before changing phase access, exports, construction authority or scheduling,
  present the producer/consumer need and obtain operator approval. Never bypass
  a missing producer or sealed boundary with a parallel analysis route.

## Workflow

- Read the owning [stage chapter](docs/diana/README.md) before capability
  changes.
  Identify consumed facts, guarantees, invariants and classified failure behavior.
  If contracts conflict with the change, stop and request a decision.
- For detailed architecture, Rust conventions, dependency choices, acceptance,
  file verification, corpus handling and reference isolation, read
  [CONTRIBUTING](docs/diana/CONTRIBUTING.md) for the relevant work. Read
  [WRITING](docs/diana/WRITING.md) before editing Diana documentation.
- Inspect existing coverage before adding core tests. Test semantic and safety
  behavior through public interfaces with independent expectations. Never add
  tests, fixtures or CI test jobs for non-core tooling, nor CLI help/argument/
  getter/forwarding tests. Do not change expected results to match engine output.
- Use `cargo nextest run` for Rust unit/integration tests, not `cargo test`.
  If nextest is unavailable, report the run blocked. Rustdoc's `cargo test --doc`
  exception requires operator approval. Missing or skipped evidence is not a pass.
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
cargo nextest run --workspace --all-features --locked
```

Run relevant core cases for focused changes; run the full applicable gates at
integration boundaries. Report failures and unrun evidence accurately.

## Documentation map

- [phases](docs/diana/phases.md): phase map and current scheduling entry.
- [spec/00–09](docs/diana/README.md): product overview and phase chapters;
  detailed contracts are still being migrated.
- [spec/00–10](docs/diana/spec/): product, input, phases, CLI and algorithms.
- [SOURCE_MAP](docs/diana/SOURCE_MAP.md): sources, attribution and reference use.
- [e2e_cases.json](docs/diana/e2e_cases.json): case inputs and independent expectations.
- [CONTRIBUTING](docs/diana/CONTRIBUTING.md): detailed contribution rules.
- [WRITING](docs/diana/WRITING.md): documentation conventions.

Technical contracts are being revised toward readable output; do not interpret
old verification gates as evidence that the revised behavior is implemented.
