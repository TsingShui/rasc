# Working on the Diana engine

Repository-wide AGENTS.md applies, including the rule that every authored Diana
document MUST follow ../../docs/diana/WRITING.md. This is the independent internal
engine package, with request resource control and cancellation but no public Engine.
Read ../../docs/diana/DESIGN.md, ../../docs/diana/ALGORITHMS.md and
../../docs/diana/TASKS.md before work.

- B01–B04 in ../../docs/diana/TASKS.md MUST pass before production engine work. An
  empty crate that builds is not T01 or E2E acceptance.
- Accept borrowed DEX bytes/declaration facts supplied by the application; no
  filesystem/ZIP/CLI loading, external runtime/process or MCP adapter here.
- Remain independent of rasc, dexdec, rusty-dex, Kuna and radguard code. Consult
  SOURCE_MAP.md before using reference material; respect its license limits.
- The engine MUST use `thiserror` major version 2 for typed error definitions.
  Public interfaces MUST preserve concrete error types rather than erase them
  into strings. Error classification follows ../../docs/diana/DESIGN.md §2.3.
  Contributors MUST obtain operator approval before introducing an alternative
  error framework.
- The engine MUST use `rayon` major version 1 for parallel compute scheduling.
  Contributors MUST obtain operator approval before introducing an alternative
  parallel compute scheduler. Rayon does not replace the cancellation, budget
  or backpressure requirements in the design.
- Add only modules consumed by an actual capability; no speculative P0–P9 trees,
  fake successful Engine results or placeholder getters/tests.
- Keep unsafe code forbidden. Preserve raw identity and program effect/handler/
  lock/initialization order; no requirement for stable source/local names or
  independent-report order. Charge budgets and validate untrusted ranges before
  allocation/slicing when those capabilities are implemented.
- Test complex semantics and malformed/resource boundaries through public
  interfaces using the frozen repository tests/e2e/ assets. Never regenerate
  expected values from engine output or drop required failures to turn green.
- Old sources/tests/release tooling live under ref-fork/rasc/, outside the root
  workspace. Do not automatically build/test/lint/format or depend on them.
- Build from the repository root with cargo build --locked -p diana-decomp.
  Keep publish=false and the independent scaffold version until release scope
  is explicitly approved; no automatic rasc version coupling.
