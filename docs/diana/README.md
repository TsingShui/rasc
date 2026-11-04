# Diana documentation

Diana is a native Rust APK/DEX decompiler. The analyzed input is its only source
of decompilation facts. Available readable code, original identities and material
uncertainty are the product goal; Java legality and compilability are not output
gates.

[PIPELINE_DESIGN](PIPELINE_DESIGN.md) is the approved, locked architecture for the
complete P0–P9 product chain. Current Engine requests implement P0 → method reader
→ P1 → P2 → P3 → P4. Later method recovery, class assembly and rendering stages are not yet
active. Current scheduling and construction boundaries are in [phases](phases.md).

| Document | Responsibility |
|---|---|
| [PIPELINE_DESIGN.md](PIPELINE_DESIGN.md) | Approved product chain, stage ownership, algorithms and failure contracts. |
| [phases.md](phases.md) | Active stage map and Engine method scheduling. |
| [spec/00-overview.md](spec/00-overview.md) | Product boundary, failure kinds and safety obligations. |
| [spec/00-class.md](spec/00-class.md) | P0 class facts and independent member coverage. |
| [spec/01-input.md](spec/01-input.md) | Shared readers, MethodSource and P1 disassembly facts. |
| [spec/02-flow.md](spec/02-flow.md) | P2 execution/completion products and translation validation. |
| [spec/03-ssa.md](spec/03-ssa.md) | P3 exact-slot states, pruned SSA, original position mapping and correspondence checks. |
| [spec/04-types.md](spec/04-types.md) | P4 value/type sources, per-use views, Phi adaptations and position-related initialization. |
| [Engine README](../../crates/diana-decomp/README.md) | Implemented engine capabilities and inspection commands. |
| [CLI README](../../apps/diana/README.md) | Application commands, selectors and publication behavior. |
| [SOURCE_MAP.md](SOURCE_MAP.md) | Source attribution, reproducibility and evidence boundaries. |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution, Rust and acceptance rules. |
| [WRITING.md](WRITING.md) | Documentation conventions. |

P5–P9 contracts are maintained in PIPELINE_DESIGN, not duplicate chapters that
claim archived implementations are active. Removed specifications and historical
execution records remain available in git history. The archived implementation
in `ref-old-impl/` supplies semantic/check reference material, not another build
or method-analysis route.

Engine inspection uses the single `dbg_shell` Cargo example. Build/lint success,
byte roundtrip and finite corpus checks have different evidence scopes; none alone
proves complete decompilation correctness. Historical test cases and fixtures
were removed by operator decision; root AGENTS.md owns the scoped core-test
exception. Production safety and stage-owned translation checks remain required.
