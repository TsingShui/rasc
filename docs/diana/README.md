# Diana Design and Implementation

Diana has a native ordinary-method engine with checked input, SSA/types/storage,
structured/dispatcher Java, protected methods and initialization. T21 generic/bridge
and T22 named-nested source profiles have recorded acceptance. T23's restricted
enum/annotation definitions, runtime uses and defaults also pass their recorded
acceptance. The engine gate accepts every required engine invocation at b39805f,
so EngineCoreRC is accepted for `darwin-aarch64`. The application seams are also
accepted, so CLIProductRC holds for the same profile; Windows, ART and
representative-input performance remain not-run. Package
versions retain their scaffold markers. [TASKS.md](TASKS.md)
owns current capability and acceptance status. B01–B04 preparation is accepted on
macOS arm64; see [preparation evidence](batches/b02-b04-preparation.md).

Diana is a decompiler: it displays recovered code by default. A failed method is
shown in its original position with its reason and analyzable original DEX debug information; a Mixed
result does not suppress the rest of the class. Complete JavaUnit qualification
is separate from normal display, and text/JSON do not change recovery capability.
Resource quotas are unlimited by default; only caller-configured limits apply.
Java is not rejected based on guessed JVM costs. Range and format checks, actual
allocation failures, cancellation and backpressure remain in force; actual
compilation is independently verified for acceptance.

Only the following documents are maintained. There is no separate HTML board,
generator, or duplicate workflow, review or task document:

| Document | Sole responsibility |
|---|---|
| [DESIGN.md](DESIGN.md) | Product and CLI contract, Engine interface, modules and data structures, errors and budgets |
| [ALGORITHMS.md](ALGORITHMS.md) | Step-by-step algorithms for decoding, SSA, types and copies, control flow, exceptions, locks, classes and application safety |
| [TASKS.md](TASKS.md) | Development workflow, B01–B04, T01–T25, H01–H10, tools, verification and completion criteria |
| [SOURCE_MAP.md](SOURCE_MAP.md) | Reference versions, sources and licenses, material retrieval and evidence boundaries |
| [WRITING.md](WRITING.md) | Writing rules: requirement strength, sentence roles, terminology, numbers, negation ownership |

[e2e_cases.json](e2e_cases.json) is the sole case and independent-expectation
specification. Schema 2 binds input and expected results per invocation; draft-7
contains 86 required cases and 177 invocation scenarios, with explicit symbolic
targets and assertion evidence bindings. Evidence-based corrections
and additions are allowed; incorrect formats or gaps are not retained merely to
preserve a count. There is no separate copy of the expectations or web mirror.
The tool lock points directly to TASKS without a historical redirect.
[B01 review](batches/b01-spec-review.md) records draft-6 approval;
[debug fallback](batches/debug-fallback.md) records the operator-approved draft-7
revision. Failed Java retains checked original instructions, references and coverage
for analysis; it does not promise standard-Smali output or reassembly. TASKS §3
owns revision-specific acceptance and bounded self-test authorization.
The frozen JSON retains its draft metadata rather than acting as a progress counter.
Windows/Linux behavior and ART execution are not-run.

For implementation, read DESIGN, then the relevant section of ALGORITHMS, then
the current TASKS card. Consult SOURCE_MAP when checking evidence. Every authored
Diana document MUST follow [WRITING.md](WRITING.md); read it before editing any
document in this directory or an AGENTS/README file. The application
is only `apps/diana/`; the engine is only `crates/diana-decomp/`. The reference
tree was removed from the working tree at commit 5cb156c and is not built or
tested. GUI and Web remain
README-only placeholders; MCP is deferred. See the root [AGENTS.md](../../AGENTS.md)
for the remaining hard constraints.

Fetch references as needed with `python3 tools/fetch_references.py --list` or
`--only <id> ...`. The cache is ignored at `.cache/references/`; papers are kept
as TXT only. SHA-256 verification is disabled for reference and E2E tooling;
E2E still requires pinned dependencies and independent semantic certification.
