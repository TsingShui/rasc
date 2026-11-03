# Diana documentation

Diana is a native APK/DEX decompiler. Its product contract favors readable code
with material uncertainty and original DEX facts explicit. The current engine
still gates many Java results on stricter checks; the revised readable-output
contract is not yet implemented.

| Document | Responsibility |
|---|---|
| [DESIGN.md](DESIGN.md) | Product, engine and CLI contracts |
| [ALGORITHMS.md](ALGORITHMS.md) | Analysis and reconstruction algorithms |
| [SOURCE_MAP.md](SOURCE_MAP.md) | Source attribution and evidence boundaries |
| [WRITING.md](WRITING.md) | Documentation conventions |
| [e2e_cases.json](e2e_cases.json) | Case inputs and independent expectations |

Historical plans and batch records are available in git history. The active
workspace contains `apps/diana` and `crates/diana-decomp`; GUI and web directories
are placeholders. MCP is deferred. Reference material can be fetched explicitly
with `python3 tools/fetch_references.py --list`.
