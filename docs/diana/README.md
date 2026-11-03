# Diana documentation

Diana is a native APK/DEX decompiler. Its product contract favors readable code
with material uncertainty and original DEX facts explicit. The current engine
still gates many Java results on stricter checks; the revised readable-output
contract is not yet implemented.

| Document | Responsibility |
|---|---|
| [phases.md](phases.md) | Stage map and scheduling entry |
| [spec/00-overview.md](spec/00-overview.md) | Product and engine overview |
| [spec/01-input.md](spec/01-input.md) | Input ownership and original facts |
| [spec/02-flow.md](spec/02-flow.md) | Flow and effects |
| [spec/03-ssa.md](spec/03-ssa.md) | Definitions, uses and SSA |
| [spec/04-calls.md](spec/04-calls.md) | Calls and declaration facts |
| [spec/05-types.md](spec/05-types.md) | Types and use conversions |
| [spec/06-variables.md](spec/06-variables.md) | Storage and copies |
| [spec/07-regions.md](spec/07-regions.md) | Regions and control structure |
| [spec/08-java.md](spec/08-java.md) | Java reconstruction |
| [spec/09-emission.md](spec/09-emission.md) | Emission and presentation |
| [spec/10-cli.md](spec/10-cli.md) | CLI and application surface |
| [SOURCE_MAP.md](SOURCE_MAP.md) | Source attribution and evidence boundaries |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Detailed contribution and engineering rules |
| [WRITING.md](WRITING.md) | Documentation conventions |
| [e2e_cases.json](e2e_cases.json) | Case inputs and independent expectations |

The chapters carry the product and algorithm contracts. Older verification-only
gates survived migration and still conflict with the readable-output direction;
reconciling them is required before implementing that direction.

Historical plans and batch records are available in git history. The active
workspace contains `apps/diana` and `crates/diana-decomp`; GUI and web directories
are placeholders. MCP is deferred. Reference material can be fetched explicitly
with `python3 tools/fetch_references.py --list`.
