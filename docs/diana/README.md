# Diana documentation

Diana is a native APK/DEX decompiler. The analysed input is its only source of
decompilation facts; external declarations are not consulted. Its product contract
favors readable code with material uncertainty and original DEX facts explicit.
Ordinary methods use `JavaMethod` with concrete recovery diagnostics and no
source-verification status. Class assembly uses original header and member facts,
not a Java legality certificate. Missing analysis or lowering products still
use the classified debug exit.

| Document | Responsibility |
|---|---|
| [phases.md](phases.md) | Current stage map and scheduling entry |
| [PIPELINE_DESIGN.md](PIPELINE_DESIGN.md) | Proposed complete-product pipeline, stage trust and class/method ownership |
| [spec/00-overview.md](spec/00-overview.md) | Product and engine overview |
| [spec/00-class.md](spec/00-class.md) | Independent P0 class declarations and debug snapshot |
| [spec/01-input.md](spec/01-input.md) | Input ownership and original facts |
| [spec/02-flow.md](spec/02-flow.md) | Flow and effects |
| [spec/03-ssa.md](spec/03-ssa.md) | Definitions, uses and SSA |
| [spec/04-types.md](spec/04-types.md) | Types, declaration constraints and use conversions |
| [spec/05-variables.md](spec/05-variables.md) | Storage and copies |
| [spec/06-regions.md](spec/06-regions.md) | Regions and control structure |
| [spec/07-java.md](spec/07-java.md) | Java reconstruction |
| [spec/08-emission.md](spec/08-emission.md) | Emission and presentation |
| [spec/10-cli.md](spec/10-cli.md) | CLI and application surface |
| [SOURCE_MAP.md](SOURCE_MAP.md) | Source attribution and evidence boundaries |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Detailed contribution and engineering rules |
| [WRITING.md](WRITING.md) | Documentation conventions |

The method chain has eight numbered phases, P1–P8. P0 provides an independent
class-fact snapshot and an internal Cargo example for inspection. `context/` owns cross-DEX lookup
and caches P1 declaration products; `engine/` owns scheduling. P2 retains exact
call formals, P4 owns typing and declaration-constraint collection, and P4 owns
declaration-constraint helpers. P7 consumes the constraints for source binding.
Partial declaration directories and lazy field facts are retained. Complete
input annotation projection and systematic readable-output uncertainty remain
incomplete.

[PIPELINE_DESIGN.md](PIPELINE_DESIGN.md) records the proposed P0 class producer,
independent method reader (`MethodSource`), P1 disassembly (`DecodedMethod`),
P2–P7 method analysis/recovery, P8 class assembly and P9 rendering design. That
restructuring is not implemented; the stage chapters and `phases.md` describe the
current contracts. The selected P3 target is exception-aware pruned SSA with
on-demand iterated dominance frontiers; the current implementation still uses
fixed block-entry Phi construction. The selected P6 target prefers natural regions
and retains explicit labelled graph transfers in Java-like notation when needed,
without generating a dispatcher or withholding the whole readable method.
The proposal's [product contracts](PIPELINE_DESIGN.md#44-产品持有构造与消费接口)
and [implementation strategy](PIPELINE_DESIGN.md#10-新结构优先的实现策略) prioritize
new products and module structure. Existing code supplies semantic/check obligations
and reusable algorithms, not a requirement to migrate files or preserve private IR.

The chapters carry the product and algorithm contracts. Java compilability is
not an output goal. Input safety, operation correspondence and semantics-preserving
transform checks remain part of the shared analysis chain.

Historical plans and batch records are available in git history. The active
workspace contains `apps/diana` and `crates/diana-decomp`; GUI and web directories
are placeholders. MCP is deferred. Reference material can be fetched explicitly
with `python3 tools/fetch_references.py --list`.
