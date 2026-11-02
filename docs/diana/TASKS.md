# Diana tasks and acceptance

**Status: B01–B04 preparation is accepted on macOS arm64. The ordinary method
chain includes checked heap operations, exception SSA/storage, protected Java,
monitor scopes and T20 initialization. T20's supported source profile and fixed
D041–D047/D079 obligations pass; unproved initialization remains classified
Debug/Mixed. T21 generic/bridge declarations and T22 named ownership pass their
recorded source profiles. T23's restricted enum/annotation profile and T25's
engine gate pass their recorded acceptance, so EngineCoreRC is accepted for
`darwin-aarch64` at b39805f. Core/application rollup remains open. The frozen
draft-7 specification holds
86 required cases and 177 must-run invocation scenarios.**
This page owns the workflow, preparation, task cards and verification. Read
[DESIGN.md](DESIGN.md) for interfaces and data, [ALGORITHMS.md](ALGORITHMS.md) for
algorithms, and [SOURCE_MAP.md](SOURCE_MAP.md) for sources.
[e2e_cases.json](e2e_cases.json) is the single specification of cases and
independent expected values. An author MAY revise a faulty form, input or oracle
with supporting evidence and MAY append a counterexample. The project MUST NOT
keep a defect to preserve a fixed count, and MUST NOT maintain a second HTML board
or a copy of the expected values.

## 1. Working rules

The full B01→B02→B03→B04 sequence precedes production feature work. Small-batch
development is not a preparation exemption. B04 tool self-tests still require
separate authorization; renaming them or running them once by hand does not claim
the same guarantee. A T/H dependency means that the named producer artifact is
ready; it does not require an integration test to complete before the artifact that
consumes it. A partial result MUST NOT mark a whole card done, and an empty file or
an empty pass is not an artifact.

Batch records, cards and specification edits MUST follow `WRITING.md`, which owns
normative keyword strength, sentence roles, terminology and document ownership.
Changing wording MUST NOT change requirement strength or expected values.

Each batch has one publicly observable goal: confirm the fixed cases and input
certification → expose a real failure → make the smallest change in the owning
module → cover positive, negative, resource and already-accepted regression →
record and finish. Create `docs/diana/batches/<id>.md` only when work actually
starts, and record only the goal and non-goals, the fixed D IDs and versions, the
producer and algorithm section numbers, the changed functions, the budget and
cancellation points, the real commands, execution counts, platform, repository-tracked evidence paths and reproducible commands, and the
remaining obligations. Batch evidence references follow WRITING §5. That record MUST NOT copy expected
values or start a new progress database. Do not start a subagent without user
authorization, do not mix several capabilities in one batch, and do not pre-lay
every phase, trait, registry or AST node.

Stop the affected part when a design counterexample appears, when a public
interface, a fixed expected value or an effective dependency needs a change, when the
oracle or the tooling cannot be certified, or when an unauthorized auxiliary test
would be required. A blocked or negative result is not a pass. A genuine oracle
error needs independent evidence and a separate review and revision; the engine
MUST NOT refresh it from actual output. Core tests cross public libraries and
modules. Do not add CLI black-box tests, mechanical getter tests, or goldens for
private IR, formatting or invocation counts. Optional beautification defaults off.

The [stage-entry authority batch](batches/stage-entry-authority.md) records
compiler-enforced construction-entry restrictions and preserved public results.

## 2. Order and completion definition

The first chain is D009 (with D055/D059 rejection) → D001 fixed Phi/edge copy →
D002/D003 dispatcher loops. After calls and types are ready, D029/D030 shadow/try
follow, and the remaining fixed cases then expand. A card MUST NOT skip a named
input.

| Increment | Tasks | Delivered behavior | Hard acceptance / condition that blocks the next increment |
|---|---|---|---|
| I0 Safe input | T01–T05 | Original scope/exact proto; 035 and T04 native 041; format/metadata/raw path; the same control wiring | Check ranges before allocating, never guess an unknown width; complete decoded coverage never omits required original facts; unprepared tools or fixtures are not a pass |
| I1 Scalar and loops | T06–T10 | Parameters/arithmetic/return, D001 merge, D002/3 loop copy and dispatcher, primitive/null/FP | Fixed SSA, parallel copy, DA initial values unobservable; cmp NaN bias and word width correct |
| I2 Calls and objects | T11–T12, T15 | Descriptor binding, receiver/args order, adjacent new/init, field/array/cast effects | Complete catalog and checked throws, no-extra-check, allocation/init and volatile/class-init not moved |
| I3 Ordinary natural structure | T13–T14 | Bounded Sequence/Diamond/Loop/Switch; an unmatched graph still uses the legal dispatcher | Same-case behavior equal, never compare text; charge before candidates, no duplicated effect, no reordered copy |
| I4 Exceptions | T16–T18 | Throw-before SSA, shadow, ordered catches and one nested layer | D029–D035 semantics; the per-site catch-stack does not widen the try, and the handler itself only gets the outer protection |
| I5 Locks and initialization | T19–T20 | Alias root/lock-stack, synchronized, static/constructor/final | Every exit releases exactly; static final introduces no new constant inlining; the final normal path assigns exactly once |
| I6 Complete source declarations | T21–T23 | Bridge, named inner, enum/annotation; Explicit/Generated and Mixed classification | Generated rules match item by item; the full unit compiles directly, without splice patching, and generated members are not counted as code-free |
| I7 Core rollup | T25 (required), T24 (optional) | Required core cases and boundary evidence for EngineCoreRC; optional pure recovery defaults off | T25 does not wait for T24; resources and cancellation run from I0 throughout, not added at the end; H08 rolls up all required cases/invocations |

The first Java chain needs only the modules that actually consume it now; do not
build every phase directory or a full AST first. The `answer7` in T06 is only an
explanation; the actual fixed case is D009. A single-block result without a call or
without P7 is a valid empty fact, not a fake pass.

- EngineCoreRC: every case/invocation in the JSON that is required with
  `acceptanceGate` `engine` or `engine-and-application`; currently 81 cases
  (D001–D065, D071–D086).
- ProductRC: every required case/invocation in the JSON; currently 86 cases,
  reusing engine evidence and adding the application obligations.
- D065/D085 `engine-and-application` requires both seams to satisfy the same
  expected value of each invocation. A missing application side means neither the
  whole case nor the product passes, and the two seams are not counted as two
  cases.
- Compute the denominator and the selector from the JSON fields. Do not hard-code
  an old ID endpoint or a historical 70/65 count. One missing invocation fails the
  whole case.
- Showing partial results by default does not lower the required Java bar.
  Unimplemented features, missing tools and unexecuted platforms still fail. T24
  blocks neither RC.
- Diana is a decompiler: default retrieval MUST NOT let a whole-class JavaUnit gate
  an already-recovered method. Show the local failure at the original method
  instead of turning every text result into a diagnostic error or requiring the
  user to switch to JSON. A large set of common legal inputs still falling back to
  raw is a quality problem, and the existence of a display option is not a reason
  to leave the algorithm unchanged.
- The root Cargo gate runs Diana only. Zero tests or a scaffold build is not
  semantic acceptance. Measure performance only on release builds with authorized
  inputs.

## 3. Preparation tasks B01–B04

The current rule in the root [AGENTS.md](../../AGENTS.md) forbids tests for any
non-core code unless the user authorizes them explicitly. The path/size/timeout
fault tests for the preparer itself in B02 and the fault-injection self-tests for
the comparator and runner in B04 MUST NOT derive authorization from existing plan
text, and MUST NOT be renamed as "core E2E preparation" to bypass that rule.
Independent semantic certification of self-made sources and inputs remains
preparation for core cases; keep the two categories distinct.

The operator explicitly requested completion of B01–B04, authorizing only their
listed preparer and comparator/runner fault tests. This bounded authorization and
the actual execution evidence are recorded in
[preparation acceptance](batches/b02-b04-preparation.md). It does not authorize
other auxiliary tests, CLI black-box tests or ART execution. The original semantic pass
conditions remain unchanged. The operator-approved [SHA-256 verification removal](batches/sha-verification-removal.md)
revises only fingerprint checks: existing digest metadata in locks, recipes and
historical evidence is informational, never an admission or acceptance gate.

[B01 review](batches/b01-spec-review.md) approves the original draft-6 bytes; [debug-fallback revision](batches/debug-fallback.md) records the operator-approved draft-7 output contract.
The preparation artifacts satisfy the following gates:

| Gate | Required artifact before production implementation |
|---|---|
| B01 | Reviewed symbolic selectors, case-specific assertion mapping, all independent oracles/mutation obligations and the approved revision; no format-only approval |
| B02 | Distributable self-made sources/recipes for every required invocation, actual tool/transitive/platform locks and a portable preparer |
| B03 | Actual independent DEX/041/AXML/ZIP projections and source references, staged input certificates, and any scalar oracle needed before engine integration |
| B04 | Comparator/runner using those certificates, plus rejection of every listed injected fault under explicit self-test authorization |

B01–B04 are accepted for the executed `darwin-aarch64` profile. The checked-in
sources, tool lock, manifest and runner reproduce from a clean source snapshot
and cache, including a checkout path with spaces and a different working directory.
Windows, Linux and ART are not-run, not passes. The first production slice implements T01 request work/allocation accounting
and cancellation; see [its batch record](batches/t01-request-control.md).
[Method budgets](batches/t01-method-control.md) add normal/raw isolation and
classified control errors. [Checked reader/header tests](batches/t02-reader-header.md)
exercise public input, malformed cases and request resource/cancellation behavior.
[Declaration-index tests](batches/t02-declaration-index.md) cover complete
signatures, MUTF-8, local failures and checked code/handler ranges.
[Member declaration isolation](batches/member-declaration-isolation.md) verifies
sibling damage through Engine selection and Java source admission.
[Typed boundaries](batches/t02-typed-boundaries.md) record the approved input API
migration. [Instruction layout](batches/t03-instruction-layout.md) checks shared
opcode/version/operand and payload envelopes. [Method structure](batches/t03-method-structure.md)
adds register/reference and control-flow boundary checks, a budgeted raw-layout
visitor and functional method-budget/cancellation tests.
[Original operands](batches/t03-original-operands.md) add borrowed literal,
register-word, relative-target and payload-bit facts using the same decoder as
the structural validator. [Debug snapshots](batches/t03-debug-snapshots.md)
retain original member/code facts and explicit coverage under the fallback
budget. [Safe excerpts](batches/t03-safe-excerpt.md) distinguish an unknown first
encoding from a decoded snapshot using the same registry, without assigning a
width. [Invoke structure](batches/t03-invoke-structure.md) validates 035 argument
word counts, wide word adjacency and move-result kinds using the normal method
ledger. [Original debug programs](batches/t04-debug-program.md) retain parameters,
line/PC changes and local-variable events, with classified local failures and
partial byte ranges. [Encoded values](batches/t04-encoded-values.md) add checked
static-value counts, exact bits/references and iterative nested arrays/annotations
with explicit partial coverage. [Core test consolidation](batches/core-test-consolidation.md)
groups the invoke/debug regression coverage and reuses resource/cancellation
tests. T05 later connects the prepared runner to the real public library. Missing
Engine entrypoints currently produce `not_implemented` for the complete suite.

## B01 — Case and expected-value specification review, freeze v1

**Dependencies**: none. **Phase**: test preparation, not a compute phase.
**Scope**: all required cases/invocations in the JSON and the integer-inlining
option.

**Files**: `docs/diana/e2e_cases.json`, TASKS §4, DESIGN §6, SOURCE_MAP §10;
the `tests/e2e/` manifest schema and its derived execution view, all relative to the
checkout. `docs/diana/e2e_cases.json` remains the only expected-value specification.

**Input→output / Interface**: behavior recipes and fixed inputs → independent
expected values, mutation killers and oracle type, with state moving from
`designed-not-built` to `spec-approved`. Record the reviewer and the
specification version; these are not fake engine results.

**Steps**:
1. For each case, check the original Java/DEX semantics, the inputs and the
   return/exception/effect order; the worked example and the JVM reference driver
   each support the conclusion. Fix a specification error before the engine
   implements the behavior.
2. Confirm seam and validationScope, and keep method-in-known-context, full-unit,
   full-unit-set and rendered-result separate. An internal op interpreter MUST NOT
   serve as an exception or lock oracle.
3. Mark a known-legal example that Java cannot express as Debug/Mixed. Mark a
   damaged input as Malformed/Unavailable; do not assert a uniform any-error for
   every negative case.
4. Verify that a case requiring Java capability does not switch to expectedDebug
   because an implementation is missing. Specify optional beautification
   separately so that it does not affect core expectations.
5. Lock the suite members, inputs, expected values, schema and distributable
   sources, and record the reviewed specification revision. The suite MUST NOT contain a
   real private APK or path.

**Invariants / limits**: A case follows observed behavior and is not tied to
phase names or print form. Inputs and expected values are designed
independently; do not generate a golden from the output under test. B01 approves
a specific revision and does not freeze a count that never changes.
Specification corrections and input strengthening supported by independent
evidence are allowed; preserve the problem identity and valid semantics, append
a counterexample under a new ID, and never delete an old case to manufacture a
pass.

**Tests**: Specification review is not a CLI test. For an oracle derivation with
a counterexample, confirm that the counterexample really has a different return,
exception or sequence. Do not write tests for JSON getters.

**Acceptance**: Every required case/invocation has concrete inputs, a selector,
expected values, an oracle/mutation and an independent certification obligation;
the scope, source and path review passes and the approved revision is recorded.
**Completion**: An engine developer no longer needs to decide what that
invocation should return. A draft revision alone is not B01 approval.

**Not in scope**: CLI black-box tests, private-phase tests, output-text goldens,
real-corpus downloads, engine implementation.

## B02 — Source fixtures, tool version locks and a portable preparer

**Dependencies**: B01. **Phase**: test tooling, not a production phase.

**Files**: `tests/e2e/sources/`, `expected/`, `toolchain.lock.json`,
`manifest.json`, `cases.json`; `tools/e2e.py` (a development tool option that
prefers the Python 3.11+ standard library), plus the necessary tests-only Java,
smali and Driver files. All paths are relative to the repository, and generated
artifacts live only under `.cache/e2e/`.

**Input→output / Interface**: TASKS §4.5 fixed artifact coordinates (source
entries in SOURCE_MAP §10) → complete version and dependency locks; every required
recipe in the JSON → executable self-made sources and literal expected values,
exposed through the `fetch-tools`, `prepare` and `verify-inputs` commands. No
personal SDK, `android.jar` or reference-engine cache is required.

**Steps**:
1. Split every required recipe/invocation by suite into an independent source
   file or driver, and check in all dependencies of the self-made sources.
   Preserve the known context of a single-target method and do not insert a stub
   that changes semantics. Bind inputs and expected values by
   `(case id, invocation id)`, not as two positionally paired arrays.
2. Record the JDK asset, URL, version and size for each supported platform, and
   the coordinates, URLs, versions and sizes of R8/smali/dexlib2 and all transitive JARs. Download
   and run the main class according to the lock; do not blindly use
   `java -jar smali.jar`.
3. Confirm that the Java 21 tool with `--release 8`, the R8 min-api/DEX version
   checks and the smali 035 generation path agree. A case that needs a newer
   version gets its own profile; do not guess a version.
4. `prepare` runs only the locked tools and self-made sources, and generates the
   DEX/ZIP/invalid-mutation/041 recipes plus the origin/identity certificate.
   Take the original return and trace expected values from the specification and
   the source reference, never from Diana output.
5. The preparer supports a different working directory, Windows separators,
   classpath and exe suffixes, and paths containing spaces. Use the system temp
   API and do not hard-code the home directory or an OS temporary path. Only
   `fetch` may use the network.
6. A CRC-broken, duplicate-entry, undefined-reference, isolated-surrogate or
   cyclic-Phi input needs an independent raw check. A named byte patch field,
   opcode anchor is not an unchecked bare offset constant.

**Invariants / limits**: Network access and subprocesses belong to the
development tool only; production crates do not depend on them. An unlocked
artifact or a placeholder coordinate blocks preparation. A missing tool cache is
an error; an author's preinstalled SDK MUST NOT stand in for it.

**Tests**: After a rebuild, the source driver agrees with the independent
expected values of each required invocation. A fixed error input and a
size, timeout or cross-platform path error are explicit prep failures; B04 owns
comparator falsification. Testing the non-core preparer itself still needs the
separate authorization of §3 and MUST NOT expand into simple CLI tests.

**Acceptance**: A clean cache can explicitly fetch and generate every core
fixture; every input, expected and dependency path is distributable and the
tool/dependency locks and fixture provenance are complete. List a case that is not generated or lacks
an oracle as blocked, never removed. **Completion**: Engine development no
longer needs a temporary fixture or source dependency designed mid-phase.

**Not in scope**: Production Rust/API implementation, automatic external
platform catalogs, real-corpus downloads, private test paths.

## B03 — Independent input and baseline verification, freeze artifacts v1

**Dependencies**: B02. **Phase**: input and baseline certification.

**Files**: `tests/e2e/manifest.json`, the independent dexlib2 projection and
helper, the source-reference Driver, the required tests-only scalar oracle, and the
`.cache/e2e/preparation-report.json` generated report (no machine absolute path
enters a golden).

**Input→output / Interface**: fixture artifacts → original identities, raw structured
projection, JVM driver reference trace and a specification-conformance report;
manifest state becomes `inputs-certified`.

**Steps**:
1. An independent decoder or dexlib2 checks the method/ref/word/branch/payload/
   handler/string units against every JSON `inputRequirements` item. In particular,
   confirm that cmpl/cmpg, an unused cast, fill/FreshArray, the old definition
   before a throw, an explicit encoded tag, a real bridge/outer capture and sget
   are still in the input. If the structure under test is missing, preparation
   fails; source text, assembler exit 0 or an unrelated catalog gap MUST NOT stand
   in for it. Do not freeze private Phi/AST counts.
2. For a source-origin core case, run the real reference Driver on an isolated
   JVM and check the specification's independent expected values. FP bits, UTF-16,
   locks and the fresh-process rule for init are strict.
3. When handwritten smali has no Java source and therefore no JVM oracle, prepare
   its tests-only independent scalar interpreter here and cross-check it against
   source-origin cases before certification. T05 reuses this prepared oracle;
   B03 MUST NOT wait for a T05 artifact that itself depends on B04. Exception and
   monitor cases MUST NOT use an interpreter that ignores effects; they either
   keep raw-integrity evidence or keep an explicit declined status.
4. The 041 builder for D065 needs both the published format constraint and an
   independent projection check; do not take a sample from a developer's personal
   device. If a legal construction cannot be proven, preparation is blocked; do
   not substitute an ordinary DEX and claim 041 coverage.
5. Fix the invalid-mutation facts: format out-of-range, unknown width and undefined
   wide each prove themselves, and assembler exit 0 MUST NOT be inferred as proof
   of all legality.
6. Rebuild the reference artifacts from a fresh checkout and a fresh cache, and
   record the supported platforms and tool versions. An unexecuted platform MUST
   NOT be claimed as passing.

**Invariants / limits**: Reference inputs and expected values are frozen before
the engine implementation. Any change needs a case v2 reason, an independent
oracle and a review; do not use an automatic "refresh golden". When generated
bytes differ, explain the compiler recipe difference first and do not
change expected semantics.

**Tests**: `verify-inputs` covers every required case/invocation asset and
`inputRequirements`. Record JVM, handcrafted raw, specification mirror and
missing oracle separately; do not hide ART-not-run as a pass. ART records only
execution evidence on an actually authorized environment. D075/D076 real format
boundaries, D077/D078 Java-side compilation acceptance and the D086
default-unlimited versus explicit-limit contrast still need certification; a
local same-kind javac probe is neither locked-toolchain evidence nor engine
execution evidence.

**Acceptance**: The source, input structure and independent expected values of
every required case/invocation are reproducible, and the input and scope
certificates are complete. No local dependency is unknown. The report gives both
the JSON-derived case and invocation denominators. **Completion**: Development
no longer needs a temporary input change to accommodate a wrong implementation.

**Not in scope**: Generating truth with Diana/dexdec/radguard, private caches,
automatically deleting a failing case, CLI test design.

## B04 — E2E comparator, runner adapter and fault falsification

**Dependencies**: B03. **Phase**: test tooling and integration contract, not
engine implementation.

**Files**: `tests/e2e/runner/`, expected/cases/manifest, the thin public-library
adapter source; `tools/e2e.py verify-harness`; actual engine integration happens
in T05.

**Input→output / Interface**: fixed cases, inputs and expectations → a defined
run-result schema, compared by original identity and behavior. Without Diana the
harness can verify preparation and the comparator, but MUST NOT report engine
E2E as passing.

**Steps**:
1. Each case input publishes the Engine request, RunContext and catalog source;
   project the result to the method language, original identity, source and
   diagnostics. The adapter stays thin, MUST NOT inspect private phases, and MUST
   NOT implement a fake engine to replace a missing implementation.
2. Compile the Java result against the same self-made dependencies and known
   context, and compare return bits, u16 values, events, exception and heap through
   the Driver. Fail immediately on a missing class, error, timeout or empty output;
   do not grep javac errors away.
3. Parse the actual debug source snapshot and compare it with independently decoded original facts. Lost instructions/register words, branches, payloads, handler priority, UTF16 units or references fail. No standard-Smali syntax, complete assembly shell or output reassembly is required.
4. Injected result mutations verify the verifier only: wrong return, missing event,
   swapped catch, changed float 0, lossy surrogate, truncated raw, wrong error
   kind, missing invocation, missing artifact, timeout and 0 cases. `verify-harness`
   MUST reject each item; D069 `TargetExists` MUST NOT match a CRC or budget error,
   and D070 empty output MUST NOT match success. A correct reference is acceptable.
   Implementing these tool self-tests still needs separate authorization.
5. Fix the `expected_java` and `expected_debug` classification; do not dynamically
   filter required members by the engine's current capabilities. An implementation
   milestone selects a fixed ID group explicitly, while the whole core suite
   always reports the full required denominator as not passing.
6. The integration failure report keeps the original case input, version, actual,
   expected and artifacts in the checkout cache under relative paths; do not write
   machine paths into a golden.

**Invariants / limits**: Program event sequences are strict; independent report
order is unconstrained. Source and local names have no golden. The comparator
MUST NOT change expected values automatically. CLI black-box tests are not
required; a tool invocation is not the CLI under test.

**Tests**: The composite wrong results above prove that the harness rejects
wrong output; a trivial `assert caseid == caseid` is not enough. BudgetExceeded and
run timeouts are bounded, and a reference Java compile failure MUST NOT be
absorbed as engine "unsupported".

**Acceptance**: `verify-harness` rejects every negative and accepts the expected
correct reference; the suite lock and paths are reproducible and an
unimplemented engine reports not-implemented. **Completion**: T01/H01 start only
after B01–B04 preparation acceptance passes explicitly, and T05 connects only to
the real library.

**Not in scope**: Production stubs, basic JSON/CLI argument tests, automatic
test-golden updates, removing cases to keep version support.

### Freeze and revision rules

After B01 approval, the revision freezes the **problems, input identity, valid
behavior expectations, oracles and required members**; it does not freeze counts,
faulty formats, private phase interfaces or output variable names. A revision MUST
preserve the original problem ID and reason and MUST supply independent evidence
and review. It MAY strengthen inputs or observations and MAY correct a genuine
oracle error. It MUST NOT weaken semantics through a format migration, delete
required members or turn unimplemented Java into raw. A bugfix or refactor MAY
change the thin adapter, tool paths and logs, but MUST NOT refresh truth from
actual output.

Test changes are allowed with a reason; "tests can never be fixed" MUST NOT harden
a wrong oracle:

| Change | Required independent evidence | Handling |
|---|---|---|
| New real counterexample | Self-made input and worked expected value | Add a new case ID; do not delete the old case |
| Specification or oracle is genuinely wrong | Re-verification against the original source, the JVM, the DEX specification or an independent decoder; never engine output | Independent review, case revision, keep the old reason with the new expected value; no refresh hidden in an implementation PR |
| Tool version or fixture rebuild | New artifact version/path, recipe difference, input semantic projection and reference result | Independent tool-lock change; MUST NOT hide a regression or delete a critical opcode |
| Public interface implementation change | A thin adapter update where the problem and expected values stay unchanged | Change only the adapter; do not bulk-edit cases to follow private phase structures |
| Not-yet-implemented Java feature | No evidence permits changing expected values | The case still fails or reports not-implemented; do not switch it to a Debug success or exclude it |

Core Java, negative raw, invalid input and the safety/domain suites are reported
separately. During implementation, each task card turns a fixed case group green
step by step. A fully red core suite is normal, but red MUST NOT be counted as a
skip pass. **Preparation complete is not Diana passing; designed cases are not
inputs-certified.**

## 4. Verification rules

All public calls share one RunContext: open → decompile → render. Options MUST NOT
store Limits a second time, and Report holds only owned result facts. Sources and
expected values are independent of Diana; F/Q labels are historical capability
tags. Compute the actual execution set and both denominators from the JSON
`required`, `acceptanceGate` and `invocations` fields; the current specification
holds 86 cases and 177 invocation scenarios. The accepted preparation artifacts
are recorded in §3; current engine evidence is recorded in task progress and
batch acceptance records.

### Case format, revision basis and obligation mapping

In schema 2 each invocation carries its own id, input and expected value, and a case
carries `acceptanceGate`, `designRefs`, `inputRequirements` and symbolic `targets`.
draft-7 maps each used expected field through `assertionSources` and records a
case-specific `mutationObservation`. These records are in the sole case
specification, not an adapter inference from expected values. The specification
MUST NOT pack several inputs into one any-error and MUST NOT count a case as passing
when an invocation is missing. Resolve scope and the complete proto from the
independent manifest; the adapter MUST NOT pick targets from expected values.
Expected fields are the semantic projection of public results and independent
artifacts, not a requirement that production copy the JSON fields. An unknown field
mapping blocks preparation.

draft-2 preserves the problem identity of D001–D070 and corrects the following
specifications. draft-3 appends D085 to correct default decompilation display.
draft-4 explains the acceptance notation, unifies the application baseline checks
as G0, separates the normal/raw/report boundary from the final JSON output, adds the
report total-overrun case to D064 and adds compression-method and encryption
rejection to D069. draft-5 releases unmeasured numeric budgets as the user
requested: Limits and application quotas default to None, and the Java rejection
predicted from node cost is removed. D077/D078 become required Java, D086 is appended
to check the default-unlimited and explicit-limit contrast, and other valid
expectations are preserved. Two same-kind self-made samples on this machine under
javac 21.0.12.1 give `code_length=30001` and `pool_count=2206`; this is independent
revision evidence, not Temurin 21.0.8+9 locked-toolchain certification, B01 approval
or E2E passing. No engine actual output exists:
- D040: DEX `access_flags` states the managed declared flag and does not add a lock
  on its own; the lock behavior and raw-flags obligation stay.
- D027/D046: a tool may change the shape of or omit a short literal or default
  encoded value. Fix the original new-array/fill shape and the explicit encoded tag
  respectively, and supply independent specification-mirror or field-binding
  evidence.
- D049/D050: exclude a coincidentally equal wrong result through a different outer
  state and a non-lexicographic enum constant respectively.
- D055/D066: both count faults MUST run separately; the definition query includes a
  real constructor and excludes referenced-only members.
- D068–D070: fix entry identity, force and the concrete error category. A success
  scenario MUST really produce a complete file; an absent file MUST NOT be presented
  as "no out-of-bounds write" success, and `TargetExists` MUST NOT stand in for a CRC
  or budget error.
The specification entries are listed in SOURCE_MAP R10. The JSON `revisionReason`
preserves the original defect and the revision reason and does not copy expected
values.

| New case | Actual obligation | Producer / rollup |
|---|---|---|
| D071–D072 | Legal return/call through a Phi from a narrow producer, and int arithmetic/mixed ranges not truncated wrongly | T10 type evidence, T11 existing call binding; T25 whole case |
| D073–D074 | null semantics of count 0, and the FreshArray accept/reject boundary of a non-empty fill | T04/T15; T25 |
| D075–D076 | Real format boundaries of parameter words and code-free declarations, and of MUTF-8 | T10/T21/P8; T25 |
| D077–D078 | A long call sequence and a complete interface declaration MUST be Java, without rejection by estimated cost | T11/T21/P8; T25 |
| D079–D080 | static final still keeps sget/class-init in a multi-unit compile; external constant and Unknown rejection | T11/T20; T25 |
| D081–D082 | Dynamic lock mismatch at one static site is rejected, and a legal loop over a parameter lock is accepted | T19; T25 |
| D083 | Extreme signed sparse keys with bounded handling | T14; T25 |
| D084 | A non-canonical synthetic bridge keeps raw/effect and is not labeled Generated | T21; T25 |
| D085 | When a local invoke-custom fails, Java/Debug, UTF-16, original identity and spans are all preserved, and default text does not hide Mixed | T03/T04/P9, H05; T25/H08 own their seams |

T01/T03/T04 supply the D086 input and resource contrast between default-unlimited,
explicit `Some(n)` and complete raw; T25 rolls it up. D085 verifies complex result
preservation and raw/UTF-16/span integrity through a public module without starting
the CLI, and adds no test for a format flag, a status getter or output formatting. A
default-display correction MUST NOT turn an existing positive Java case into a Debug
success.

This mapping MUST NOT automatically classify new cases as Java positive. The
support or rejection result of each invocation comes only from the JSON. Method,
declaration and pool boundary checks are core semantics, not CLI arguments, getters
or tool self-tests. A new specification or specification derivation does not replace
B01 approval, B03 input and compiler-artifact certification, or explicit bounded
authorization for B04 tool self-tests.

Preflight clarification of draft-5 preserves every case/invocation and every
expected value. It makes D059's unknown-opcode mutation and D062/D063 target
recipes concrete, replaces D041's private-SSA wording with original-input/source
observations, and makes D079's two reads share one fresh JVM per invocation.
The batch record identifies the reviewed bytes and source/JVM evidence. These
clarifications are not B01 approval or B03 input certification.

### 4.1 Validation layers

| Layer / ID | What it checks | Evidence | What it cannot prove |
|---|---|---|---|
| V0 InputSafety | offset/count/LEB/alignment/ID/word/payload/UTF-16/handler/depth | Small self-made binaries, targeted malformed mutations, budget/cancellation, no-panic/no-unbounded-allocation tests | Safety for arbitrary input, completeness of all DEX verifier rules |
| V1 FlowSSA | raw instruction consumption, normal/exception edge, parameter words, definition/dominance/Phi/old state | Public seam profile cases plus independent artifact-verifier mutations; recorded edge/env expectations | Neither an acyclic graph nor SSA names prove all semantics |
| V2 Storage | parallel copy, critical edge, loop latch, pre-throw snapshot, unobservable default values | CopyPlan re-execution tests, definition availability, exact type/no-extra-check, negative cycle/order cases | one-local-per-value does not automatically give legal Phi elimination |
| V3 Structure | op/edge coverage, transfer target, try/handler order, monitor release | Independent verification of Structure/Protection/Monitor certificates; matching behavior fixtures | Fewer labels/gotos/states does not prove correctness; a visited set is not enough |
| V4 JavaLegality | method/class context, member binding, primitive/ref/null, checked throws, constructor/final/nested/enum/annotation | P8 validator plus **a real javac exit=0 on the complete classpath** | Compilable Java does not prove equivalence; ignoring a missing-dependency error is not a pass |
| V5 Behavior | return, heap/call log, exception and cleanup, loops, locks, initialization, string/FP bits | Isolated-run differential between self-made baseline source and recovered Java; authorized ART environment when required | Implementation-dependent behavior where JVM≠ART, uncovered paths, all inputs |
| V6 DebugInformation | Available original code/declaration facts and honest coverage after Java failure | Public debug snapshot and actual rendered source slice versus independent original-input projection | Debug information is neither verified Java nor reassemblable Smali; a byte excerpt does not imply decoded coverage |
| V7 System | status denominators, budget/trace/catalog consistency, actual source spans, unordered-output completeness, license, performance | A report with no empty run, identity-linked records and evidence, several arrival orders, document/schema checks | Cross-run text or record arrangement is not required to match; a release success or multithreading does not prove quality |

Each capability has at least one positive case, one counterexample that can break its
core invariant and one limit/cancel case; the counterexample MUST verify the correct
classification, not merely "returns some error". V1/V2 MUST reject a default
initializer that turns an undefined read into 0/null.

### 4.2 States and denominators

#### 4.2.1 The report counts per method

EngineCoreRC and ProductRC select the required set from §2 and the JSON
`acceptanceGate`; every invocation and every seam of a shared case needs evidence.
An unexecuted application check MUST NOT count as an engine pass, and the engine
denominator MUST NOT become the product denominator. D065/D085 keep evidence for both
seams separately and each still counts as one case; a completely displayed Mixed
result does not count as a JavaUnit.

The report counts the following mutually exclusive states and outputs the
denominator at the same time:

- `Java` executable methods (with code, P8-verified) and their behavior-tested
  subset.
- `DeclarationOnly` (abstract/native, no body) — MUST NOT raise the executable Java
  recovery rate.
- A verified `Generated` bridge/enum/inner member counts as Java executable and
  records the owner generation rule; the report MUST NOT print a conflicting
  declaration twice or count it as DeclarationOnly. A full unit MUST generate the
  expected behavior from that owner syntax; a get-method that cannot be expressed on
  its own still yields the original Debug and gets no invented wrapper.
- `Debug` (checked original facts with explicit complete/partial coverage); it is
  neither executable source nor a reassembly promise, and remains distinct from
  `Unavailable` with only a safe excerpt or no recoverable information.
- `Unavailable` (damaged/unknown width/no reliable fallback/resource failure), with
  the diagnostic reason Malformed, Unsupported, BudgetExceeded or InternalInvariant.
- `JavaUnit` and `Mixed` class counts; a single inner fragment, missing shell
  metadata or an arbitrary fallback is not a complete JavaUnit.
- RequestError and Cancelled are listed separately; a BUG MUST be a red gate signal
  even when raw output is produced.
- Per-reason counts for missing catalog, known unsupported instruction, exception
  profile, monitor profile and constructor profile.

For example, report `Java=12/20 executable, DeclarationOnly=3, Debug=5,
Unavailable=3; JavaUnit=1/4; BehaviorVerified=10/12 in env X` rather than
"17/20 can be emitted, so 85% succeeded". A Java fragment that cannot really compile
MUST NOT count as a V4 pass even when P8 succeeds.

#### 4.2.2 Frozen-baseline discipline

Each baseline records the engine commit, the options/profile/schema, the input
identity and recipe, the catalog identity of that request, the toolchain versions, the run
command, the per-identity method status/trace and the tested input set. It does not
freeze query or cross-method result sequences, and does not require local names or
generated source to match across runs. Text samples serve diagnostics only, and
source spans are checked item by item against this run's real output. A complex
query result is checked through a public module against the original-identity
multiset; the CLI offers no pagination or count-truncation argument. Removing an old
flag needs only a source and document check, not a new CLI test. An insufficient
safety budget fails explicitly through a domain interface; it MUST NOT succeed for
the first N entries. A behavior change still needs independent review.

Output from dexdec, radguard and DAD serves triage only: equal output may be equally
wrong, and different output may both be equivalent forms. A lenient normalizer MUST
NOT be frozen and then declared truth, and a comparison-rule change and an engine
change MUST be recorded separately. This design copies no RG pickle or DEX fixture,
runs no unauthorized cache, and lets no fixture name reveal a real corpus identity.

### 4.3 Behavior Driver

#### 4.3.1 Driver output structure

This project does not guarantee a stable order for analysis results, but **the
execution trace of the analyzed program is not a display order**. The Driver MUST
preserve the event seq, argument evaluation order, exception/catch, cleanup, lock and
initialization order of the program item by item; it MUST NOT sort by event kind or
compare them as a multiset. Only independent method and query reports are linked by
original identity with display order ignored.

The harness compares the JSON structure of the specified fields and the semantic
event sequence, not printed Java text or JSON key arrangement. Each case runs the
same public entry of the original self-made source and of the recovered source
separately, with the same Driver/Logger and catalog versions.

```json
{
  "case": "D029",
  "result": {"kind": "int", "value": 7},
  "events": [
    {"seq": 0, "kind": "call", "label": "throwing-producer"},
    {"seq": 1, "kind": "catch", "type": "fixtures.ExpectedError", "old": 7}
  ],
  "exception": null,
  "heap": {"counter": 1},
  "completed": true
}
```

Type encoding: int/long as decimal or fixed bits; float/double as raw bits with an
explicit NaN policy (whether canonical NaN suffices depends on the case); strings as
a `u16` code-unit array; null differs from an empty String; objects use only
self-made identity tokens and visible fields, never an address or the default
`toString`.

Logs exist only in self-made dependencies and record observations such as calls,
field accessors, cleanup and locks. The harness MUST NOT add a stub to the recovered
program that changes original behavior. Additional Driver logging can itself have an
effect, so a fixture MUST avoid a logging change that alters the profile the
algorithm can express; source and recovered code use the same dependencies, but that
is still evidence for the given fixture only.

Exception comparison covers type, visible cause or self-made message (no runtime
address), the event prefix at the occurrence, catch selection and cleanup behavior.
The harness MUST NOT simply compare top-level success or failure, because a wrong
catch may also return -1. Stack-trace line numbers are not a success requirement;
the error type and handler MUST NOT be ignored.

#### 4.3.2 Round-trip testing and the evidence limits of original DEX

The basic loop is self-made Java → javac/d8-generated DEX → Diana → Java → javac →
JVM Driver. The baseline is the self-made source and the JVM that generated the DEX,
which relies on d8 compiling semantics that match the target. This tests the
recovery logic, but **it is not a direct comparison of the original DEX and the
recovered DEX executed on ART**.

The stronger layer runs the original DEX and the DEX compiled from recovered Java by
d8 through the same Driver and compares events, once the user explicitly provides an
authorized Android test environment. Production still depends on no external
runtime. Without that environment, V5 reports `JVM-only, ART-not-run` and lists the
implementation-dependent parts such as volatile, monitor, FP and constructor
allocation as risks instead of claiming full ART equivalence.

For an irreducible CFG built by hand-written smali or a raw case that Java cannot
directly produce:

- A normal op-graph or copy test MAY use a **tests-only reference interpreter** that
  supports only self-made pure scalar ops and is compared against real Java execution.
  The interpreter has no production path and does not become a DEX runtime.
- Exception and monitor cases MUST NOT use a simplified interpreter that ignores
  exceptions or locks as an oracle. Without an authorized ART environment, a
  hand-made protected graph yields only V0/V1/V2/V3/V6 evidence, and its Java profile
  stays experimental with the corresponding V5 contract incomplete.
- Text emitted by dexdec or radguard MUST NOT serve as truth for a self-made case. The
  baseline is the specific return and trace in the case design and the independent
  semantic derivation; a reference engine only adds debugging information.

`behavior_roundtrip` MUST distinguish a `source-origin JVM case`, a `handwritten
smali + scalar reference interpreter case` and an `ART case`. The last is not-run or
fails as required when no applicable execution oracle exists; do not substitute
source for "a simpler but different CFG". B03 prepares the tests-only restricted
pure-ops interpreter and T05 integrates it without rebuilding the oracle. It has
no heap, exception or monitor support, and its positives need cross-checking
against the JVM before input certification; it MUST NOT
count as a complete DEX VM.

The specification mirrors of D027/D073 are limited to the closed shape the JSON
specifies. D027 covers the single new array, a known sufficient length, constant
elements and no escape or handler, and proves the bulk fill and the in-place Java
stores by FreshArray heap and transfer. D073 covers a known primitive component,
count 0 and no handler or other effect, and proves the same normal and exception
transfer of the fill and the length read per state. The certificate lists the
original DEX and mirror CFG/site correspondence, the allocation and exception
conditions, and the derivation, and then compiles an independent Java mirror. It
MUST NOT call the mirror execution of the original DEX, and MUST NOT generalize it to
an exception or lock VM. A missing independent derivation blocks B03; a hand-made
complex protected or monitor graph still obeys the ART evidence boundary above.

A full-unit-set (D079) only invokes the existing public Engine class request for the
fixed roles in turn and then compiles every complete JavaUnit together. It adds no
production batch command, keeps no original target classfile that hides a recovery
defect, and changes no unselected dependency to build a stub. If any unit or
invocation is unacceptable, that invocation fails.

#### 4.3.3 Timeouts, termination and concurrency

Every subprocess start has a deadline; a timeout kills the process and records a
non-pass, and a cumulative output cap prevents log explosion. An unbounded-loop
fixture proves only some finite prefix; after a timeout the harness MUST NOT call
the expected run complete. Loop tests use bounded counters and exercise 0, 1 and
several exits separately.

Monitor tests first cover `Thread.holdsLock` on the same thread, a null-enter
exception, lock release after every exit, and reentrancy and nesting on the same
object. A two-thread handoff check with a deadline MAY be added, but a successful
stress run is not a proof of the complete memory model. A CFG certificate MUST
supplement every lock release; "the body still runs" is not evidence of preserved
synchronization semantics.

#### 4.3.4 Process observation of file publication

D070 invokes each unsafe target and force combination separately; a default no-force
`TargetExists` MUST NOT verify the alias and symlink safety of force.
`originalFilesUnchanged` checks the original input path, the original handle and a
sentinel at the same time; it MUST NOT compare only a frozen snapshot and miss a
replaced original path. The two publishers use different deterministic payloads, and
the final file MUST equal the actually successful payload byte for byte; the other
publisher MUST observe `TargetExists`. Two publishers MUST NOT both report success
while only one payload is visible.

The final-path observer is ready before the publisher starts: the public cancellation
callback always returns false and only observes the final path, supplemented by a
concurrent reader with a deadline. The test MUST NOT inject a private publish hook or
match phases or callback counts. Each read opens one handle and checks the complete
content through that handle, so content from before and after two renames is never
concatenated. no-force permits an absent file or one complete payload; force permits
the old KEEP result or a complete new payload, and empty, prefix or mixed content
fails. At the end the test observes the complete new file at least once and checks
that all temp files are cleaned; an absent file MUST NOT pass vacuously. The bounded
write and copy loops check cancellation at most every 64 KiB, so a mutation that
writes a temp file as the final file in chunks exposes a half file through the public
callback; the callback observation itself is read-only and never blocks waiting for
the producer's next stage.

Finite reader sampling does not prove all OS scheduling: atomicity also depends on the
hard_link/rename primitive of ALGORITHMS §14.3 and on the file-safety review that
holds the Dir, and a passing stress run MUST NOT replace it. When the platform does
not support these primitives, preparation fails honestly; the test MUST NOT degrade
to exists+rename or drop that required invocation.

#### 4.3.5 Assertion projection and run-result contract

B04 compares observations, not a mirror of the production CLI JSON. Its thin
adapter records `(case_id, invocation_id, seam, spec_revision)`, original selected
identities, `execution` (`completed`, `not_implemented`, `blocked`, `timeout` or
`failed`), projected public method/class outcomes and typed diagnostics, and
relative paths to actual artifacts/observations. Only `completed` with every
required assertion satisfied passes; other states retain their reason and never
count as successful skips. This result schema is tests-only and does not enlarge
Engine::Report or the CLI command surface.

| Expected assertion family | Independent actual source and comparison |
|---|---|
| `outcome`, Java/Debug inclusion, `hasBody`, `noJava`, `noFakeJavaUnit`, `noFallbackReport` | Public Report/RequestError and complete original-member set; Java capability requires the fixed validationScope, and resource/cancel/BUG cannot become Unsupported success |
| `reason`, `unsupportedReason`, `errorKind`, `otherErrorKind`, `limit`, `targetItem` | Typed failure kind/code and configured failing item; snake_case projects public domain reasons, while named Unsupported reasons retain their specific code |
| Returns, FP bits, string units, events, exception, heap, monitor and enum/annotation observations | Same isolated Driver and fixed entry/setup on baseline and recovered code; exact program order and type-directed bits/u16 comparison under §4.3.1 |
| `debugFactsExact` and declaration/dynamic/bridge/payload/handler/ref/string integrity | Actual original-information snapshot versus the certified input projection; preserve original PCs, opcodes/registers, ordered targets/effects/handlers, payload bits, references and u16 values. Parse the actual rendered snapshot independently; no assembly or text-layout golden |
| Original members, Generated, final fields/proto, `orderedInvokeSites`, `declarationsPreserved`, `constantValueAbsent` | Original certified declaration/code projection plus actual compiled output/reflection where required; inspect all selected members, not only ones that compiled |
| `sourceSpansExact`, `textAvailable`, `failedMethodHasNoJavaBody` | Real rendered UTF8 and original-key spans; bounds and exact member/raw slice extraction, with relocated Mixed prefixes; no private AST or language-getter substitute |
| Defined methods, absent refs, referencing method/site count and `allCertifiedSites` | Public query's original-key multiset versus certified definitions or direct operand sites; independent row order ignored, duplicate/missing sites fail |
| File/payload bytes, targets, temporary files, escaping/explicit-path and original-file safety | Actual filesystem handles and public extract results under §4.3.4; compare full bytes, final state and permitted observations, not a success flag or a snapshot of expected |

Before B01 approval, associate every used expected field with one comparison
above through the specification's assertionSources and its concrete case
observation. The source group identifies the producer, not a boolean result;
B02/B03 materialize the actual observation artifacts in the manifest. A field with
no observation/mapping blocks preparation; do not ignore it, infer its actual
value from expected/input, or add a production boolean to make it pass. Compound
assertions require their whole predicate, for example all original members
exactly once and a complete raw slice for each required raw member.

The comparator first verifies the exact required invocation/seam set and input
certificate, then source eligibility and hard-failure classification, then the
case's independent observations. Results link by original identity; program
traces stay ordered. Expected object fields are all mandatory; specified scalar
and array values compare exactly under their encoding, while unasserted runtime
metadata is not a text golden. Null, absence, zero and empty collections are
not interchangeable. Within one invocation/seam, duplicate selected member or
operand-site identities fail; the same member in another required invocation,
Driver repeat or ordered call is not a duplicate analysis result.
`mustNotInclude` checks the complete multiset; `observationsOnly` admits every
actual observation only from its whitelist and does not invent a sampling count.
Compact string/file patterns expand to full exact values under the JSON policy.

`inputRequirements` remains case-authored semantic predicates, not a second
production format. The manifest binds each predicate by its index and spec revision and distinguishes
`input_certification` from `result_validation`. B03 certifies the original-input
and reference predicates; B04 wires recovered-result predicates for real T/H
execution. A predicate containing a recovered-source obligation does not require
a nonexistent engine output before B03 can pass, and remains mandatory at its
own validation stage. `mustRejectMutation`
is a behavior obligation, not necessarily a byte edit: result mutations belong
to B04's already-listed falsification, while an invalid-input recipe records
its original path, named item/field or opcode anchor, changed bytes and independent
post-edit facts. A bare unchecked offset is not a mutation certificate.

B03 owns preparation of the scalar oracle that T05 integrates, with only the
pure families actually required
by the handwritten normal-control cases: entry arguments/constants, word-correct
moves, int/long arithmetic/bitwise/shifts/conversions, comparisons, branches,
integer switch and return. Div/rem is admitted only with a non-zero divisor in
the certified execution; it does not add an exception oracle. Its closed supported-op set and unsupported-op error
are recorded with the interpreter; no heap/call/exception/monitor or native FP
shortcut is accepted as a scalar oracle. It executes independently decoded raw
instructions/registers, not Diana SSA/copies. Supported operations and branch/loop
results are cross-checked against source-origin JVM cases before they certify a
handwritten input. A missing operation or execution oracle blocks that evidence.

### 4.4 Capability family index (the execution truth is the JSON required set)

**The execution specification is the required cases and invocations in
[e2e_cases.json](e2e_cases.json); B01–B04 review, prepare and certify the complete
inputs, expected values and oracles first.** The following F00–F25 and U/M items are
only a capability-family and counterexample naming map, not an independent execution
list; they do not authorize an implementation agent to add expected values or delete
a case based on current capability.

Every F/U/M fixture is self-made. A binary method satisfies the compiler or the
smali assembler rules; a dedicated invalid case exists for rejection, and no
requirement says a malformed input can also produce Java. Non-executable native or
abstract declarations get their own count.

| Fixture | Core input/path | Expected success/counterexample | Owning task / verification |
|---|---|---|---|
| F00 ResourceControl | Budget isolation, failure release and cancellation stop of a public request | Real boundaries and overrun, fallback does not bypass the budget; basic enums, wrappers and private call order are not tested | T01/T05; V7 |
| F01 TinyDex | Self-made small header/catalog/exact proto/abstract | Exact method key and raw ranges; M01/M02 out-of-range, LEB and count failures | T02; V0 |
| F02 RawMethods | move/return/wide/range/branch formats | Original DEX facts remain analyzable when Java is unsupported; M03 unknown width keeps explicit original bytes without guessed instructions | T03/T05; V0/V6 |
| F03 Utf16Payload | NUL, quote, backslash-u, isolated surrogate, switch/fill, debug/encoded | u16/bits/payload target preserved; M04 truncated/overflow/metadata rejection | T04/T05; V0/V6 |
| F04 HarnessSelfTest | Missing tool, wrong size, 0 selected, syntax or behavior failure | The harness itself MUST be red and contributes no Java result; separate §3 authorization remains required | B02/B04, reused in T05; V7 |
| F05 ScalarReturn | answer 7, param 41+1, int wrap MAX+1 | The target method returns correctly with correct locals; early runs may use method-in-known-context with the class honestly marked Mixed; M05 undefined/bad return/instruction/cancel rejection | T06; V0/V1/V2/V4/V5 |
| F06 DiamondPhi | `choose` with two branches, nested, same target on different edges | One evaluation of the condition; M06 missing Phi/edge/no-dominance and M07 wrong-branch copy rejection | T07/T08; V1/V2/V4/V5 |
| F07 CopyCycleLoop | a/b swap, 3-cycle, 0/1/N latch, critical edge | Simultaneous assignment and loop Phi; M07 naive sequence/temp/observed-default rejection | T08/T09; V1/V2/V5 |
| F08 DispatchGraph | Ordinary two-entry cycle, multiple exits, throw terminal | Dispatcher returns and logs agree; M08 lost/duplicate/default/eligibility failures | T09; V3/V4/V5 (with the smali oracle limitation) |
| F09 PrimitiveBits | boolean/int/ref 0, wide, shift, FP NaN/-0/cmp bias/conversion | Core bits and exceptions agree without library calls; a bitcast subcase without P4 evidence stays Unsupported in this card; M09 partial-wide/unresolved-type rejection | T10; V1/V4/V5 |
| F10 CallKinds | static/private/super/virtual/interface, overload/null, effect args | Same callee, argument order and call result; M10 bad result/kind/proto/visibility | T11; V1/V4/V5 |
| F11 MissingCatalog | F10 legal raw with empty/partial external facts | Debug plus BindingEvidenceMissing; Java with a complete catalog; public facts MUST NOT be invented | T11; V6/V7 |
| F12 AdjacentConstruction | Args ready, new/init/pure alias, super/this chain | Construction trace identical; U02 complex pre-super/may-throw/lock rejection | T12; V1/V4/V5/V6 |
| F13 NaturalControl | Ordinary if/while, nested loop, break/continue | Both the natural and dispatch adapter agree; M11 duplicate region/lost edge | T13; V3/V4/V5 |
| F14 IntegerSwitch | packed/sparse, negative/extreme/shared/fall/default | Selector evaluated once, copy edge exact; M12 payload/key/bounds | T14; V0/V3/V5 |
| F15 HeapEffects | static init/field/array/null/bounds/volatile/unused cast | Effect and exception prefix complete; M13 field kind/payload/missing catalog | T15; V0/V1/V4/V5 |
| F16 ThrowState | old=7; a result-producing call throws; catch reads old | Catch 7 is not the normal new result; M14 normal state/order/move-exception faults | T16–T18; V1/V2/V3/V5 |
| F17 HandlerPhi | Two sites with different old values, loop protected, shared target | Snapshots precede the throw; M15 post-call/Undefined/cast/alias not allowed | T17/T18; V2/V3/V5 |
| F18 ProtectedControl | Subtype order, catch-all/rethrow, cleanup return/throw, cleanup throwing itself, one nested level | Same handler/domain/trace; U03 complex reentry/ownership degrades; M16 expanded try and similar failures | T18; V3/V4/V5/V6 |
| F19 MonitorScope | holdsLock/return/throw/null, nested/reentrant, static/instance | All releases and same identity; U04 mismatch/missing/multi-entry degrade | T19; V3/V5/V6 |
| F20 Initialization | encoded null/false/bits, non-constant static, this delegate, blank final, init exception | Initialization order in a new process; U05 unrepresentable final → Mixed | T20; V4/V5/V7 |
| F21 ErasedContracts | Self-made generic/raw interface/bridge/covariant/null overload | Raw warnings acceptable, member binding unchanged; U06 return-only/unknown/bridge effect rejection | T21; V4/V5/V6 |
| F22 NamedNested | static/instance outer capture, private access, name conflict/Unicode | Legal unit and span mapping; U07 cycle/missing/ambiguous ownership | T22; V0/V4/V5/V7 |
| F23 EnumAnnotation | Canonical enum name/ordinal/values clone/valueOf, runtime annotation | No `extends Enum`, reflect metadata agrees; U08 custom body/annotation unrepresentable → Mixed | T23; V4/V5/V6 |
| F24 PureRecovery | On/off single-use pure copy; call/read/div/lock negative | Same behavior, the option really consumed; effects MUST NOT move | T24 optional; V3/V5/V7 |
| F25 HardeningMatrix | All M/U, small budget, cancel, original semantics/cancellation/budget/spans, known 038/039 raw extension | Baseline support and status agree, no panic and no 0 tests, BUG gate red | T25; V0–V7 |

For the hand-written smali part of F07/F08 the manifest MUST state the range that the
reference interpreter, ART or the JVM actually proves. If the F17 shared handler
lacks an ownership profile, it becomes a U03-style degraded case; its Java
unavailability does not negate T18 and it is not listed as positive Java support for
T18.

#### Required-case producer links

This table links the JSON problems to owning producers and algorithm sections;
it is not another suite, progress counter or copy of expected values. Batch
records name concrete D IDs/invocations before implementation; no selector is
inferred from current support or a chapter-string hit count. Related cards share
producer obligations rather than duplicating a case in the denominator.

| D cases | Owning producers / supporting integration | Algorithm section |
|---|---|---|
| D001–D003 | T07/T08/T09 | §3, §5, §6 |
| D004–D008 | T08/T09/T13/T14 | §5, §6 |
| D009 | T06 | §4 |
| D010–D017 | T10 | §4 |
| D018–D023 | T11 | §9 |
| D024–D028 | T15, T04 payload input | §2.4, §4, §9 |
| D029–D035 | T16/T17/T18 | §3, §5, §10 |
| D036–D040 | T19 | §11 |
| D041–D043 | T12/T20 | §9.2–§9.3, §12 |
| D044–D047 | T20 | §12.2 |
| D048–D051 | T21/T22/T23 | §12.1, §12.3 |
| D052–D054 | T04/T10, P9 literal printing | §1.1, §4.8 |
| D055–D058 | T02/T03/T04/T07 input rejection | §1, §3 |
| D059–D061 | T03/T04/T05, T21/T22 raw shell | §7–§8, §12 |
| D062–D064 | T01/T05, H05 delivery | §8, §13; DESIGN §3.7/§7/§10.3 |
| D065 | T04, H04 | §1.1, §14.2 |
| D066–D067 | H03/H09 | §14.2 |
| D068–D070 | H02/H10 | §14.3 |
| D071–D072 | T10/T11 | §4.9, §5, §9 |
| D073–D074 | T04/T15 | §2.4, §8 |
| D075–D076 | T10/T21, T03/T04 raw | §4.8, §7, §12.4 |
| D077–D078 | T11/T21 | §12.4–§12.6 |
| D079–D080 | T11/T20 | §9.1, §12.2 |
| D081–D082 | T19 | §6, §11 |
| D083 | T14 | §6, §12.5 |
| D084 | T21 | §9, §12.3 |
| D085 | T03/T04/P9, H05 | §8.1; DESIGN §2.2/§6.5 |
| D086 | T01/T03/T04 | §1, §7–§8, §13; DESIGN §3.7/§10.3 |

T25 rolls up all engine obligations and H08 all product obligations according to
JSON acceptanceGate; shared D065/D085 need both seams. D041 is a constructor
problem, not a 041 container fixture. D065 already certifies multiple logical
members and sharing; a header-chain or cross-container-range counterexample
needs an independently reviewed appended case, not an unrecorded change of D065.
AXML checks belong to H03's core manifest obligations, not an invented D-case pass.

### 4.5 Pinned tools and dependency locks

Validate the candidate tool combination before engine implementation, because it
generates self-made fixtures and runs the baseline. The production binary depends on
no Java, Maven or Python.

| Tool | Pinned version and public source | Use and verification |
|---|---|---|
| Rust | Root manifest minimum `1.93`; fix the patch version when the runner is implemented | Ordinary native build; the lock file records `rustc -Vv`, and the author MUST NOT hand-edit the existing project toolchain |
| Python | Harness preparer minimum 3.11 (if Python is chosen) | Development tool only; MUST NOT enter the production runtime, and scripts are cross-platform |
| JDK | Eclipse Temurin **21.0.8+9**; recovered Java pinned to `--release 8 -proc:none -g:none`, and the original fixture debug flags locked by the case recipe | [Official release](https://github.com/adoptium/temurin21-binaries/releases/tag/jdk-21.0.8%2B9); select the JDK asset for the OS and arch from the official release; do not download only the JRE |
| R8/D8 | `com.android.tools:r8:8.3.37` | [Google Maven POM](https://dl.google.com/dl/android/maven2/com/android/tools/r8/8.3.37/r8-8.3.37.pom), main class `com.android.tools.r8.D8`; build the argument array from the pinned JAR and do not depend on an Android SDK wrapper script |
| smali / dexlib2 | `org.smali:smali:2.5.2`, `org.smali:dexlib2:2.5.2` | [Maven Central POM](https://repo.maven.apache.org/maven2/org/smali/smali/2.5.2/smali-2.5.2.pom), main class `org.jf.smali.Main`; the POM has transitive dependencies, and **the author MUST NOT assume an arbitrary `smali.jar` runs with `java -jar`** |
| Test dependency classpath | Self-made Java sources or declared JDK entries | All sources are checked in and the classpath comes from the recipe; the core suite depends on no Android platform jar |
| ART verification | A device or image that the user explicitly authorized and that is reproducible | Reported separately from JVM-only runs; without an authorized reproducible environment ART is not a pass and MUST NOT become an implicit local SDK dependency of core prep |

The tool combination MUST actually generate and certify every fixture in B02/B03;
an accessible POM does not make the combination usable. B02 produces
`tests/e2e/toolchain.lock.json` with the full URL, version, size, dependencies
and main class of every required artifact; the JDK needs an asset per target
platform. Missing coordinates, placeholder values or unlocked transitive versions
block preparation. SHA-256 fields are optional informational provenance and MUST
NOT participate in download, cache or acceptance decisions. Path/size checks do
not authenticate contents or detect every same-size edit; independent semantic
comparison remains required.

The author MUST NOT move the R8/smali JVM dependency into Rust production or
`build.rs`. A controlled Maven step MAY produce the classpath, but it MUST lock the
transitive dependencies and artifact versions and then run offline; the lock MUST NOT
list only the two top-level POMs. The checked-in artifact manifest and source recipe of
the core suite let other developers re-verify without an external reference
repository.

#### Production dependency review

Cargo.lock owns exact resolved Rust versions/features; this table does not
pretend to certify an unused crate version. A dependency enters the owning card
only after reviewing its real safe interface, reachable input-driven allocations,
platform behavior and relevant unsafe internals. Active Diana crates still forbid
unsafe; that lint does not prove transitive crates contain none.

| Use / owner | Selection or review obligation |
|---|---|
| Capability-relative ordinary-file access, H02/H10 | `cap-std`; the owning Module MUST use safe open options and same-held-Dir link/rename/remove meeting ALGORITHMS §14.1/§14.3 on each claimed platform |
| File identity / private temp ownership, H02/H10 | `same-file` / `tempfile`; the owning Module MUST use handle-based identity and keep temporary creation/publication relative to the held parent Dir |
| Native deflate / CRC, H02/H03/H10 | Incremental pure-Rust inflate candidate, with `crc32fast` for actual payload CRC; review end-state, bounded caller output, input-driven scratch allocation and cancellation between calls before choosing crate/features |
| Unicode category, T21/P8 | Native category data matching the Java8 compilation profile; review Unicode-version differences against the locked JDK's Java8 mode, not just a Cargo version or ASCII-only fallback |
| JSON, H05 | Native serialization candidate such as `serde`/`serde_json`; ordinary Rust String serialization alone cannot preserve an isolated UTF16 surrogate, so review the application escaping and fallible/output accounting path |
| AXML, H03 | Focused reviewed native reader under ALGORITHMS §14.4; the archived patched reader is evidence only and cannot be a Diana path dependency |

The handwritten ZIP central-directory reader is the scheme in ALGORITHMS §14.1;
production extraction writes an entry's decompressed payload, not a new ZIP.
No ZIP writer is required by that interface. Tooling MAY generate self-made ZIPs
without making that writer a production dependency.

Record the selected version/features, safe calls, allocation/cancellation points,
platform evidence and unresolved limitations in the owning batch. Existing names
or this table are not dependency approval. If a candidate cannot enforce the
contract, stop and review a focused alternative; do not add unsafe to Diana, a
catch-panic wrapper or silent successful truncation. Dependency/tool self-tests
remain separately unauthorized under §3.

### 4.6 Asset layout and generation discipline

The fixed behavior specification lives only in `docs/diana/e2e_cases.json`; this
section maintains the preparation and asset contract. Actual B02 output:

```text
tests/e2e/
  sources/<case-id>/        # self-made Java/smali/driver/dependencies; no machine paths
  expected/<case-id>.json   # derived, read-only view of the approved JSON expectations; never truth
  inputs/                  # small self-made DEX/ZIP/AXML/bad bytes; or a reviewed generation recipe
  manifest.json            # sources, recipe/toolchain lock, artifact paths/sizes, fixed suite members
  toolchain.lock.json      # complete artifacts/versions/transitive dependencies/main classes
  cases.json               # derived execution view (ids, pointers, revision) of the approved JSON
  runner/                  # thin adapter over the public interface; no private phase dependency
```

The execution workspace is `.cache/e2e/`; sources and expected values elsewhere are
never written back automatically. A checked-in binary fixture needs a scoped
allowlist `.gitignore` entry rather than removal of the global DEX/APK ignore rule,
plus a source, provenance and small-size review. A recipe-only check-in is allowed, but prep
MUST verify that each developer's generated result obeys the same semantic
field requirements and MUST NOT assume the author's cache exists.

When compiling or joining inputs can change symbol-pool offsets, the semantic check of
the recipe compares each field. A changed artifact needs an independent fixture
rebuild record; it MUST NOT be treated as "decompilation failed, regenerate a simpler
DEX". Return and event expectations always come from the specification worked example
or an independent baseline, never from copied Diana/dexdec/radguard printed text.

#### Fixture manifest schema 1

The manifest binds the approved case specification to original input assets; it
is not a second expected-value database. B01's `tests/e2e/cases.json` is a derived
execution view containing case/invocation IDs and JSON pointers to targets, input,
expected and assertion-source records in the reviewed specification. It contains
no copied expected values or fabricated successful certificates. Top-level fields are `schema_version:1`,
`spec:{revision,path}`, `toolchain_lock:{path}`, `assets[]` and
`cases[]`. It records no hand-maintained case count. IDs/paths/revisions are strings and
counts/indices are non-negative integers. Optional legacy digest fields are
informational only. All required fields are present; irrelevant collections are empty and only the
explicit alternatives below use null. Paths are checkout-relative POSIX paths,
with generated artifacts under `.cache/e2e/`; implementations resolve them
independently of the working directory.

| Record / fields | Binding contract |
|---|---|
| Asset `id`, `path`, `size` | Unique local name, checkout-relative path and file size; checked-in source or generated output, never a machine path |
| Asset `source_paths`, `license`, `recipe` | Source-path array and license string; recipe object has tool/profile IDs, argument arrays and input/output asset IDs, or an explicit literal/byte-builder description. An original checked-in source has null recipe, not missing provenance |
| Case `id` / `invocations[]` | Exact JSON case and invocation IDs; unknown, duplicate or missing required entries fail preparation |
| Invocation `id`, `input_assets`, `expected_ref` | Asset IDs and JSON pointer into the reviewed approved specification, not copied expected values |
| Invocation `bindings[]` | `{role, selector, origin}`; bind each role named by the input, including implicit `case` when used, with a full class/method/field/string selector or explicit physical entry selection |
| Binding `origin` | `{asset_id, entry_ordinal, logical_ordinal, header_offset}`; entry ordinal is 0 for bare DEX, and logical/header fields are null only for a non-DEX entry/error. No display label or request-local DexId |
| Binding `selector` | Tagged `kind`: class has `class_def_idx,descriptor`; method adds `method_id_idx,name,proto`; field adds `field_id_idx,name,type`; string has `string_id_idx`; entry has `name,ordinal`; query/error binds the explicit request/input without inventing a member |
| Invocation `request`, `catalog`, `driver` | Public seam/setup and explicit limits, declaration fact assets/completeness, Driver entry/args/isolation. Catalog/driver are null only when the seam uses neither; concrete setup refers to the reviewed case invocation, not inferred external SDKs or production factories |
| Invocation `checks[]` | `{requirement_index, stage, evidence_source, projection_path}` for each JSON inputRequirements obligation; stage is `input_certification` or `result_validation`, and one mixed predicate may need both. Original evidence references certified assets; recovered evidence names a public observation under §4.3.5, not a fabricated pre-engine output |
| Invocation `assertions[]` | Expected JSON pointer, observation source and fixed comparison under §4.3.5; no cached actual boolean used as truth |
| Invocation `certificate` | Artifact/projection/reference-evidence IDs, validationScope, actual tool/platform and certification result; missing evidence is explicit |

B01 fixes the reviewed manifest shape, roles and complete symbolic selectors
from the independent case recipes; B02 materializes actual asset paths/sizes/table
indices and B03 certifies them. A draft binding or schema does not claim
`inputs-certified`; B01 approval does not require pretending B02 has already
built the binary assets. For a malformed fixture whose target is not locatable,
bind the exact input and request/error seam, not a fabricated MethodKey; the
fault certificate explains that absence.

Before an Engine request, resolve the binding against the independently certified
original bytes, then adapt the same origin into this request's DexId/ClassKey/
MethodKey. Check indices and descriptor/name/proto in both directions. The
manifest MUST bind a method role without an explicit method selector to exactly
one original method; a class with several methods is insufficient. Duplicate descriptors across assets
or 041 views never use a first match. Full-unit-set and `through` bindings retain
all specified roles/seams, with no duplicate denominator.

The preparer MUST rebuild and certify 041 directly with original physical
header_offset and shared-item extents under ALGORITHMS §1.1; an ordinary DEX
rewritten or header-overlaid for a 035 decoder is not an independent 041
projection. The manifest binds original identities and semantic certificates, not private phase
shapes, byte offsets guessed from output, or values refreshed from Diana.

### 4.7 Execution and failure determination

The development tool interface is
`python3 tools/e2e.py fetch-tools|prepare|verify-inputs|verify-harness`. Only `fetch` may use the network; the other commands read only the
locked tools and checked-in sources and MUST NOT scan the home directory, an SDK or a
cache. Subprocesses use argument arrays and the locked classpath, Windows uses
`join_paths`, and no command may assume a bare `smali.jar` runs with `java -jar`.
Each case isolates its temporary directory and its original and recovered classfiles;
clinit runs in a new JVM per input, and processes and output are both bounded. The
manifest records the original identity, source and license, recipe and
tool locks, `validationScope`, request and catalog, independent expected values, checks
and Driver inputs. Results carry `spec_revision`, which MUST equal the reviewed
specification and manifest revision; digest fields MUST NOT be checked.

A method-level run replaces exactly the complete declaration and body of the target
inside the original self-made legal context and MUST NOT change the target, a
dependency or a stub; it counts only the target method, not a JavaUnit. A full-unit run
compiles the complete output directly and MUST NOT splice-patch it. D8 input is an
explicit classfile list or a self-made JAR, and a directory MUST NOT be treated as a
classfile; `--lib` entries are recorded explicitly and warnings MUST NOT be ignored.
A raw run assembles the original shell and compares through an independent projection
of opcode, word, ref, target, payload, handler, UTF-16 and metadata; offsets and debug
bytes need not match, and the recovered-side decoder MUST NOT generate truth for both
sides. A clean checkout and a new cache run explicitly
fetch → prepare → verify-inputs → verify-harness; an unexecuted platform is honestly
not-verified, a case without a dependency source, license or lock stays blocked and is
not removed from the required set.

Acceptance notation (it applies to every later task card):
- **G0**: the fmt, Clippy, release build and real-test baseline checks below, limited
  to the Diana workspace. Engine cards and application H cards use G0 uniformly and
  then add the domain behavior obligations of each card; no second implicit baseline
  gate exists.
- **B(F…/U…)**: the behavior loop of the corresponding approved D case. The recovered
  Java really compiles in the original legal method context or inside the complete
  unit, and an independent source or JVM Driver, or an explicitly authorized
  independent oracle, compares return, exception, heap, effect, UTF-16 and FP bits.
  The evidence scope of a mirror, hand-written scalar or ART case obeys §4.3 and does
  not check javac success alone.
- **S(F…/U…)**: the original-information check of the corresponding fixed case. Compare actual public debug source/facts with the independent input projection, including identity, opcode/register words, PCs, refs, targets, payloads, handler order and UTF16. Malformed or unknown-width inputs retain their actual failure and safe excerpts, not a fabricated complete-decoding pass.
- The parenthesized labels are capability-family and counterexample tags, not
  executable case selectors. Concrete inputs and expected values come only from the
  JSON and the D ID named by the task card (for example D009 in T06); an author MUST
  NOT run the `answer 7` example from the capability text, and MUST NOT pass with an
  empty selector.

G0 (active workspace only):
```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
```
Once real tests exist, run the public contracts by fixed D ID or card prefix; roundtrip
and assembly are explicit opt-in. A missing tool, fixture or classpath, a nonzero exit,
a timeout, 0 selected cases, or a missing or truncated artifact fails; a skip MUST NOT
count as a pass. An approved revision's full suite is not filtered by current engine
support; a batch MAY select fixed IDs, but the ledger keeps every required case and
invocation in the JSON, including appended IDs. Do not run the ref-fork gate. The
workspace having zero selected tests is not a pass, and a command interface is not
an actual run result.

## 5. Engine tasks T01–T25

## T01 — Engine contract and direct typed scheduling

**Dependencies**: B04. **Phase**: P0/Pipeline/P9. **Source references**:
KU-01/02/04/05, RG-04.

**Target files**: the existing scaffold `crates/diana-decomp/Cargo.toml` (other
paths in this page are relative to that crate), `src/{lib,engine}.rs`,
`src/pipeline/{schedule,control,diagnostics}.rs`,
`src/p0_knowledge/{mod,policy}.rs`, `src/substrate/{ids,origin}.rs`,
`tests/contracts.rs`. The package is `diana-decomp` and the Rust import is
`diana_decomp`, separate from the application package `apps/diana/`. The crate
is already a workspace member, so building the scaffold MUST NOT count as this
card done. The old dependencies were removed from the main workspace and lock
and kept in ref-fork; H08 checks that they are not reintroduced. Versions follow
the formal process, and the author MUST NOT hand-drive a release.

**Input→output / Interface**: implement the
`DexImage<'a>`/`Engine<'a>`/`RunContext`/
`Options`/`Request`/`Report`/`MethodOutput`/`ClassOutput` types and classified
diagnostics of DESIGN §2; an unwired Java path MAY only report Unsupported or
Unavailable, never a placeholder success. Limits live only in RunContext;
open/decompile/render share `&mut RunContext`, and Limits default to all None.
Only an explicit limit uses cumulative used/held quotas and a bounded raw
reservation. `schedule` calls concrete typed functions directly, and PhaseId is
a diagnostic label only; the author MUST NOT implement PassSpec, a registry, a
trait or a macro.

**Steps**:
1. Create the minimal crate and its test registration; only the owning validator may
   access the enums and the private Verified constructor.
2. Confirm the basic enums, interfaces, forwarding and error wrapping by compilation
   and source review; do not write an "enum value equals itself" or mechanical getter
   test.
3. Implement the optional request and method quotas per DESIGN §3.7. The default None
   adds no threshold and no fixed raw reservation; only an explicit finite raw value
   claims from the parent quota. The normal and raw work sub-quotas are independent,
   used quantities are never refunded, and only unconsumed held quantities are
   returned. open/index/render charge and cancel the same way, Report detaches from
   temporary IR, and trace is optional and bounded.
4. Do not write tests for private Pass call order and do not create empty algorithms
   for unimplemented phases. Verify through a real public request that resource
   exhaustion and cancellation stop scheduling, with trace as a location aid only.
5. Implement the diagnostic contract of DESIGN §2.3: typed code/kind, severity,
   original method/site, optional finite phase and charged detail. Wall time MUST
   NOT enter a text golden; no pass registry or speculative code catalog is needed.

**Invariants / limits**: Every state is exhaustible; a required typed transition
MUST NOT be disabled by a recovery policy; no registry or PassId repetition test
is written. A work or allocation charge MUST NOT overflow, and a refund or retry
MUST NOT bypass a limit. Cancelled stops the request and does not trigger
fallback.

**Tests**: F00 focuses on complex resource control: a public request exactly at
a budget boundary and over it, fallback budget isolation after a resource error,
stopping after cancellation of long work, and repeated requests or failures that
bypass no limit and leak nothing. Basic state enums, wrappers and private Pass
call order are not tested; real Malformed and Unsupported behavior is verified
by later input and semantic fixtures. While T01 has no runnable real request,
that part stays a later integration requirement and the author MUST NOT
fabricate invalid tests to fill it.

**Acceptance**: G0 plus source and interface review. Run the nonzero `t01_`
domain tests once a real resource or cancellation request exists, and state
explicitly that the interface awaits T05 wiring while it is unwired; do not
write an empty green.
**Completion**: The crate compiles and the resource-control implementation has
an explicit verification location. Basic enum, routing and forwarding tests are
not required, and unwired capability MUST NOT count as success.

**Not in scope**: Pass trait/registry/schema generators, TOML/build.rs,
ActionDatabase, restart, parallel cache, a separate console.

## T02 — Safe DEX catalog and raw method ranges

**Dependencies**: T01. **Phase**: P1/P0. **References**: RG-06, DX-11/12.

**Files**: `src/p1_input/{mod,cursor,header,catalog,code}.rs`,
`src/p0_knowledge/declarations.rs`, `src/substrate/types.rs`, `tests/input/`.

**Input→output**: `DexImage<'a>` → `ProgramIndex<'a>`, with bytes held by the
caller's scope. This card uses `header_offset = 0` for an ordinary DEX; T04
supplies native 041 parsing and H04 supplies application identity adaptation.
Locate a class by descriptor plus `class_def_idx` and a MethodKey by `name` plus
`proto`. This card produces `RawMethodView`, which guarantees only that the code
and tries ranges are safe; T03/T04 construct the `MethodInput` carrying a
`ValidatedRawMethod` only after the complete P1 `validate_raw`, so an unchecked
offset never crosses the seam. T04 supplies MUTF-8 decoding, and ASCII MUST NOT
replace a complete name or descriptor.

**Steps**:
1. Implement `checked_range(offset, count, stride)` plus u16/u32/ULEB32/SLEB32, and
   enforce the byte and code-unit type distinction.
2. Bound and align the header 035, little-endian, file_size, header size, map and ID
   tables separately. An overrun is BudgetExceeded and truncation is Malformed; an unknown
   container or version is Unsupported.
3. Check the Type/Proto/Field/Method/class_data indexes with `checked_add` on the
   accumulated delta; handle a duplicate class or ambiguous name by key and proto.
4. Cover the complete code_item range: `insns_size * 2`, odd padding, try table,
   handler list base and target ranges. Check the count against the remaining bytes,
   the minimum record and the global budget before allocating.
5. Support abstract and native declarations with zero `code_off`, check consistency
   between proto words and `ins_size` when code exists, and let the raw fallback read
   only the original validated range.

**Invariants / limits**: Verify lengths before allocating; reject high-bit
overflow in 5-byte LEB; do not allocate `sleb::MIN` absolute size directly;
budget decoded metadata in total, not only per table. A locatable local method
malformation MUST NOT cost the other methods of the class their raw identity.

**Tests**: F01 `TinyDex`: an empty class, an abstract method and an exact-proto
overload; M01 header truncation, `offset * stride` overflow and map
out-of-range; M02 method code delta overflow and wrong tries padding or
`handler_off`; a small BudgetExceeded limit. Rust builds the tiny DEX itself and
records the builder source; the test MUST NOT reference an unknown real sample.

**Acceptance**: G0 plus `cargo test -p diana-decomp --test contracts t02_`.
**Completion**: Correct MethodKey and raw range; every M01/M02 is rejected
explicitly before allocation, with no panic.

**Not in scope**: APK/JAR loading, all-platform symbol resolution,
source-support decisions, calling a third-party parser first and guarding
afterwards.

## T03 — Instruction formats and the original-information debug exit

**Dependencies**: T02. **Phase**: P1/P2 (raw decode)/P9. **References**: DX-11,
RG-02/05.

**Files**: `src/p1_input/formats.rs`, `src/p2_flow/decode.rs`,
`src/p9_emit/{mod,debug,spans}.rs`, `src/engine.rs`, input/debug tests.

**Input→output**: checked original ranges → original instruction/declaration
facts → readable debug snapshots under ALGORITHMS §7. Java analysis consumes
validated raw facts; the debug exit retains available facts even when the whole
method cannot be validated. Known unsupported Java and unknown instruction width
remain distinct. No standard-Smali or reassembly guarantee is made.

**Steps**:
1. Share opcode, mnemonic, format, width and version facts between the decoder,
   validator and debug renderer. Do not guess a missing instruction width.
2. Check instruction/payload starts, registers, word counts, references and branch
   targets. Preserve original PCs and invoke word order; T04 supplies payloads,
   strings and metadata facts.
3. On Java failure discard its unverified source, release unused scratch and show
   the original method identity, typed reason, reliable instructions, targets,
   payloads, handler order and referenced values. Other methods keep their Java.
4. Expose the same snapshot in default text and JSON, with actual source spans.
   Partial decoding identifies missing ranges and any safe original-byte excerpt;
   it never fabricates instructions, Java bodies or complete coverage.
5. Charge the same request and any explicit fallback quota. Cancellation stops
   immediately; Malformed/BUG/resource failures retain their actual failure kind.

**Invariants / limits**: The debug exit reads only original checked bytes/facts,
not private Java analysis products. It does not assemble, retry another engine,
raise limits or introduce numeric default quotas.

**Tests**: F02 and the fixed negative cases compare actual original-information
snapshots with independent decoded inputs, including words, PCs, references,
branches and explicit unknown regions. Unavailable/partial information remains
honest; it is not counted as verified Java or executable source.

**Acceptance**: G0 plus nonzero `t03_` public-interface cases and S(F02) after T05.
**Completion**: Supported facts stay analyzable after Java rejection; a local or
unknown-width failure neither guesses code nor discards other members.

**Not in scope**: A bytecode VM, standard-Smali output, output reassembly,
alternative engines, exception stubs or a recovery subsystem.

## T04 — Lossless UTF-16, payload and raw metadata completeness

**Dependencies**: T03. **Phase**: P1/P9. **References**: RG-06/07/09, DX-08/11.

**Files**: `src/p1_input/{strings,payload,values,debug}.rs`,
`src/p9_emit/{debug,literals}.rs`, `src/substrate/types.rs`, input and literal
fixtures.

**Input→output**: MUTF-8 → `DexString(Vec<u16>)`; a valid switch/array payload,
encoded value or raw debug record → complete Method/Declaration raw records.
Also implement the native 041 `0x78` header and physical offset rules and accept
the original container plus `header_offset` directly; this card owns the D065
engine read, and H04 keeps the application adaptation. A diagnostic lossy string
MUST NOT become literal truth.

**Steps**:
1. Decode MUTF-8 NUL, surrogate and invalid encoding and declare `utf16_len`. Check
   the remaining bytes and the string units first, and never let a replacement change
   the literal.
2. Base switch payload alignment and targets on the switch PC and check the key
   monotonicity, uniqueness and count the original format requires; do not reorder the
   original input for stable output. Check the fill-array width count and preserve the
   byte bits; a payload is not an executable block.
3. Decode the debug program opcode by opcode within bounds and check address
   increments, line values and sign overflow. If that metadata cannot be decoded
   reliably, retain the actual failure and mark the unavailable metadata explicitly; other checked facts remain visible.
4. Bound the annotation and encoded-array stacks and reject an illegal value width,
   tag or index. Preserve system metadata under its original identity and do not emit
   it as a Java keyword.
5. Handle Java literals and JSON debug snapshots per ALGORITHMS §4 for NUL, newline, backslash-u and
   surrogate. Walk the logical header chain of 041 per §1.1: `file_size` is only a
   step size, and every section offset stays a physical coordinate that may point to
   later shared data but never to earlier data. Do not overlay or re-checksum.

**Invariants / limits**: Preserve every u16 and every float/double bit; cover
all payload bytes and charge the nested count depth; never run
`Vec::with_capacity(input_count)` before validation.

**Tests**: F03 `Utf16Payload`: unpaired high and low surrogates, NUL, the
literal `\\u000a`, NaN bit patterns, sparse and packed switch, fill array; M04
negative relative payload, overlap, oversized array or annotation, bad debug
target and unterminated MUTF-8. A later S(F03) checks original debug facts against independently decoded literal and
payload values.

**Acceptance**: G0 plus `cargo test -p diana-decomp --test contracts t04_`, with
S(F02, F03) after T05. **Completion**: The original-information comparison explains
per field either exact preservation or explicit unsupported status, with no
silently lossy value.

**Not in scope**: Source-level annotation or generic beautification,
StringBuilder concatenation, all newer DEX metadata.

## T05 — Integrate the frozen E2E suite and the minimal typed product/AST

**Dependencies**: T04. **Phase**: Pipeline/Substrate/P9 and the test system.
**References**: KU-08, RG-08, DX-13.

**Files**: `src/substrate/{cfg,java}.rs`, `tests/contracts.rs`, the thin
`tests/support/e2e_adapter.rs`, `examples/inspect_class.rs`; read the root
`tests/e2e/` cases, manifest, expected values and locked harness that B01–B04
prepared, and do not copy a second oracle or tool source.

**Input→output**: Build the Flow/Ssa/Typed/Storage/PlannedBody backbone and its
private constructors per DESIGN §3.2; the fixture manifest drives native input
tests, independent debug-fact comparison and the javac/d8 behavior loop. Verify
DeclarationOnly and a raw method first, and never treat an empty AST as success.

**Steps**:
1. Adopt the ownership wrappers of DESIGN §3 without variation; a raw view borrows
   caller-owned bytes, each method gets an owned arena, and an ID is never recycled.
   Use the schema of DESIGN §3 and do not substitute a second context system. Create
   only the artifacts actually consumed now and do not pre-lay a complete AST.
2. `MinimalJavaAst` supports a legal declaration and literal/return placeholders; a
   Validated or Verified value MUST NOT be publicly constructible, and P9 accepts
   verified input only.
3. Read the manifest, D cases, expected values, `toolchain.lock.json`, the
   B03-prepared scalar oracle and input certificates frozen by B01–B04. This card
   MUST NOT refill expected values or
   generate a golden from engine output, and a different public interface changes only
   the thin adapter.
4. Integrate the team-reproducible root test tool and driver, taken from the public
   version/dependency lock and the checkout cache; do not look for a personal SDK, a platform jar
   or a local reference-repository directory. A missing tool or wrong size fails, and
   subprocesses run test tooling only.
5. Compare the actual debug snapshot and its rendered source slice against the independent original-content evidence, including D059–D061. Keep original identities, instruction/register/target/payload/handler/reference facts and UTF16 units; no output assembler is involved.
6. Have the public development example emit Report status and spans, reused by the
   other core tests, instead of calling a private printer directly.

**Invariants / limits**: Test fixtures are self-made and authorized with fixed
sources; no third-party pickle is loaded; an error normalizer MUST NOT delete a
type or catch difference; a zero run count or a nonzero timeout is not a pass.

**Tests**: B04 already falsified harness faults; this card verifies only the
actual Engine result integration: a real wrong return, missing event, wrong raw
or compile error is rejected by the same comparator, and a correct source
reference is accepted. A missing tool or size error stays a failure, and a
required member MUST NOT change because the real engine does not support it.
S(F02/F03 fixed D case groups) MUST inspect the actual public debug output and its original-fact projection.

**Acceptance**: G0 plus nonzero real semantic and raw tests of the public
adapter, referencing the suite frozen by B04. **Completion**: The same fixed
case, expected value and harness connect to the real public library without
redesigning the oracle; unimplemented cases still report not passing, and
complete semantic verification MUST NOT be claimed.

**Not in scope**: Large-corpus campaigns, CI auto-download of private APKs, RG
cache copies, production CLI wiring.

## T06 — First Java end to end: constants, parameters and explicit scalar locals

**Dependencies**: T05. **Phase**: P2–P9 minimal profile. **References**:
DX-01/02/08, RG-02.

**Files**: `src/p2_flow/{mod,build,results,effects,verify}.rs`,
`src/p3_ssa/{mod,inputs,build,verify}.rs`, `src/p4_calls/mod.rs`,
`src/p5_types/{mod,constraints,lattice,solve}.rs`,
`src/p6_variables/{mod,locals,definitions}.rs`, `src/p7_regions/mod.rs`,
`src/p8_java/{mod,expressions,declarations,verify}.rs`, `src/p9_emit/java.rs`.

**Input→output**: The selected static method of F05 `ScalarReturn` yields a
VerifiedJavaMethod through the real schedule. A class containing an unsupported
member such as a constructor is honestly marked Mixed, and an unsupported other
method stays original DEX debug. This card does not require a complete ordinary class to
be a JavaUnit. P4 returns an empty valid binding when there is no call, and P7
returns a single-block plan when there is no complex region; the card MUST NOT
claim either is fully implemented.

**Steps**:
1. Supported ops: constants, parameter moves, simple int add/sub/mul and return.
   Verify the normal terminal, and still validate raw code after the return.
2. Bind input words, op result ValueIds and simple type constraints, and map ValueId to
   LocalId with an explicit assignment, without prior GVN or inlining.
3. Emit an int literal with its original bits and appropriate signed spelling; a Java
   expression statement covers only call, new and update, and arithmetic writes a local
   assignment.
4. Check the Java AST completion, return type and identifiers, and mark a method
   Verified only after checking the directly determinable signature, MUTF-8 and member
   facts per ALGORITHMS §12.4–§12.6. The card MUST NOT predict code, locals, stack or
   pool and then reject source; a static declaration of 256 ints MUST be
   Unsupported/raw, and an unused parameter MUST NOT be deleted. P9 emits the method
   and context status plus origin spans and does not guess a proto from the name. The
   F05 compile replaces the selected method only inside the known context of the
   original self-made source, keeps the constructor and other members from the original
   source, and generates no semantic stub; therefore it counts only the Java legality
   of the selected method and does not count Mixed as a complete Diana JavaUnit.
5. Real input produces a result through the implemented typed transitions, and a
   failure leaves no Java fragment. Accept the result through the semantics, error
   status and budget/cancellation observations of the public interface. Artifacts and
   trace only aid location, and the card adds no private Pass call-order, call-count or
   per-phase count test.

**Invariants / limits**: No handler and no monitor; an undefined read MUST be
rejected; long and double word contracts arrive only in T10, so an unknown
profile is Unsupported instead of a guessed int; an effectful expression MUST
NOT be inlined.

**Tests**: T06 actually runs the fixed D009 of B01–B04 (int addition/overflow)
rather than a new answer/inc fixture generated during development; `answer() ->
7` and similar cases only illustrate the implementation. Undefined reads, wide
values and budgets connect through D055–D064 and the fixed task mapping, and an
insufficient scope still fails without changing expected values. D009 runs the
recovered method in a known legal context, and an unsupported default
constructor MUST NOT be counted as a complete class JavaUnit.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F05).
**Completion**: At least one non-empty method completes the real
compile-and-execute loop, and the negative cases are not default-value
"successes".

**Not in scope**: Complex branches, handlers, variable merge, expression
beautification, generic typed-helper generation.

## T07 — SSA for branches and join Phi with fixed identity

**Dependencies**: T06. **Phase**: P2/P3/P5/P6. **References**: DX-03/04, RG-03.

**Files**: `src/p2_flow/build.rs`, `src/p3_ssa/{liveness,build,verify}.rs`,
`src/p5_types/constraints.rs`, `tests/pipeline/ssa.rs`.

**Input→output**: normal-branch FlowBody → SsaBody with join inputs identified
by EdgeId and stable ValueIds. A method that Java copy lowering does not fully
support yet MAY be Debug, but the internal SSA MUST be fully verified.

**Steps**:
1. Build block leaders from the entry, branch targets and after-terminal positions;
   normal branches have exact true and false targets, and a goto is a terminator, not
   an effect NOP.
2. Compute the monotonically growing live-in slot set backwards per ALGORITHMS §3, and
   keep the normal write kill separate from the throw-before requirement. Build a fixed
   Phi for every live-in slot of every block, including a single predecessor; a virtual
   EntrySeed provides the parameters.
3. Scan each block once: bind all reads and save the required throw old values first,
   then invalidate overlapping slots and write fixed definitions. Connect every incoming
   once; an unfilled Option is not a semantic Undefined.
4. Keep the Phi closure from actual uses and propagate Bad and grounded monotonically.
   Reject a missing definition, a partial wide value and a rootless definition cycle.
   The validator replays the block's latest definition and the normal/exception origin,
   and MUST NOT substitute a normal dominator for the exception check.
5. Do not eliminate trivial Phis or shrink aliases; types keep finite constraints. Drop
   the Env and live scratch after sealing while keeping the necessary Phi inputs and use
   bindings for P5/P6.

**Invariants / limits**: A finite worklist; a loop-preallocated Phi is
internally testable even though this card emits no loop; a branch condition
binds exactly one use; every eligible edge gets one Phi input.

**Tests**: F06 `DiamondPhi`: `choose` with two const or param values, a nested
diamond and the same block reached by two edges; M06 missing Phi input,
wrong-edge ValueId, non-dominating use and undefined branch join. Positive input
SSA plus mutation counterexamples.

**Acceptance**: G0 plus `cargo test -p diana-decomp --test contracts t07_`.
**Completion**: The join certificate is really generated and independently
checked, with stable-ID and budget tests; T08 MUST connect the F06 Java
behavior.

**Not in scope**: Exception SSA, full heritage, dominance-frontier optimization,
coalescing. A card MUST NOT call something SSA from a variable table alone while
omitting the join.

## T08 — Independent Local allocation, edge parallel copy and loop values

**Dependencies**: T07. **Phase**: P3/P5/P6/P8 minimal transfer. **References**:
DX-04/05.

**Files**: `src/p6_variables/{locals,copies,definitions}.rs`,
`src/p3_ssa/build.rs`, the minimal diamond adapter in
`src/p8_java/structured.rs`, storage tests.

**Input→output**: TypedBody → StorageBody plus a normal CopyPlan; every
non-literal definition and Phi gets an independent LocalId. The F06 simple
diamond can emit Java; a loop is verified first through an internal plan, and
T09 provides general execution.

**Steps**:
1. A parameter and `this` are pinned bindings, and different ValueIds do not share
   locals by default; a register number MUST NOT become a LocalId directly.
2. `Phi.inputs` produce parallel assignments by EdgeId, and a critical edge keeps its
   edge-copy placement instead of landing at the end of the source block.
3. Use the parallel sequentialization of ALGORITHMS §5 strictly: emit a destination only
   when it is not in the remaining sources, otherwise save one cycle source into an
   exact-type temp and rewrite every use of that source. An entry copy runs before the
   first entry, and a branch or latch copy runs after the selected edge.
4. `DefinitionCertificate` independently derives original-use and edge-copy availability.
   Only an ordinary local already proven unobservable MAY use default initialization to
   satisfy Java DA.
5. Preallocate and propagate the several Phis of a loop entry or latch, and verify the
   normal copy plan through an internal interpreter in tests; the minimal-diamond P8
   implements single-condition evaluation and transfer.

**Invariants / limits**: A dead default is no excuse to read Undefined; a copy
cycle charges a temp and proves the source-destination type; the edge is
selected before the copy, and every copy is pure with no throw. Uninit MUST NOT
be initialized to null to impersonate a Ref.

**Tests**: F06 `choose` with positive and negative conditions in two drivers;
F07 `CopyCycleLoop`: `a<-b, b<-a` expecting a swap, 2- and 3-cycles, a critical
edge, and a loop entry and latch; M07 naive sequential assignment, a copy placed
on the wrong branch, an observed default and a type-cast copy. B(F07) follows
after T09.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F06).
**Completion**: The simple branch is real Java; the validator fails every cycle
and critical-edge parallel-copy counterexample; the loop-copy obligation is
explicit and the card MUST NOT claim all loops are Java complete.

**Not in scope**: Interference coloring, local-name role inference, exception
shadow, cross-region leave-copy heuristics.

## T09 — Ordinary CFG dispatcher and end-to-end loops

**Dependencies**: T08. **Phase**: P7/P8/P9. **References**: KU-06/07. The
approach is a new DEX adapter that does not copy Cgoto.

**Files**: `src/p7_regions/{normal,verify}.rs`,
`src/p8_java/{dispatcher,verify}.rs`, `src/p9_emit/java.rs`.

**Input→output**: A StorageBody without handlers, monitors or Uninit →
DispatchPlan → VerifiedJava, with one case per reachable block. The local pc
takes the value matching this run's BlockId, and case display order need not
match across runs.

**Steps**:
1. Check eligibility explicitly; a graph containing any handler stays out of the
   dispatcher, and a constructor uninit is likewise rejected.
2. Inside a case follow the original op order; each arm of a Branch emits the
   corresponding copy, the pc next value and a continue, while return and throw exit.
   Certify that the default pc is unreachable.
3. Route loops entirely through the edge pc and do not recognize while or for first;
   nested cases keep legal Java brace scopes with all locals visible in the outer scope.
4. `StructureCertificate` re-enumerates ops and edges to confirm coverage. The synthetic
   pc assignment has no heap effect and no allocation, but the default-unreachable
   assertion MUST NOT depend on a guess about user input.
5. Support an ordinary irreducible multiple-entry loop, where an extended budget affects
   BudgetExceeded only and not output semantics; a no-handler case MUST NOT fall back wrongly
   because natural structuring failed.

**Invariants / limits**: Evaluate the condition once, copy the edge before
assigning the pc, and map case targets and keys to block identity; the display
order of dispatcher cases that do not fall through to each other need not be
stable. Reserve an output estimate against the limit first, and never build a
full string and then drop it.

**Tests**: B(F07) swap loop with several iterations, 0 iterations, continue and
break; F08 `DispatchGraph`: simple cycle, two-entry cycle, multiple exits and
throw terminal; M08 missing edge, duplicated effect, reachable default and
ineligible handler. A hand-made two-entry CFG uses the B03-prepared tests-only scalar
interpreter integrated by T05 as an independent oracle and is compared against the actual
execution of recovered Java; this card introduces no fake source substitute for
an original hand-made CFG that the JVM cannot generate directly, and exception
or monitor cases do not use this simplified oracle.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F07, F08). **Completion**: A complex ordinary graph yields legal Java instead
of goto pseudo code, and an eligibility negative MUST NOT be accepted.

**Not in scope**: A try/monitor state machine, an `Object[]` register-boxing
interpreter, duplicating the effect tail, byte identity as a natural-structure
quality metric.

## T10 — Complete primitive/null/FP legalization

**Dependencies**: T09. **Phase**: P2/P5/P8/P9. **References**: DX-08, RG-09.

**Files**: `src/p5_types/{lattice,constraints,solve,conversions}.rs`,
`src/p2_flow/decode.rs`, `src/p8_java/expressions.rs`,
`src/p9_emit/literals.rs`.

**Input→output**: bit and word types plus use constraints →
TypedBody/UseConversion → legal primitive or ref-null Java. The card supports
boolean, byte/short/char/int/long, float/double, cmp/cmpl/cmpg, conversions and
shift; insufficient type evidence is explicitly Unsupported.

**Steps**:
1. Close the descriptor facts and the family equations per ALGORITHMS §4, then
   propagate origins, Known01 and producer ranges monotonically. After sealing, select
   the source type once and give each use of a literal its own view. ProvenNarrow
   accepts only a lossless conversion that falls entirely inside the target range.
2. Check wide word pairs and alias overlap or partial reads; map every shift mask,
   signed and unsigned operation, narrowing and DEX type semantic table entry to its
   opcode.
3. Express a boolean bitwise operation as legal truth values and `eqz` on a ref as
   `== null`; an ambiguous non-literal value MUST NOT be reinterpreted as a direct cast.
4. Emit an FP cmp as `x < y ? -1 : x > y ? 1 : x == y ? 0 : bias`, where x and y are
   locals and bias is -1 or +1. No `isNaN` library call is needed and the NaN branch is
   not folded away. Do not reassociate FP. A NaN payload or bitcast that cannot be
   expressed exactly is Unsupported, and recovery adds no library call for it.
5. Have the typed literal escape cover negative MIN, Infinity, -0 and surrogate, and
   keep every UseConversion checkable for no-extra-runtime-check.

**Invariants / limits**: Finite constraints converge and a Conflict is rejected
explicitly; overflow and Java numeric promotion do not rely on a host Rust
default guess.

**Tests**: F09 `PrimitiveBits`: boolean negation and bit operations,
0/null/false, MIN/-1, shift -1/32/64, NaN cmpl/cmpg, negative zero, float→int
boundaries and wide overlap; M09 wrong kind, partial wide and unresolved
primitive. Confirm that a byte parameter can narrow losslessly after an int Phi
and that a value mixed with short/int arithmetic sources MUST NOT be proven
byte. B(F09) compares bits and traces, not formatted text. The primitive
positives of F09 introduce no library call, and a bitcast without an exact
expression rule is Unsupported/raw. An existing library call in the original
program binds per T11 and gets no new synthetic call. This card supplies the
type evidence for the fixed D071 return and D072; the D071 call completes the
whole case only after T11 binding. D076 covers the MUTF-8 target byte boundary.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F09).
**Completion**: Every family of the primitive matrix has a positive and a
boundary case, and a ruleless bitcast or a synthetic intrinsic MUST NOT be
marked supported.

**Not in scope**: Full reference generics, arithmetic constant folding or GVN,
FP algebraic compression, universal numeric reinterpretation.

## T11 — Exact call contracts, external declarations and result binding

**Dependencies**: T10. **Phase**: P0/P2/P4/P5/P8. **References**: DX-02/06/08,
KU-01.

**Files**: `src/p0_knowledge/declarations.rs`, `src/p2_flow/results.rs`,
`src/p4_calls/{mod,members,contracts}.rs`, `src/p5_types/constraints.rs`,
`src/p8_java/expressions.rs`.

**Input→output**: raw invoke plus a complete or unknown catalog → CallEvidence
with exact kind, owner and proto, a bound receiver plus argument words, and a
typed result; missing binding facts → Debug, never "the call looks right, so
emit it".

**Steps**:
1. Check the invoke format and range, receiver first and descriptor words; a move-result
   MUST follow a legal producer, have the correct kind and not be a branch target. An
   unconsumed call result still keeps the call.
2. A self-made complete catalog supplies static/direct/virtual/interface/super,
   visibility, overload groups and throws; keep Unknown separate from Partial. P4
   produces descriptor constraints and BindingObligations and does not require a source
   type that P5 has not solved.
3. After P5 finishes types and UseConversion, P8 discharges the exact Java binding:
   receiver or static owner, argument static types, unique overload or dispatch
   equivalence. A cast MUST be no-extra-check or the original checkcast.
4. Treat a call as conservatively may-throw and do not DCE an unused result; checked
   throws agree with the caller declaration and override, and the card MUST NOT add a
   sneaky throw, wrap a catch or widen the contract.
5. The report keeps the original MethodRef and the EnvironmentId, and a same-name-
   different-proto request selects precisely.

**Invariants / limits**: Charging a catalog candidate scan; Unknown never fills
a default public; argument evaluation is not reordered; any added runtime check
needs evidence.

**Tests**: F10 `CallKinds`: overload int/long/Ref/null, virtual override,
interface, private/direct, super, effect arguments, unused throwing call; F11
`MissingCatalog` gives Java for the same input when complete and explicit Debug
when empty; M10 bad result/argument words/static receiver/visibility/return-only
overload. The D071 call consumes the sealed range; D080 verifies Constant and
Unknown rejection separately and adds no initialization helper.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F10) plus S(F11). **Completion**: Every kind has a behavior driver and a
reject case; a missing catalog is an expected degradation, not a pass that
"ignores errors".

**Not in scope**: Native calling conventions, reflective inference, dynamic
invoke-custom or bootstrap Java semantics, an all-platform catalog.

## T12 — Object initialization identity and restricted constructors

**Progress**: [Inherited binding repair](batches/t12-inherited-constructor-binding.md)
adds checked class/interface ancestry, constructor overload selection and
covariant receiver/bridge proofs. D033 now passes source-JVM E2E without changing
allocation/init order. Missing or cyclic ancestors, uncertain bridges/generics
and real return-only collisions remain rejected. The
[entry initialization batch](batches/t12-entry-initialization-facts.md) replaces
the P1 register-prefix interpreter with shared P2 decoding, P3 definition/use
binding, P4 declarations and P5 initialization-link evidence. Delegation checks
retain caller Java across a callee's post-init definition, reference, encoding or
return-shape failure; cycles and unrelated constructor owners remain rejected.

**Dependencies**: T11. **Phase**: P2/P3/P4/P5/P6/P8. **References**: DX-09, DX-02
RecoverConstructors.

**Files**: `src/p1_input/method_envelope.rs`,
`src/p2_flow/{local_ops,entry}.rs`, `src/p3_ssa/{normal,entry}.rs`,
`src/p4_calls/{allocation,entry_declarations}.rs`,
`src/p5_types/{entry_initialization,initialization/chain}.rs`,
`src/pipeline/constructor_chain.rs`,
`src/p8_java/{branch,constructor_entry,constructor_graph}.rs`.

**Input→output**: An adjacent new-instance and invoke-direct init → allocation
evidence and a legal Java `new`; the UninitThis of `<init>` → a single legal
super or this chain → a normal Ref body.

**Steps**:
1. Track only a pure move-object with an AllocationId and forbid uninit escape, an
   ordinary call or a field read. Verify availability per use after init completes
   normally, and use DeferredAllocation in P6 instead of globally turning the ValueId
   into a Ref or assigning null.
2. Require the arguments to be prepared before `new`, or to be literals, with no
   intervening effect or may-throw; the Java `new` and constructor combination MUST NOT
   move an effect across the allocation or class-init timing.
3. A constructor prologue permits only pure argument preparation that does not observe
   uninit this, plus a single expressible chain; the real super or this behavior is not
   deleted.
4. Check ownership and proto access fully; multiple init, a joined uninit or an illegal
   Java argument order → Unsupported, preserved by the raw fallback.
5. T20 handles complex final-field assignment in full; this card keeps a simple
   post-super field write eligible first and does not move it to an initializer early.

**Invariants / limits**: Uninit MUST NOT become an ordinary Ref through a
default null; new and init are consumed by one source event while both origins
are preserved; there is no universal static helper.

**Tests**: F12 `AdjacentConstruction`: pure move alias, arguments ready before
allocation, super trace → body trace, this chain; U02 `ConstructorPrologue`: a
pre-super this field, control flow/monitor/may-throw arguments, duplicate init,
which MUST be Debug rather than Java.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F12) plus S(U02). **Completion**: The constructor trace order agrees, every
uninit negative is handled explicitly, and an unsupported special case MUST NOT
delete super to hide it.

**Not in scope**: An arbitrary constructor carrier or helper, anonymous capture,
an enum implicit constructor, a large rewrite of object allocation.

## T13 — Reducible if/while natural-structure adapter

**Dependencies**: T10. **Phase**: P7/P8. **References**: KU-06/07, the DX-07
region responsibilities.

**Files**: `src/p7_regions/{normal,verify}.rs`,
`src/p8_java/{structured,verify}.rs`.

**Input→output**: normal StorageBody → private SESE/SCC region plan → natural
Java; when the schema cannot cover the graph, the ordinary dispatcher of T09
applies and the original CFG is not modified.

**Steps**:
1. Build Sequence/Diamond/NaturalLoop from the RPO, normal dominance and post-dominance
   the algorithm needs, and reduce the region node count on each collapse. Add no stable
   sort or tie-break to force one source form.
2. Keep the EdgeId of the loop header, latch and exit; copies for continue and break run
   before the transfer, and the tail MUST NOT be deleted by block visitation.
3. Support the restricted profile of a single-entry loop with multiple exits, where the
   legal `while (true)` plus `if break` form suffices, and do not sugar it into for or
   foreach.
4. `RegionCoverage` verifies that the leaf block sets do not overlap and that Transfer
   maps exactly; origin and effect coverage is not a text count.
5. For the same fixture, compare behavior evidence through the StructuredThenDispatch
   and DispatchOnly adapters; a pure readability difference does not require identical
   text.

**Invariants / limits**: The original topology, SSA and CopyPlan are unchanged;
the private arena is bounded; an irreducible graph uses the existing legal
dispatcher, and a handler or monitor graph still forbids an ordinary fallback.

**Tests**: F13 `NaturalControl`: diamond, while with 0 and several iterations,
nested loops, conditional break and continue, copy on exit; M11 lost edge, wrong
scope, duplicated region; B(F13) with both modes agreeing on result and trace.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F13).
**Completion**: A simple loop yields an explicit natural AST and a hard graph
uses the dispatcher; an invalid certificate MUST NOT degrade into a lenient
success.

**Not in scope**: NoMoreGotos/tail-dup/full SAILR, exception regions,
variable-count reduction, goto quality as a correctness oracle.

## T14 — Integer switch, payload and fallthrough structure

**Dependencies**: T13. **Phase**: P2/P7/P8. **References**: RG-02, DX-07, KU-06.

**Files**: `src/p2_flow/build.rs`, `src/p7_regions/normal.rs`,
`src/p8_java/structured.rs`, `src/p9_emit/java.rs`.

**Input→output**: A verified packed or sparse payload → key and EdgeId mapping →
a Java switch or the dispatcher, with shared targets and fallthrough preserved.

**Steps**:
1. Keys are ordered and unique and relative offsets are correct; the default edge MUST
   NOT be faked as the last case.
2. Merge a shared target into one case but keep the original keys and origin edges;
   fallthrough, return and break targets are explicit and different copy plans are not
   merged.
3. When edges with different Phi inputs reach the same target, run the corresponding
   copy inside the case before transferring to the target; the edge difference MUST NOT
   be lost because the block is the same.
4. Prefer the dispatcher for a non-SESE case or a loop inside a case, and an ordinary
   graph structuring failure MUST NOT emit an undefined goto.

**Invariants / limits**: The selector is evaluated once; every key is
semantically exact; switch key count and output bytes are bounded; a payload is
not an executable op.

**Tests**: F14 `IntegerSwitch`: negative/min/max keys, shared target, default,
fallthrough, loop break, case edge copy; M12 duplicate key, out of range, branch
into a payload. B(F14) covers every case and the default with a side-effect
selector. The fixed D083 extreme keys verify bounded completion and the real
branch without asserting an AST print form.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F14).
**Completion**: Ordinary integer switch coverage is complete, a string or enum
switch keeps its integer control flow, and no unspecified switch sugar is
generated.

**Not in scope**: String hash-switch or enum ordinal sugar, unknown bootstrap
switches, automatic restart of branch cascades.

## T15 — Fields, arrays, explicit check-cast and visible effects

**Progress**: [Shared static int fields](batches/t15-shared-static-fields.md)
adds ordered `sget/sput` with declaration, constantness and qualifier checks.
D044's `get` member recovers Java; its required whole-class result still fails.
[Numeric fields](batches/t15-numeric-fields.md) and
[array components](batches/t15-array-components.md) add float/wide access,
descriptor-first load typing and original word-pair checks.
[Boolean/narrow heap access](batches/t15-narrow-heap.md) adds original truncating
stores and checked boolean views. [Reference fields](batches/t15-reference-fields.md)
adds exact/null/Object-widening assignment views and field-produced array types.
[Nominal type checks](batches/t15-nominal-type-checks.md) adds original casts and
instanceof. [T15 contract review](batches/t15-acceptance.md) records the executed
native evidence and fixed cases. [Object[] allocation](batches/t15-reference-array-allocation.md)
closes Object[] new-array admission. [Filled arrays](batches/t15-filled-arrays.md)
adds argument/result binding and unused-result storage. [Derived array types](batches/t15-derived-array-types.md)
adds bounded component identities, multidimensional copies and reference store
views. The task-scoped T15 profile is verified by 125 core tests, the twenty
required heap rows and independent JVM Drivers in both representation policies.
The supplemental float-filled oracle correction is operator-approved and retains
that original member as classified raw output.

**Dependencies**: T12. **Phase**: P2/P4/P5/P8. **References**: DX-06/08, RG-06.

**Files**: `src/p2_flow/{decode,effects}.rs`, `src/p4_calls/members.rs`,
`src/p5_types/{constraints,conversions}.rs`, `src/p8_java/expressions.rs`.

**Input→output**: iget/iput/sget/sput, array load/store/new/fill/length and
instanceof/checkcast → exact typed ops and legal Java, with raw member identity
not overwritten by a source name.

**Steps**:
1. Match the descriptor kind against the narrow, wide or reference opcode, and infer
   null and array types from constraints instead of guessing String at first use.
2. Take field static/instance, visibility and volatile/class-init facts from the complete
   catalog; unknown flags conservatively affect the effect, and the card MUST NOT assume
   public final.
3. Keep the original array index, value and receiver order. Handle fill per ALGORITHMS
   §2.4: with count 0 still emit the length read and keep the NPE; for a non-empty fill,
   expand the stores in the original order only under a FreshArray proof of sufficient
   length and no escape, keeping the bits of each element; every other shape is raw and
   the card MUST NOT invent an exception or helper.
4. An unused field or array read and a cast still produce a typed temp that keeps the
   throw, init or volatile effect; the original checkcast MUST NOT be deleted because
   the result is unused.
5. Convert a reference join only with no-extra-check evidence; a new cast on Object MUST
   NOT exist merely so javac passes.

**Invariants / limits**: Default heap read, heap write and call barriers, with
no DCE or hoisting; every array allocation and count is charged; a static-init
field read does not become a literal constant and lose its trigger.

**Tests**: F15 `HeapEffects`: field get/set and static-init trace, array
load/store order, null and bounds, an unused checkcast throw, preserved volatile
instructions, wide and float array bits; D027 MUST be Java, a zero-element fill
still NPEs on null, and a non-empty missing shape MUST be proven raw without
partial writes followed by a claim of original semantics; M13 wrong field
opcode, array payload type, missing catalog. B(F15) compares the result, the
mutation log and the exception class. D027 needs a real fill/FreshArray
certificate; D073/74 cover the zero-count exception and the complete raw result
for a non-FreshArray respectively.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F15).
**Completion**: Every supported heap op has a throw and effect negative, and the
original member span is located correctly.

## T16 — Ordered exception graph and SSA environment per throwing site

**Progress**: [Throw-state batch](batches/t16-throw-state.md) adds checked
protection products, ordered exception edges, pre-write SSA inputs and handler
ExceptionAt definitions. The same-register and pure-result-boundary regression
passes, including verifier rejection of reordered ranks and normal-result
substitution. Ordered typed/catch-all and shared handlers, a handler throw under
outer protection, wide old state and resource/cancellation cases pass. The
operator-approved availability diagnostic distinguishes unproved exceptional
ingress from definite malformed reads. T16's graph/SSA contract is verified;
protected Java remains the T18 obligation.

**Dependencies**: T15, T07. **Phase**: P1/P2/P3. **References**: DX-03/07.

**Files**: `src/p2_flow/{protection,build,verify}.rs`,
`src/p3_ssa/{liveness,build,verify}.rs`, `src/p1_input/code.rs`.

**Input→output**: checked try/handler raw code plus may-throw ops → ordered
ExceptionEdges; an SsaBody with the exception environment before the throw and a
handler ExceptionValue. This card verifies internally first; the Java protected
profile completes in T18.

**Steps**:
1. Split every protected may-throw op into terminal sites and split the try boundaries
   too. Preserve the handler list order and the catch-all position, and keep the normal
   and exception graphs independent.
2. A successful instruction result commits only on the normal edge; the exception edge of
   a throwing definition that overwrites an old register keeps the old value, and every
   other opcode-specific transfer is listed explicitly.
3. A handler entry move-exception gets a new ValueId, and a typed chain or common handler
   may have several exception edges; incoming Phis follow the original EdgeId.
4. The exception SSA validator MUST NOT substitute the normal dominator tree for exception
   availability; a read-before-def stays Malformed and an unknown type is not marked
   correct.
5. A normal jump into a handler, an overlapping exception region or an illegal range is an
   explicit error or Unsupported at the raw and flow layers, and is not fixed by
   reordering.

**Invariants / limits**: Each throw edge corresponds to the actual opcode effect
of its site; may-throw classification is conservative; graph expansion charges
the limits, and an over-complex graph MUST NOT be treated as handler-free.

**Tests**: F16 `ThrowState`: `v = old; v = call(); catch { use(v) }`, multiple
typed catch and catch-all, shared handler, throw in a handler; M14 using the
normal new result, a reordered catch rank, an illegal move-exception target, a
try in the middle of an instruction. Tests check the environment, not source
text alone.

**Acceptance**: G0 plus `cargo nextest run -p diana-decomp --all-features --locked --test contracts t16_`.
**Completion**: The exception site and old-state chain certificates are
complete; while Java protected capability is absent the card does not pass as
that Java feature, and T18 MUST add B.

## T17 — Handler Phi shadow snapshots and exception ingress copies

**Progress**: [Exception-storage batch](batches/t17-exception-storage.md) adds
the operator-approved P4/P5 catch-type prerequisite, exclusive P6 shadows,
per-edge pre-throw snapshots and verified ingress copies. Source availability,
exact types, ownership, coverage and resource/cancellation checks pass. P8 records
original-site placement obligations. T18 consumes them for accepted protected
regions and adds behavior evidence for D029/D030; its remaining acceptance is
recorded below.

**Dependencies**: T16, T08. **Phase**: P6. **References**: DX-05.

**Files**: `src/p6_variables/{exceptional,copies,definitions}.rs`,
`tests/pipeline/exception_storage.rs`.

**Input→output**: exception SSA plus TypedBody → a pure pre-throw ShadowSnapshot
and HandlerIngress CopyPlan, with a typed source proof for the handler Phi of
every incoming throwing site.

**Steps**:
1. Enumerate the eligible exception edges of every handler-retained Phi and create a
   ShadowLocal with a fixed identity inside the request; the number need not match across
   runs. The card MUST NOT insert intermediate catch code through ordinary critical edges.
2. Save the old-state value of that edge before the throw site, so that every snapshot of
   every possible handler is no-throw, no-allocation and local-only; bind the catch
   ingress to the PhiLocal.
3. Each (handler, Phi) pair always gets an exclusive shadow, and a throw inside a nested
   inner handler likewise saves the old value for the outer handler; do not merge slots by
   type. Protection ownership follows the one-nested-layer rule of ALGORITHMS §10, and a
   shared or crossing graph is rejected only when ownership cannot be assigned; the card
   MUST NOT use "the latest register value".
4. `DefinitionCertificate` verifies that each snapshot source is defined at that site and
   that the conversion is exact and introduces no runtime check; a frame-level default
   slot is unobservable.
5. P8 produces an explicit placement obligation list, and the try construction MUST
   consume the same-site position; a source copy MUST NOT land in another protection
   domain.

**Invariants / limits**: A shadow affects only synthetic locals, with no heap
and no extra exception; the total number of handler snapshot copies is bounded
and copy cycles are serialized per T08. Every throw edge has a source, and a
value MUST NOT be guessed from the handler's last write.

**Tests**: F16 old-value internal storage, F17 `HandlerPhi` with two sites
holding different old values, a loop try shadow, a shared target; M15 a snapshot
checked cast, an Undefined snapshot, a snapshot placed after the call, an alias
reuse that overwrites wrongly; the validator MUST fail.

**Acceptance**: G0 plus `cargo nextest run -p diana-decomp --all-features --locked t17_`, with
B(F16, F17) added after T18. **Completion**: The plan and the certificates are
recomputable, and "some shadow variables were generated" MUST NOT count as
complete exception SSA lowering.

**Not in scope**: Complex cleanup variable merging, dynamic interpretation that
boxes every exception, a dead-copy heuristic on exception paths.

## T18 — Restricted try/catch Java and protection-domain ownership

**Progress**: [Protected-Java batch](batches/t18-protected-java.md) implements
acyclic Sequence/Diamond ownership, ordered and shared catch entries, one nested
layer, same-site snapshots/ingress and precise catch-parameter rethrow. D029,
D030, D031, D032 and D035 pass source-JVM E2E; D034 remains passed. The
[T12/T11 binding repair](batches/t12-inherited-constructor-binding.md) also makes
D033 pass, so the fixed D029–D035 source-JVM group passes. Shared unassigned
handler tails retain classified Debug. The accepted T18 group covers acyclic
protection with one nested layer; T19 adds checked monitor loops through the
same protection forest.

**Dependencies**: T17, T13. **Phase**: P7/P8. **References**: DX-07, KU-07.

**Files**: `src/p7_regions/{protection,verify}.rs`,
`src/p8_java/{structured,declarations,verify}.rs`.

**Input→output**: A single contiguous interval with independent handlers and a
clear normal join → ProtectionCertificate → Java try/catch. A graph outside the
profile stays Debug, and the ordinary dispatcher MUST NOT accept an exception
graph.

**Steps**:
1. Map the exact protected operation set to a Java try, order the catches by the DEX
   chain, catch Throwable first for a catch-all while keeping its code, and never guess
   finally.
2. The handler has an exclusive entry with no ordinary reentry, and join/return/throw are
   expressible; catch unreachability and checked throws follow the same P4 contract.
3. For a pre-throw snapshot, find the position before the source op in the same protection
   domain, and consume all normal exit copies and exception ingress copies.
4. Keep a copied cleanup unchanged on the different paths; the try size MUST NOT expand to
   a normal cleanup or an extra call, which would change the second-catch behavior of an
   exception.
5. Handle one nested layer per ALGORITHMS §10: a throw inside a handler points to the
   outer domain, so reduce the inner try and handlers first and then let the outer domain
   contain them, and then recompute the first matching catch-stack target per site. A
   sibling catch does not catch the handler's own exception. Reject a shared or crossing
   graph that does not match, and never guess finally.

**Invariants / limits**: The original handler selection of every protected site
agrees, throw and return completeness agrees, and a catch cannot goto directly
outside the scope. The definition of a certificate failure as InternalInvariant
or Unsupported MUST distinguish an algorithm BUG from an unsupported shape.

**Tests**: B(F16, F17); F18 `ProtectedControl`: ordered subtype catches, an
unexpected error through catch-all or rethrow, cleanup return or throw, cleanup
throwing itself, one nested level; U03 `ComplexProtection` with normal reentry,
shared handlers or an unknown release needs Debug; M16 a missing edge, an
expanded try, swallowing an exception from outside the call.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F16, F17, F18) plus S(U03). **Completion**: A positive protected method really
runs as Java, a negative retains original debug information, and an incomplete finally recovery does
not affect the real trace.

**Not in scope**: Arbitrary exception ownership, finally beautification on all
paths, a label goto across a try, swallowing an exception so that javac passes.

## T19 — Monitor scope certificate and synchronized

**Progress**: [Monitor batch](batches/t19-monitor-scopes.md) implements finite
reference roots, acquisition-layer stacks, all-exit/freshness replay, canonical
release cleanup and synchronized source through the ordinary chain. D036–D040
and D082 pass their fixed source-JVM invocations; D081 passes classified
raw-integrity acceptance. D040 produces a verified JavaUnit. Supplemental public
core cases cover implicit method locks, declared-only flags and U04 failures;
native mutation cases reject changed locks, releases, cleanup, scopes and loop
edges. The [class-field completeness correction](batches/t19-class-field-completeness.md)
rejects units with unrecovered static or instance fields while preserving their
available Java methods. The batches record full gates and original-member
regression evidence.

**Dependencies**: T18, T12. **Phase**: P2/P7/P8. **References**: DX-07
(synchronization).

**Files**: `src/p7_regions/monitors.rs`, `src/p2_flow/effects.rs`,
`src/p8_java/{structured,declarations,verify}.rs`.

**Input→output**: A declaration flag or an explicit balanced monitor CFG →
MonitorCertificate → Java synchronized; an unknown or mismatched lock state →
Debug, never a fake Java keyword.

**Steps**:
1. Review the raw flag semantics and keep the implicit lock of a synchronized method separate
   from an explicit monitor; a static lock on the Class object and an instance lock on this
   are explicit.
2. Derive a finite reference root per ALGORITHMS §11. For One the root producer MUST also
   lie outside the whole holding scope and MUST NOT re-execute, so the same static
   AllocationId MUST NOT identify one object across iterations. The lock stack agrees at a
   join, an exception on enter pushes nothing, an exit pops exactly and a reentrant acquire
   counts per layer.
3. Check the single-entry scope, the throw timing of a null lock and the release handler
   interval or protected site for agreement; after the match, replace the original enter,
   exit and catch cleanup only through the certificate.
4. Verify a nested balanced profile scope by scope and never skip a monitor on an unproven
   path; a call in the scope body MUST NOT be moved outside synchronized.

**Invariants / limits**: A lock effect is not pure, so no inlining or hoisting;
the certificate has an all-exit walk budget and a loop does not rely on
unbounded enumeration. When stopping cannot be proven, the card stops at that
profile instead of assuming balance.

**Tests**: F19 `MonitorScope`: return/throw/null lock, nested/reentrant, static
and instance method, a `Thread.holdsLock` driver in the body, lock release after
a finally throw; U04 mismatched lock, missing exit, multiple entry → Debug;
negative coverage on native verify. D081/82 fix rejection of a re-executed
dynamic root and acceptance of a parameter root in a loop, and MUST NOT fall
back to raw just because a loop exists.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F19) plus S(U04). **Completion**: An explicit semantic trace, holdsLock and
release check exists (a concurrency stress run only supplements and is not a
complete proof), and raw flags do not print declared synchronized directly.

**Not in scope**: Guessing an alias value across locks, an arbitrary monitor
state, removing "redundant" locks, dynamic verification in a production run.

## T20 — <clinit>/<init>, encoded initial values and final fields

**Progress**: The [initialization batch](batches/t20-initialization.md) adds
original field results, explicit/implicit default preparation, checked source
superclasses and ordinary-chain Java initializer bodies. D041/D042/D044/D046/D047
pass full-unit source-JVM acceptance; D043 remains passed. The
[boolean preparation slice](batches/t20-boolean-preparation.md) admits non-final
static BOOLEAN(true) preparation with independent Java8 compilation and JVM
evidence. The [own final-int slice](batches/t20-final-int.md) verifies D045's
non-delegating natural-diamond constructor with exactly one write on each normal
path. The [static/delegation acceptance](batches/t20-static-delegation.md) adds checked
scalar/String preparation, own-clinit static-final writes, no-repeat this-chain
composition and independently recovered D079 units. T20's supported source
profile passes its fixed and supplemental obligations; unproved initialization
shapes retain classified rejection. The [phase-boundary repair](batches/phase-boundary-repair.md)
collects receiver/constructor declarations in P4 and seals source conditions in
P8 before further initialization work. FinalAssignmentUnproved preserves the field identity and final
modifier in the original facts; D045 passes its fixed JavaUnit/JVM obligations,
and D079 passes independent related-unit compilation and class-init/getstatic
observations.

**Dependencies**: T19, T15. **Phase**: P1/P4/P8. **References**: DX-08/09
(initialization).

**Files**: `src/p8_java/{initializers,declarations,verify}.rs`,
`src/p1_input/values.rs`.

**Input→output**: static encoded values plus clinit and constructor field writes
→ a legal class declaration and initializer plan; when the complete shell cannot
be represented, Mixed keeps the raw field or method, and a final field MUST NOT
get a default and a second write.

**Steps**:
1. Bind encoded and default values per ALGORITHMS §12: emit a static block first for a
   non-final non-default value. For a final field with no original write, assign even the
   default explicitly once; with an original write, accept only default preparation and
   write nothing else. Do not emit a constant field initializer for a static final primitive
   or String, because that would inline sget and remove class-init.
2. Emit clinit as a static block per ALGORITHMS §12, turn return-void into a break of the init
   label, and legalize an always-abrupt body per the established if (true) rule. Do not
   create a `<clinit>` method and do not change the read that triggers initialization.
3. Track write counts per ALGORITHMS §12 with the finite {0, 1, Many} path set, joining by
   union and incrementing on write; every normal construction chain and clinit end MUST
   write exactly once. A this delegation inherits the target write set and emits no repeated
   initial value; an overridden, missing or unknown path MUST NOT be patched with a default
   to make the compile pass.
4. A final field left unassigned without a constructor body MUST NOT be default-initialized
   to make the compile pass; a class shell failure does not affect independent method
   analysis but is reported as Mixed.
5. Take initialization throws and class final modifier or raw visibility evidence from P4
   and metadata, not from printed text.

**Invariants / limits**: Verify that a source-constant final cannot change the
class-init trigger; initialization count, order and failure paths are visible; a
clinit field overwrite loses no origin.

**Tests**: F20 `Initialization`: non-constant static field trace, multiple
constructors, this delegate, blank final, encoded null/false/float bits, init
exception; U05 complex final reassignment or an unprovable default →
Mixed/Debug; B(F20) runs each driver in a new process so class-init has not
already happened. D046 MUST show explicit encoded tags; D079 recovers Case and
Reader separately and compiles them together, checking class-init and the real
getstatic.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F20) plus S(U05). **Completion**: The init trace of a legal unit agrees, a
final that source cannot express gets an explicit declaration status, and the
final contract MUST NOT be deleted to hide the problem.

**Not in scope**: Field initializer beautification, a constructor factory
carrier, interprocedural init optimization, constant propagation.

## T21 — Raw generics, bridge/override and source binding legality

**Progress**: The [interface-unit slice](batches/t21-interface-unit.md) admits
D078's complete 2200-method abstract interface without invented constructors or
pool estimates. The [covariant bridge slice](batches/t21-covariant-bridge.md)
adds D048 Generated ownership, D021 source override binding and the approved
D075 invocation-catalog/context correction; all fixed T21 cases pass, including
D084 rejection. The [raw-signature slice](batches/t21-raw-signature.md) adds checked List<String>
and Object-bounded identity erasure for no-code declarations with retained
original Signature. The [generic-call slice](batches/t21-generic-call.md)
verifies static Object-bound identity binding using existing Object use views.
The [raw-interface acceptance](batches/t21-raw-interface.md) verifies variable-
independent raw parent/interface binding and an evidenced String specialization
with an actual javac-generated parameter-cast bridge. T21's bounded supported
source profiles pass their fixed and supplemental obligations; unproved generic
substitution remains classified Debug/Mixed.

**Dependencies**: T20, T11. **Phase**: P0/P4/P5/P8. **References**: DX-08 and
the signature/source ABI questions.

**Files**: `src/p4_calls/{contracts,members}.rs`,
`src/p0_knowledge/declarations.rs`,
`src/p8_java/{declarations,expressions,verify}.rs`.

**Input→output**: Signature metadata MAY stay raw without recovering `List<T>`;
an erased declaration, override and call binding succeeds only when all Java
evidence passes, and a return-only collision MUST NOT rename externals
arbitrarily.

**Steps**:
1. Review the difference between the DEX descriptor and the Signature and keep the original
   metadata record; a raw declaration does not change the binary member proto, and a typed
   cast adds no runtime check.
2. Use `MemberEmission::Generated` of ALGORITHMS §12: exact override facts MUST match the
   canonical moves, cast, target call and return, and the source MUST generate a bridge with
   the same signature and behavior before the card suppresses the duplicate print. Generated
   still has code and an original MethodKey and does not count as DeclarationOnly; a bridge
   that a standalone request cannot express returns the original Debug.
3. A Java interface or override implementation keeps its return, throws and visibility
   consistent with the raw contract, and a generic parent member binding MUST NOT be guessed
   from the local descriptor alone.
4. Fall back for the affected methods under an incomplete catalog, and never let a
   missing-type exception "ignore javac errors" and become the contract.

**Invariants / limits**: Pure generic beautification is optional, binding
correctness is not; the overload-complete status and EnvironmentId stay stable,
and source public identity is unchanged.

**Tests**: F21 `ErasedContracts`: self-made generic List/identity, covariant
bridge, raw interface impl, null overload; U06 a return-only DEX overload, a
differing bridge effect, an unknown parent contract → Debug/Mixed; B(F21) with a
matching complete self-made classpath. D048/D084 require a canonical Generated
bridge and rejection of an effectful bridge respectively; D075 includes the
target boundary of a code-free declaration, and D078 requires 2200 abstract
declarations to form a complete JavaUnit without rejection by a pool estimate.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F21) plus S(U06).
**Completion**: A raw generic warning is acceptable but a real compile error is
not ignorable; the legacy full type solver is not imported wholesale.

**Not in scope**: A complete GenericTypeSolver/variance/inference
beautification, general wildcard projection, arbitrary sneaky throw or externals
renaming.

## T22 — Named nested classes, capture and unit ownership

**Progress**: Accepted for the [named ownership profile](batches/t22-ownership.md).
F22/U07 and both D049 source-JVM invocations pass. Public regressions cover static
and captured inner classes, multilevel same-name ownership, Unicode spans,
private access through the original accessor, metadata/capture mutations and
configured resource failures. Original class/method/field identities remain
traceable; a standalone inner fragment stays Mixed.

**Dependencies**: T21, T12. **Phase**: P1/P4/P8. **References**: DX-09/12,
RG-04.

**Files**: `src/p8_java/ownership.rs`, `src/p1_input/catalog.rs`,
`src/p8_java/declarations.rs`, `src/p9_emit/spans.rs`.

**Input→output**: Reliable Enclosing/InnerClass/MemberClasses metadata plus
declarations → named nested Java or Mixed; anonymous-class source sugar is not
required, and an illegal ownership or missing dependency MUST NOT silently drop
the whole class.

**Steps**:
1. Verify the metadata ownership graph for cycles and duplicates; a `$` name is only a hint
   and does not prove an inner class on its own.
2. Establish explicit static or instance outer binding, constructor capture fields and private
   access facts; keep the original MethodRef and FieldRef in a sidecar mapping.
3. Implement a legal nested name first; an anonymous original MAY be reported as a synthetic
   named class, but access, capture and constructor semantics MUST agree; an inexpressible
   declaration keeps original debug information.
4. Reject a package, import, name or visibility conflict in the unit and a top-level static
   class; route a single inner fragment into a Mixed context and do not impersonate a
   JavaUnit.
5. Relocate final spans consistently and keep Unicode UTF-8 boundaries and the original class
   key traceable.

**Invariants / limits**: Nested closure dependency, depth and output have a
request budget; `MemberClasses` MUST NOT miss a member and a missing inner class
is explicit; an internal rename cannot change the external binding contract.

**Tests**: F22 `NamedNested`: static nested, non-static outer capture, the same
simple name, Unicode identifier, private access; U07 metadata cycle, missing
inner, ambiguous ownership → correctly classified Malformed/Unsupported; B(F22)
with a real classpath compile of the whole unit and a capture trace.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F22) plus native ownership and span mutation tests. **Completion**: The
top-level unit is legal, the original identity mapping is complete, and the
single-fragment status is correct.

## T23 — Restricted enum/annotation and complete shell classification

**Progress**: The restricted profile passes its recorded acceptance. The
[canonical enum batch](batches/t23-enum.md) passes D050 and public
generation/order/clone/valueOf/escape/custom-body regressions. Generated members
retain original facts, raw snapshots and checked class-owner spans. The approved
[annotation batch](batches/t23-annotations.md) passes D051, annotation interface
source and checked defaults, including independent reflection/array/UTF-16
observations on recovered definitions and uses. Its original class/member/parameter
metadata sidecar retains local damage without discarding independent code;
unrepresented metadata prevents a complete unit. G0 passes with 187 real tests
and zero skips. Full E2E is 79/86 cases and 156/181 seams; the existing application
failures remain open and are not an EngineCoreRC pass.

**Dependencies**: T22, T20. **Phase**: P1/P4/P8/P9. **References**: DX-08/09,
RG-09.

**Files**: `src/p8_java/{enums,declarations,verify}.rs`,
`src/p1_input/values.rs`, `src/p9_emit/literals.rs`.

**Input→output**: A canonical self-made enum, an annotation interface and
runtime-visible values → a legal JavaUnit; a shell that cannot express its
identity, initializer or metadata → Mixed, while an independent method may still
be Java.

**Steps**:
1. Verify the enum constant field, order, name and ordinal, the canonical constructor
   implicit Enum parameters and the values/valueOf contract item by item; the source MUST be
   a Java enum and not an ordinary class extending Enum.
2. Do not delete an arbitrary synthetic method; a canonical generated member needs complete
   behavior evidence, and a multi-constant body or extra effect stays Unsupported.
3. Really check the annotation element type, default, Retention, Target and user annotation
   literal; treat system Signature and InnerClass as metadata and do not fake `@Signature`
   Java.
4. Keep raw metadata in a sidecar, but a missing runtime annotation MUST NOT claim identical
   full-class semantics; when policy allows an inspection-only Java fragment, Mixed marks it
   as not a complete unit.
5. The status assembler verifies that any Debug, Unavailable or unsupported declaration makes
   the result Mixed; an error counter of 0 MUST NOT be taken as a JavaUnit.

**Invariants / limits**: Metadata full source differs from inspection only;
annotation array and string UTF-16 limits are recursively bounded; a canonical
enum missing one constant or initializer is rejected.

**Tests**: F23 `EnumAnnotation`: enum iteration/name/ordinal/values clone,
canonical valueOf, runtime annotation reflect/default/array/UTF-16; U08 an
unprovable enum custom body, an invalid annotation element, missing retention
facts → Mixed; B(F23) accepts the canonical profile only in this card. D050 uses
non-lexicographic constants and checks valueOf so that field-table ordering
cannot coincidentally satisfy the expectation.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F23) plus S(U08) on its legal raw part. **Completion**: The supported profile
is recomputable, the JavaUnit classification matches the metadata semantics
contract, and no `extends Enum` pseudo-legal code exists.

**Not in scope**: Full enum-switch desugaring, a large annotation
platform-library catalog, complex enum constant bodies, a reflective-name
rewrite promise.

## T24 — Optional pure-expression recovery (not required core)

**Dependencies**: T23, T14. **Phase**: P8/Pipeline/P0. **References**:
KU-04/05/08, DX-06.

**Files**: `src/p8_java/recovery.rs`, `src/pipeline/schedule.rs` (actual wiring
only), `src/p0_knowledge/policy.rs`.

**Input→output**: verified JavaBody → the single-use integer inlining of
ALGORITHMS §15 → verified again. `inline_pure` defaults to false, and turning it
off does not affect CoreRC.

**Steps**:
1. Implement one real optional pure function with an explicit option in P8 only, default off;
   create no PassSpec or registry. Handle it per the Applied/Declined/BUG contract of
   ALGORITHMS §15, build no plugin framework, and keep required legalization non-optional.
2. Match only an adjacent `t = expr; return t` in the same block; expr contains only the
   specified pure int/long operations and none of FP, div, rem, heap, call, ref cast or
   allocation or synchronization, and the card MUST NOT inline a Phi, shadow or deferred
   allocation.
3. Check the resources needed for the copy, the verification and the complete emit against
   an explicit finite quota only, with no estimated rejection threshold by default. Copy the
   JavaBody, scan once in the original AST order and replace non-overlapping pairs, without a
   repeat pass or a rollback edit log.
4. Verify source type, binding, control, effects, Origin and the quota. Report Applied on
   success, Declined on a mismatch or an insufficient quota, and stop on cancellation; an
   internal error is a BUG.

**Invariants / limits**: No effect duplication or motion, and post-verified
types stay valid; the default-off and default-on runs each keep the same profile
behavior; a local count or text shape MUST NOT become a correctness criterion.
ALGORITHMS §15 governs the complete accept and reject conditions of integer
inlining.

**Tests**: F24 `PureRecovery`: a pure-copy inline positive and
call/read/div/monitor negatives; B(F24) with on and off drivers agreeing, a
mutation with a duplicated effect failing, and the budget never disabling
required copy lowering.

**Acceptance**: G0 plus this card's public core cases (unified rules in §4);
B(F24).
**Completion**: The small option is really consumed, its trace is visible, a
failure keeps the baseline, and the card does not affect the T25 core gate.

**Not in scope**: Constant-branch-to-CFG conversion, GVN/LICM, StringBuilder or
finally recovery, a global restart, a registry and persistent HARD assertions.

## T25 — Fixed core-set hardening and EngineCoreRC

**Progress**: The approved [rendered-integrity batch](batches/t25-rendered-integrity.md)
passes both D085 engine-render invocations with checked original call-site/handle
facts, InvokeCustomUnsupported and retained Java/raw members and spans. Workspace
nextest passes 190 tests, zero skips; full E2E is 158/181 seams and 79/86 cases.
D085 still fails its required application seam. The
[artifact/replay batch](batches/t25-artifact-replay.md) retains actual compiled
Code/handler/pool observations and repeats all required invocations with different
worker counts and hash seeds without identity/status regressions. Polymorphic
invocations now check the prototype reference and stay raw-only with a specific
reason. The [engine-gate rollup](batches/t25-core-rc.md) records 156/156 required
engine seams accepted, the known-unsupported raw coverage, the seeded mutation
matrix and the source/license review. At commit b39805f the clean revision accepts
156/156 required engine seams across 80 cases, so EngineCoreRC is accepted for the
`darwin-aarch64` self-made suite. ProductRC remains open for the application seams.

**Completion**: EngineCoreRC accepted at b39805f; the card's remaining work is the
application-owned ProductRC rollup in H08, not an engine obligation.

**Dependencies**: T23, T14. **Phase**: all/verification. T24 is optional
beautification and is not a prerequisite of this card. **References**: KU-04/08,
RG-08, DX-13.

**Files**: `tests/contracts.rs`, the thin public adapter,
`src/pipeline/control.rs` and the actual core validators; read the root
`tests/e2e/manifest.json`, cases, expected values and input certificates; a new
crate README source register and the implementation status of this design. Each
crate MUST NOT copy a second mutable fixture or expected value.

**Input→output**: The actual supported profiles of T01–T23 → the CoreRC evidence
set: a positive, negative, limit and status per capability, behavior drivers and
raw fallbacks, no panic and mutation coverage, identity and content completeness
of unordered output, and a source license review. This does not declare that
every DEX is Java.

**Steps**:
1. Check the manifest fixed by B01–B04 against the actual typed transitions; a required
   member MUST NOT be deleted or skipped because of the current implementation, and
   turning a core-Java failure into a Debug success is unacceptable. Assert that no filter
   reduces the required set to zero; state the unsupported lists for known 038/039, custom
   and polymorphic cases and their raw coverage separately; an unknown format MUST NOT fake
   a fallback.
2. Generate byte and artifact mutation tests for count, offset, LEB, payload, handler, type,
   Phi, copy, site, loop, state and output budget with a fixed seed, where cancellation
   terminates. Target verification covers static 255/256, instance 254/255, wide parameter
   words, MUTF-8 and compiler-owned layout; D077/D078 MUST be Java and D086 checks the
   default-unlimited and explicit-limit contrast. Rejection MUST NOT use an estimated cost,
   and a real JVM format boundary MUST NOT become a configurable resource budget.
3. Every known-unsupported legal fixture gets an S gate; a Malformed Unavailable or raw
   excerpt does not impersonate decoded instructions, and a BUG does not count as success. A fallback policy
   that swallows an exception or type error MUST fail its counterexample.
4. V4 compiles with Temurin 21.0.8+9 `--release 8 -proc:none -g:none`, reads the classfile
   independently and records the actual code, locals, stack, handler and pool, checking the
   real format limits and member layout instead of a guessed projection; javac exit 0 alone
   is not checked. This check verifies core generated artifacts, not tooling self-tests. Run
   every required invocation whose JSON `acceptanceGate` is `engine` or
   `engine-and-application`, including D071–D086, without filtering by current support or by
   the old D065 endpoint. H cards add application evidence, and H08 rolls up every required
   product invocation. List JVM, ART and specification-mirror, catalog, metadata and exception
   or lock evidence separately. T24 off runs the whole core gate first, and T24 on is verified
   separately as implemented beautification only.
5. Repeat verification with different worker arrival orders and hash seeds: records and
   artifacts stay linked by original identity, with no duplication and no loss, each source is
   actually legal and each span points into this text. Generated text, local names, declarations
   and diagnostic sequences need not be byte-identical, and the test adds no canonical sort.
   Different legal schedules that trigger different budget results are classified honestly, but
   they MUST NOT cause wrong semantics; this card does not deliberately create parallelism.
6. Record release elapsed time and peak memory with the method stated, rather than a debug
   assertion that something is fast; a representative input runs opt-in after authorization,
   and environment and skip status are explicit. Make no promised percentage reduction.
7. Apply this repository's workspace gates, per-crate license sources and self-contained
   constraints; this card accepts only the internal EngineCoreRC, the root CLI replacement is
   accepted by H05/H08, and no MCP is designed.

**Invariants / limits**: A hit on an explicit quota or an actual allocation
failure is BudgetExceeded, not Malformed; a directly confirmed target format overrun
is Unsupported and is not rejected by an estimate. All Limits default to None,
an explicit request quota covers several methods, and a small limit MUST NOT
truncate into success. A fuzz case has a timeout, seed and range to avoid an
unbounded run.

**Tests**: F25 `HardeningMatrix`: every M/U/F is registered in the manifest,
actual artifacts fail, cancel versus fallback, near-boundary allocation and
report spans; when result ordering or synthetic IDs vary, the check uses
original identity and does not require full text equality across runs. A
known-unsupported extension is raw only and has a version restriction.

**Acceptance**: G0, the public core `t25_` behavior,
Java/raw/input/resource/041/target-boundary evidence for every required
invocation of the JSON engine gate, and the four root gates. Application domain
and extract checks do not become engine development dependencies, and H08
accepts every required product invocation. The selected members do not change
with implementation status, and §4 lists the run commands.

**Completion**: A traceable EngineCoreRC report where every core obligation and
invocation listed in the JSON is accepted against independent expectations; the
product ledger still shows every required item, with no empty or skipped item
and no Java-to-Debug answer change. While a counterexample is unfixed, the
capability stays Unsupported and the required item still does not pass, so the
author MUST NOT declare CoreRC.

**Not in scope**: A production engine switch, version tags or releases,
automatic private-corpus downloads, unmeasured cache or parallelism, fixing a
comparison score with a normalizer.

## 6. Application tasks H01–H10

## H01 — Product naming and domain contract freeze

**Dependencies**: B04. **Scope**: Diana application branding, CLI and
internal-crate positioning; no new MCP design.

**Files**: the existing `apps/diana/Cargo.toml`/`src/main.rs` scaffold plus
`apps/diana/src/lib.rs` and `src/cli/{args,mod}.rs` (all relative to the Diana
application root); update the README/AGENTS product description. The old rasc
tree already moved entirely to ref-fork and is reference-only, never built or
tested; the historical release and license text is not replaced globally; this
card creates no CLI test file.

**Input→output / Interface**: the application package and binary `diana`
(`apps/diana/`); the internal package `diana-decomp`, path
`crates/diana-decomp/` and import `diana_decomp`; the original CLI domain,
flags, exit codes and result completeness form the baseline. Output order is
explicitly no longer guaranteed, and independent task results publish in
completion or arrival order. A rename does not mean the new algorithm has
replaced the old one.

**Steps**:
1. Register every live consumer of the name: the clap name, crate imports, test binary
   environment, skill, release artifact and version assertions. Register MCP items only as
   deferred or as necessary compile references, and design no server-name or schema
   migration.
2. Check the commands, fields and output channels against the existing source and DESIGN.md
   and the migration list; do not run CLI black-box checks or establish help or format
   goldens. The old display order and pagination interfaces are deliberately cancelled; the
   corresponding module behavior tests carry complex domain obligations such as original
   identity, decode completeness and file safety.
3. Implement the new product in the `apps/diana/` entry point and the internal-crate
   scaffold; the root is already a Diana-only workspace. The old rasc stays in ref-fork and
   its CLI MUST NOT serve as an alias or dependency. H08 still needs product capability
   evidence, the old release tooling is archived, the Diana release policy is reviewed
   separately, and historical sources are not replaced wholesale.
4. Freeze the 11 commands per DESIGN.md, with their specified TSV/JSON columns, canonical
   `at`, complete method selector, default decompilation display and the JSON representation
   of the same result; the old getclass/findrefs/skill, class-id, member prefix and
   require-java keep no alias. H09/H10 implement the new fields, methods, xref sites and
   extract respectively, and the old interface MUST NOT constrain the new design.

**Invariants / limits**: The active workspace contains Diana only, and the old
rasc is fully archived and never built or tested; Diana forwards to no old CLI
or engine; there is no JVM or Python production runtime; the original copyright
and sources are unchanged. A baseline number MUST NOT be invented without
execution output.

**Tests**: No automated test is added. Q01 is a manual review checklist for
naming and the basic interface, not a test family; help, flags, column names,
alias and pagination removal are checked against DESIGN.md, with no CLI
black-box or golden. The matching domain task tests complex identity, decoding
and file safety.

**Acceptance**: Format, lint and build checks plus the recorded Q01 manual
review, with no `h01_` test filter. **Completion**: The package name and the
eleven-command contract match the implementation, unimplemented capability is
marked, and no basic CLI test is required.

**Not in scope**: MCP schema or server identity migration, rewriting
archive/query, creating a release or tag, changing the version, global string
replacement.

## H02 — Safe input and the CLI request scope

**Progress**: The [safe-input batch](batches/h02-safe-input.md) implements the
immutable input lease, restricted ZIP entry metadata, stored and deflate payload
reads under explicit bounds, root DEX selection with path, budget, damage and
snapshot-isolation evidence, and the per-request scope that owns the lease. The CLI
argument surface and `extract` publication remain with H05/H10.

**Dependencies**: H01. **Scope**: root input lifetime.

**Files**: `src/analysis/{archive,apk}.rs`, referencing the snapshot and
AllowedRoot of `src/analysis/session/mod.rs`; add `src/analysis/input.rs`,
`request.rs` and the complex domain test `tests/product_contracts.rs` as needed,
and do not force reuse of session types that carry protocol dependencies.

**Input→output / Interface**: an ordinary path → `InputLease` (a private
immutable byte owner, the input-scope identity and restricted entry metadata) →
borrowed views within the lifetime. The CLI adds no open, close or status
command, and the lease releases at request end.

**Steps**:
1. Reuse the bounded-copy, regular-file and check-before-after approach; state the allowed
   path and symlink policy explicitly, permitting a normal file and rejecting a
   non-regular device or FIFO.
2. Check the input and directory budgets before reading, allocating or mapping; a source
   truncated or replaced after the snapshot is created does not affect the lease.
3. A unified immutable CLI snapshot may add disk-copy cost; record the intent and measure
   it on a release build, and do not claim byte-identical performance.
4. Provide `BytesSource` and borrowed entries inside the scope using a safe-Rust owned
   snapshot or the safe interface of a reviewed dependency, and never let a borrow escape
   its owner. **Both active packages set `unsafe_code = forbid`**, so the input module gets
   no exception; an mmap approach that needs unsafe calls is not adopted. If the platform
   file primitive H02/H10 needs has no safe interface that meets the contract, review the
   dependency and platform-support decision first and do not substitute a race window or a
   weakened safety contract.
5. A terminated request or an output error drops every temporary-file owner and MUST NOT
   delete the user's input.

**Invariants / limits**: Lengths and offsets are checked, a copy that changes is
INPUT_CHANGED; no unsafe `'static` extension; temporary files are cleaned per
scope and never faked into a cross-request cache.

**Tests**: Q02 ordinary file, bare DEX and stored ZIP; counterexamples for the
truncation race, symlink escape (limited-root mode), FIFO, an over-budget input
or directory and an early owner drop; the test temporary directory never touches
real user files.

**Acceptance**: G0 plus `cargo test -p diana --test product_contracts h02_`.
**Completion**: The source change is isolated, cleanup is correct, and the
budget-first behavior and the borrow boundary are testable; the cost record
invents no gain.

**Not in scope**: A long-term target protocol, MCP open or schema, an
all-platform file database, rewriting the ZIP codec for the snapshot.

## H03 — Lightweight query, manifest and completion-order output

**Progress**: The [query-core batch](batches/h03-query.md) adds the approved
read-only P1 facts (`DexString::units`, `Prototype::visit_parameters`,
`ClassView::visit_fields`, `string_ids_len`) and streams `entries`/`classes`/
`methods`/`fields`/`strings` rows in completion order with explicit bounds,
cancellation and original identity. The [xref batch](batches/h03-xref.md) adds
instruction-level `string/type/method/field` sites with a `use` mnemonic column and
the direct `--extends`/`--implements`/`--subinterface`/`--kind` filters on
`classes`. The [AXML batch](batches/h03-axml.md) adds a bounded binary-manifest
reader and last-physical-entry selection. It is validated on self-made fixtures
and five authorized local APKs whose element sets, attribute names and attribute
values match both a self-written chunk decoder and `androguard` 4.1.4. A seeded
mutation campaign (6000 runs, no panic or hang) and committed mutation/style/text
regressions cover structural robustness. Damaged documents now return balanced
partial XML with byte-offset warnings, as DESIGN §6.7 records. The
[CLI batch](batches/h03-cli.md) wires `entries`, `manifest`, `classes`, `fields`,
`methods` and `strings` with their columns, canonical `at`, JSON shapes, exit
codes, `-o` atomic publication and warning channels; `xref` belongs to H09 and
`extract` to H10, so this card's query, manifest and presentation obligations are
complete.

**Dependencies**: H02. **Scope**: the existing non-decompilation CLI capability.

**Files**: `src/analysis/{apk,archive,query,manifest}.rs`, `src/analysis/dex/`,
`src/cli/mod.rs`; the existing scanner and unit fixtures are reusable, and a
vendored AXML change needs PATCHES first.

**Input→output / Interface**: InputLease → classes/strings/entries/manifest plus
the symbol and reference scan foundation that H09 reuses, without going through
the Engine or SSA. The columns DESIGN.md specifies are the new output contract;
the old scanner only supplies the read and lookup algorithms, and the old
method-level findrefs result MUST NOT be copied as an xref site.

**Steps**:
1. Wire the new scope onto the H01 baseline and keep root DEX selection, the duplicate-entry
   rule, the last-manifest selection and the real physical, logical and row identity; display
   order is not guaranteed, and the last manifest or a duplicate class definition MUST NOT be
   chosen by whichever finished first.
2. Keep the ASCII and raw-substring prefilter with its non-ASCII fallback and the exact or
   fuzzy domain query; the displayed string may differ from the engine's exact UTF-16 literal
   representation, so each contract is explicit.
3. Workers submit a row or batch in completion or arrival order, so remove the wait by entry
   ordinal, the full sort, the reorder buffer and the lead window that only preserves input
   order. Keep weighted-resource admission, bounded byte and row queues, backpressure and
   cancellation; there is no unbounded parallel collect. Different threads only need the same
   complete record multiset, not the same sequence.
4. A filter keeps its match semantics and returns every match; delete the user pagination
   arguments with their skip/take, truncation count and pagination early-stop logic, and do
   not substitute a cursor or preview. Keep error, cancellation and safety-budget stops and
   the internal bounded batch (a batch is not user pagination). A stored entry is borrowed and
   deflate decompresses on demand; a query builds no full ProgramIndex and no decompilation
   IR.
5. Regress fields, diagnostic category and exit; do not add a sort or stable mode for the
   old display sequence, record this deliberate change and measure its release cost, and
   never invent a claim that unordered output is necessarily faster.

**Invariants / limits**: The definition and reference indexes of the same input
correspond to the same entry, and a successful query returns every matching
record with no duplication or loss; no Python or external JVM; the query budget
and the engine budget are not mixed. Removing user count truncation does not
remove the safety limit; an over-budget query MUST fail rather than return the
first N rows and mark success.

**Tests**: Q03 verifies multi-DEX, duplicate entries, duplicate manifests,
UTF-16 matching, concurrent record completeness and resource errors through the
public query module interface. A controlled delayed producer confirms that there
is no ordinal wait and that the buffer is bounded, comparing content by original
identity; the test does not check CLI flags, table headers, basic filter routing
or empty-list printing, and does not assert private phase call counts.

**Acceptance**: G0 plus `cargo test -p diana --test product_contracts h03_` plus
input and decompression regression through the Diana public domain interface.
The old self-contained and inflate policy serve only as a review question;
ref-fork tests are not run and their pass counts are not part of acceptance.
**Completion**: The new CLI columns return every matching record; reading,
matching and original-identity completeness hold, and output order and user
pagination are deliberately cancelled; a slow entry does not block unrelated
results, and the fast path needs no new engine.

**Not in scope**: Merging every DEX parser, an MCP general pagination protocol,
rewriting AXML, claiming a speedup after measuring a debug build.

## H04 — Raw DexImage, 041 and definition-identity adapter

**Progress**: The [image and locator batch](batches/h04-images.md) parses the
canonical and short selectors, maps roots to engine images including 041 logical
members, and validates a locator's descriptor/name/proto before use. The engine
loop for a selected object belongs to H05.

**Dependencies**: H03, T04. **Scope**: the root → internal-engine input
interface.

**Files**: `src/analysis/dex/container.rs`, the selected-class and engine
adapter of the root `analysis`; the native 041 input interface plus origin and
keys already provided by T04. This card no longer carries the engine parser
implementation.

**Input→output / Interface**:
`SelectedClass{scope,entry,logical,classdef,descriptor}` → `Vec<DexImage<'a>>`,
ClassKey/MethodSelector and a bounded DeclarationCatalog, with the result
identity mapped back to the root's original input.

**Steps**:
1. Parse class-at, method-at, field-at and string-at per DESIGN.md and check the physical,
   logical, class-def and member index against the descriptor. When a readable get-class
   name has several definitions, report it explicitly as ambiguous; the card MUST NOT feed
   the engine and then pick arbitrarily by name. Distinguish an illegal `at` syntax from an
   invalid index, and map a bare DEX to `e0:l0`.
2. Read native 041 through T04 and pass only the borrowed original container plus the logical
   `header_offset`, mapping the original ImageOrigin to the physical entry and logical member.
   This card MUST NOT overlay or copy the container per member.
3. P1 and the Engine already adopt a borrowed view per DESIGN §3. The H02 lease obtains the
   necessary bytes first, then the same scope creates the RunContext and calls
   open/decompile/render; Report is an owned result and no temporary IR borrow is returned.
4. Locate a dependency declaration precisely with the scanner and generate facts within the
   dependency byte and metadata budget; report Unknown when completeness is insufficient.
   Do not parse the whole APK automatically or trust the lenient scanner validation as the
   engine validator.
5. The fields/methods output `at` and the get-class/get-method selection come from the same
   original SelectedClass or MemberRef; an original runtime index does not change with a Java
   rename, and there is no `--members` table prefix.

**Invariants / limits**: Container offsets, original sites and UTF-8 span
coordinates are not mixed; native 041 support is input support and does not
imply support for every newer opcode; a case beyond the original raw profile is
explicitly Unsupported and MUST NOT be overlaid through.

**Tests**: Q04 verifies several logical 041 members, a duplicate definition or
physical entry, an identity that expires across inputs and a descriptor or index
mismatch through the public input and definition-parsing interface, and checks
the original definition and proto against the unmodified raw bytes. The test
does not check CLI selector argument syntax or string formatting.

**Acceptance**: G0 plus `cargo test -p diana --test product_contracts h04_` plus
the engine T02–T04 regression. **Completion**: Borrowing, identity, native 041
and the raw fallback are complete, and the scanner being able to list something
MUST NOT be equated with engine support.

**Not in scope**: A full production-parser cache, a permanent cross-file ID,
switching the default to the old engine, MCP class-id returning the new schema.

## H05 — CLI wiring and the semantic loop of the public decompilation interface

**Progress**: The [decompile batch](batches/h05-decompile.md) wires
`get-class`/`get-method` through the shared engine chain with canonical locators,
Mixed/Debug preservation, `at`-mapped spans and JSON schema 1. The CLI still opens
an empty declaration catalog; assembling it from the request's own declarations
remains open.

**Dependencies**: H04, T06. **Scope**: early CLI wiring that does not wait for
the whole engine.

**Files**: the new engine adapter of the root `analysis`,
`src/cli/{args,mod}.rs`, `tests/product_contracts.rs` and the relevant fixtures.

**Input→output / Interface**: a canonical `get-class`/`get-method` selector →
Engine Report → default decompilation text or the JSON representation of the
same result. By default the CLI delivers the existing
Java/Mixed/Debug/Unavailable content directly, and a local failure does not
clear the other members; there is no old-command alias and no dual engine.

**Steps**:
1. After the CLI locates the class, exact method and proto, the borrowed engine runs the T06
   fixture, with callback and control provided by the scope.
2. Known-context validation of a single-method source follows TASKS §4, and a Mixed class
   MUST NOT be called a JavaUnit; a method that does not meet the Java representation
   constraints keeps raw plus its language marker.
3. Display the selected result directly per ALGORITHMS §8.1: a JavaUnit as complete Java, and
   Mixed keeping the shell, the successful Java and the raw or reason of the failing method;
   the Debug or Unavailable of get-method is equally visible. Text and JSON do not change
   algorithm eligibility, and no `--require-java` exists; a hard error keeps its nonzero
   status, and an existing fully renderable Report is not discarded because of it.
4. Keep the complete method owner/name/proto or method-at per DESIGN.md, with Mixed carrying
   the per-method language, reason and original identity, and extra logging on stderr; the
   stdout and file payload of the same run agree; JSON does not serialize the private arena,
   and no MCP envelope is designed.
5. Keep the old implementation in ref-fork for reading only, leaving no active old entry
   point, dependency or dual-engine routing. A Diana request MUST NOT silently fall back to
   dexdec; a failure follows only the original original DEX debug contract.

**Invariants / limits**: The request source and method proto are exact and the
output status is truthful, with language eligibility separated from full display
and hard errors; no stdio pollution, and no stub-based pass. A missing catalog
or representation limit MUST be marked locally and MUST NOT escalate into
suppressing display for the whole class by default.

**Tests**: Q05 runs F05 → recovered Java → javac → behavior Driver through the
public decompilation interface and verifies the actual return and semantics, the
original identity and degradation completeness. F10/Q04 cover complex overloads
and cross-DEX; D085 additionally checks that default display keeps Java and the
invoke-custom raw, the complete UTF-16 and call site, and the real spans; the
whole case is accepted once the format facts T04 needs are ready. The test
starts no get-class/get-method process and does not check flag routing, a
getter, JSON wrapping or not-found text.

**Acceptance**: G0 plus `cargo test -p diana --test product_contracts h05_` plus
engine B(F05). CLI wiring is source-reviewed and gets no black-box test.
**Completion**: The new module passes the real semantic loop and the CLI already
calls that interface rather than merely linking the new crate.

**Not in scope**: A new MCP getclass tool, a throwing stub, forcing Java for an
unverified shape, immediately deleting every old dependency.

## H06 — Request cancellation, layered budgets and output publication

**Progress**: The [limits batch](batches/h06-limits.md) maps explicit CLI options to
the input and engine ledgers, threads a cooperative cancellation flag through one
`RunContext`, and checks each delivered payload on its actual bytes before atomic
publication. SIGINT and per-request caching are not claimed.

**Dependencies**: H05, T09. **Scope**: CLI, control and engine output.

**Files**: the root input, request, engine adapter and CLI emit;
`crates/diana-decomp/src/pipeline/control.rs`; tests for private input,
interrupt and backpressure.

**Input→output / Interface**: caller stop or UserInterrupt → RunContext →
pipeline checkpoints; per-request byte, work and output limits → the same
RunContext; complete text or JSON, or an explicit BudgetExceeded or Cancel error.

**Steps**:
1. The cancellation callback MUST NOT introduce tokio into the engine; check it at every
   decode, worklist, copy, region and emit checkpoint, and Cancelled does not continue into
   the raw fallback.
2. After the user file or scope ends, no stale result is emitted; the owner and reservation of
   the same request's cache loader or reader are released, a failure leaves no half cache, and
   no hard timeout is promised.
3. When creating the RunContext, combine only the explicitly set Limits with the remaining
   scope allowance; an unset limit stays None, and no fixed input, inflate or JSON threshold is
   added. Deduplicate a dependency and a main declaration by the same physical owner, charge
   derived data, all methods and render cumulatively, and let only an explicit finite raw quota
   participate in the parent reservation.
4. Check output separately against `max_method_output_bytes`, the independent
   `fallback_output_bytes` and `max_report_output_bytes` of DESIGN §10.3; P9 sums the shell,
   members, markers and diagnostics, and JSON checks the length after the actual escaping,
   repeated fields and spans per DESIGN §7. Output length MUST NOT be charged twice as an
   allocation, and real work or allocation is never refunded; an overrun is never a truncated
   success, and the file publishes atomically and safely. D064 fixes the member overrun and the
   report overrun separately.
5. A broken pipe in a CLI stream query stops the producer and backpressures; a SIGPIPE that
   kills the process MUST NOT be called a catchable cancellation guarantee.

**Invariants / limits**: Cancellation is cooperative and the non-preemptible
libdeflate is explicit; by default input, inflate, JSON and engine have no
numeric quota, and BudgetExceeded is reported only on an explicit overrun or a real
allocation failure; there is no new MCP semaphore or schema contract.

**Tests**: Q06 verifies long-work cancellation, owner lifetime, normal/raw
budget isolation, the output byte budget for heavily escaped text, spans
pointing into the actual source, a cache failure reservation and a bounded queue
when the consumer closes, through the public request, decompilation and
extraction interface. It is not a simple JSON field wrapping or private phase
call-order test, and each case has a finite timeout.

**Acceptance**: G0 plus `cargo test -p diana --test product_contracts h06_` plus
the T09/T25 control regression. **Completion**: The exact output limit and the
cancellation state have non-empty execution evidence, and the card MUST NOT
build a huge String first and then claim the allocation budget is checked in
advance.

**Not in scope**: MCP close/status/permit design, a crash-isolated subprocess,
arbitrary real-time preemption, a cross-process cache service.

## H07 — Unified help, output and agent documentation for the eleven commands

**Progress**: The [help batch](batches/h07-help.md) freezes the ten product commands
plus `help`, their columns, `at` grammar, JSON shapes, exit codes and the `extract`
exception in the binary help and the package README, which also carries the agent
usage text. Q07 was a manual review; no CLI golden test was added.

**Dependencies**: H06, H09, H10. **Scope**: CLI consumers and usage text.

**Files**: `src/cli/{args,mod,agent_skill}.rs`, `skill/SKILL.md`, README; the
historical sources stay. This card adds no CLI test, help snapshot or
argument-routing test.

**Input→output / Interface**: the 11 commands, specified columns, canonical
`at`, default decompilation text and the same-result JSON, and the extract
exception, all formally frozen; the agent usage text is an ordinary document and
release asset, and no public skill-install command is provided. MCP stays
deferred.

**Steps**:
1. Help lists exactly get-class, entries, extract, strings, xref, manifest, classes,
   get-method, fields, methods and help, with the specified descriptions and columns; the
   TSV header and escaping and the JSON domain fields follow DESIGN.md, and no old member
   prefix is added. The complete method selector contains name and proto instead of the old
   separate `--method`/`--proto` pair, and `at` can be passed back.
2. A complete Mixed or Debug display of a local default Unsupported may exit 0 exactly like
   JSON without becoming a JavaUnit; usage errors exit 2, a missing single object exits 3,
   ambiguity exits 4 and an empty query set exits 0. Malformed, BUG, BudgetExceeded, Cancel and I/O
   still fail, and an existing fully renderable result is kept with a nonzero exit. The old
   getclass, findrefs, skill, require-java, class-id, members and pagination flags are
   explicitly rejected.
3. The agent documentation switches to the new Diana commands and explains the fields, `at`,
   JSON, the one explicit external d8 conversion and its reuse; it MUST NOT move or delete a
   user-installed rasc skill automatically, and adds no twelfth skill command.
4. Stdout, file, help, version and the Windows binary target are complete; the JSON `version`
   field is a local CLI report and no MCP is designed.
5. Check the rename list of every live CLI brand consumer; a legacy release or provenance item
   is not a remaining bug.

**Invariants / limits**: An unknown flag MUST NOT be silently ignored; a
JavaUnit stays pure Java, Mixed MUST mark language and reason explicitly and
keep its members, and the JSON status follows the typed Report; TSV
control-character and isolated-surrogate escaping loses no value, and `at` and
spans are accurate; extract binary output writes only `-o` and MUST NOT be
polluted by the generic stdout copy rule.

**Tests**: No automated test is added. Q07 is a consistency review checklist for
DESIGN.md, the source and the agent documentation, not a test family; help,
routing, flags, column names, old aliases and default wrapping get no black-box
or golden. F03/Q04/Q06/Q09/Q10 test complex UTF-16 fidelity, identity, spans and
extraction safety, and basic layout is not retested.

**Acceptance**: Format, lint and build checks plus the Q07 manual review, with
no `h07_` test filter. The old tests stay in ref-fork; they are not deleted but
enter no active gate. **Completion**: The eleven-command wiring matches the
documentation, the matching domain task supplies complex verification, and no
MCP is designed.

**Not in scope**: MCP input or output schema, automatic changes to a user's
agent configuration, a global old-brand license or history rewrite, a release.

## H08 — CLIProductRC and the old-code retirement list

**Dependencies**: H07, T25, T14. **Scope**: overall acceptance and release
configuration preparation; MCP stays deferred.

**Files**: the active root Cargo manifest and lock, README/AGENTS, the
application query and engine adapter, `apps/diana/tests/product_contracts.rs`;
verify that no ref-fork dependency exists, and maintain no old MCP build.

**Input→output / Interface**: EngineCoreRC plus H01–H07 evidence → a
CLIProductRC report, a live call-graph and dependency retirement list, and a
single-diana release configuration. Acceptance covers the completeness and
identity of unordered output and does not require cross-run source or report
sequences to match; this card creates no tag and uploads no release.

**Steps**:
1. Check the DESIGN.md commands, columns and routing by source and document review without
   adding a black-box test; Q02–Q06/Q09/Q10 and the F tests verify complex
   defined-versus-referenced behavior, xref site completeness, extract original bytes/CRC/file
   safety, cross-DEX and 041 identity, and engine behavior. Q01/Q07 are manual checklists only,
   Q08 rolls them up, and they MUST NOT be treated as executable test families.
2. Verify that the active CLI and dependency graph contains no call to the old dexdec, MCP,
   helper or ref-fork code; the old code stays complete in the reference and needs no compile or
   test run, and no new adapter is designed for MCP.
3. ref-fork is neither a production dependency nor a gate; it keeps its original source, tests,
   license and history, and the existence of the current reference directory MUST NOT make it a
   second release product or a legacy dependency requiring a test run.
4. Verify that the active workspace contains Diana only, that packages stay `publish=false`, and
   that no archived release script or CI wiring exists; a reference-project build result MUST NOT
   replace product acceptance.
5. Run the Diana-only G0 and the complex H and engine CoreRC gates, reviewing basic H01/H07 only
   and creating no empty test; an authorized representative-input release measurement is explicitly
   not-run, and no old MCP regression or release test runs.
6. List knownUnsupported, raw coverage and the deferred-MCP statement, and do not describe
   CLIProductRC as a migration of every project interface.

**Invariants / limits**: No silent fallback across the old engine; a single
binary with no version or tag change; active code MUST be verified; the old
reference does not participate in this branch's gate; the card MUST NOT claim
all DEX semantics are correct.

**Tests**: Q08 rolls up the complex semantic and domain tests: cross-DEX and 041
identity, input, extraction, cancellation and budgets, plus the T25 behavior and
degradation completeness. It establishes no whole CLI command matrix and no
branding, help or flag black-box; CLI wiring, old dependencies and the release
configuration use source review and a local build check.

**Acceptance**: G0 plus the actual complex Q02–Q06/Q09/Q10 tests and the T25
core gate (nonzero runs); Q01/Q07 are manual checks, Q08 rolls them up, and no
empty `h08_` test is required. The release artifact is checked locally and not
published.
**Completion**: Every required case and invocation in the JSON is accepted
against independent expectations, reusing T25 engine evidence and completing the
application side of engine-and-application and the application cases; nothing is
removed or skipped and no core Java becomes Debug. MCP stays deferred, and the
basic interface gets no additional automated test.

**Not in scope**: MCP migration, directly deleting `src/mcp`, a release or tag,
automatic private-corpus downloads, cache or parallel optimization without
evidence.

## H09 — Defined symbol listing and cross-DEX reference sites

**Progress**: The [xref batch](batches/h09-xref-cli.md) wires `xref` with the four
DESIGN columns, exact string/type/method/field targets and the exact `--class`
restriction; `fields`/`methods` already list only real definitions. A 041 logical
member beyond the first is not scanned yet.

**Dependencies**: H04. **Scope**: fields/methods/xref with the specified
columns, without running the decompiler.

**Files**: root `src/analysis/dex/{mod,members,opcodes}.rs`,
`src/analysis/query.rs`, `src/cli/{args,mod}.rs`, plus `symbols.rs` and
`references.rs` as needed; Q09 in `tests/product_contracts.rs`.

**Input→output / Interface**: every actual field or method definition of a
class_data → a single field-at or method-at column; every actual opcode
reference site → the three columns `entry`, `context` and `match`. methods is
not all method_ids references, and xref is not the old findrefs
one-method-per-row form; DESIGN.md is the only field and selector specification.

**Steps**:
1. Walk class_defs and class_data and read the checked delta index, flags and code offset of
   static and instance fields and of direct and virtual methods separately; generate the canonical
   `at` carrying the class-def, member-id, owner, name and proto, and list abstract, native,
   constructor and clinit declarations too.
2. `fields`/`methods --class` selects the original definition exactly, and `--filter` matches a
   semantic name or descriptor literally and outputs every match; verify `at` against a duplicate
   class_def and do not rely on ordering or positional identity.
3. `xref` resolves the kind string/type/method/field target exactly and scans the original code of
   every root DEX logical member, reading the opcode operand and ref index instead of searching the
   recovered Java text. Record the referencing MethodKey, the code-unit offset and the actual
   matched value or ref.
4. Build the context as method-at plus `@cu:0x...`, and list each real site of a method separately;
   for several operand references at one site, create records with an explicit operand-site identity
   and do not reuse method-level deduplication that hides sites.
5. Keep the 035 and existing 041 scan format boundaries, handle a known-unsupported opcode or profile
   per the original safety contract, and keep a batch only for resource backpressure and completion-
   order output, with no global sort or pagination.
6. Output the TSV header and escaping or the JSON items per DESIGN.md; test the difference between the
   exact UTF-16 string value and the root original scanner display, and never do an exact xref after a
   replacement.

**Invariants / limits**: Defined and referenced, the string pool and a
const-string site stay separate; every row has an explicit original identity;
every member, count, code and ref index is checked and the output and match
scratch are bounded. A successful query returns every match with no duplication
or loss, and empty is normal; an equal numeric index MUST NOT merge records
across DEX files.

**Tests**: Q09 verifies multi-DEX same-name and same-proto, duplicate class_def,
an external method_id that is not a definition, several xref sites in one
method, an unused string-pool entry, an isolated surrogate and an illegal delta
or ref index through the public symbol and reference module interface. It checks
the original identity, site and UTF-16 value and does not test columns, command
arguments, JSON item layout or a simple getter.

**Acceptance**: G0 plus `cargo test -p diana --test product_contracts h09_` plus
the H03/H04 regression; the nonzero Q09 checks the original identity and site
set.
**Completion**: fields/methods/xref are really wired into the CLI and the
definitions and sites are exact, not just a new help command.

**Not in scope**: Fuzzy regex or bare-method-name overload guessing, a complete
DEX object model, Java source xref, an MCP tool, pagination, new unrequested
default columns.

## H10 — extract: original entry bytes and safe atomic files

**Progress**: The [extract batch](batches/h10-extract.md) selects one physical
record exactly, validates its payload, decodes binary XML by default with `--raw` as
the byte-for-byte alternative, and publishes atomically while refusing unsafe
targets.

**Dependencies**: H02. **Scope**: single physical-entry extraction, with no
batch unpacking.

**Files**: `src/analysis/archive.rs`, the root input and request scope,
`src/cli/{args,mod}.rs`, plus `src/analysis/extract.rs` as needed; Q10 product
tests. A new dependency for CRC needs its own reason, license and lock change,
and a hand-written lenient CRC check without review is forbidden.

**Input→output / Interface**: an APK/ZIP plus an exact entry name (optionally
`--entry-index`) plus the required `-o` → a complete decompressed payload file.
Method 0 leaves the bytes unchanged, method 8 decompresses the original bytes,
the card MUST NOT normalize the DEX, switch to 041 or generate Java, and stdout
stays empty. A binary XML entry is decoded to text XML by default (the same
bounded reader as `manifest`); `--raw` writes the original payload bytes. CRC and
size are checked over the original payload before decoding, and a partial decoded
document follows the manifest warning and `--strict` rules.

**Steps**:
1. The directory lookup keeps every physical record with the same name and exits 4 on an
   ambiguous name; the entry index and the name MUST agree, and basename or path normalization
   MUST NOT change the original name or select the wrong record; a bare DEX is not a ZIP entry.
2. Check the local and central structure and the data range first, then reject EncryptedEntry
   and UnsupportedCompression other than 0 or 8, and only then check size and CRC under the
   unencrypted rules. An encrypted stored entry carries an extra header, so the card MUST NOT
   wrongly report equal-length plaintext damage; there is no password or decryption path.
   Review the codec and check CRC with the actual Q10 inputs, and MUST NOT assume the default
   performs complete validation.
3. Write only the user-specified file, and the entry name takes no part in path construction.
   Reject an output directory, a symlink, and the input file itself or a hardlink alias to it;
   no-force never overwrites. Validation and file open or publication use a safe atomic
   primitive to avoid a race where rename overwrites after an exists check.
4. Create a private temporary file in the target directory, borrow or decompress within bounds
   and write, and publish only after complete validation; cancellation, I/O, CRC and budget
   overrun release the temporary file and the reservation. `--force` replaces only a permitted
   ordinary file explicitly, with Windows and Unix semantics accepted separately.
5. The default path decodes a binary XML entry through the checked reader and
   otherwise copies the payload; `--raw` bypasses decoding. No other byte stream passes
   through UTF-8 or JSON conversion, there is no binary stdout or success status
   line, and debug output and errors go only to stderr. Use an independent binary
   file output adapter and do not misuse the generic emit for stdout and file at once.

**Invariants / limits**: The output equals the complete decompressed payload of
the original entry word for word and the raw 041 header and checksum are
preserved; the card MUST NOT create an entry-name directory tree automatically;
it MUST NOT delete an existing user file or the input; a CRC or size failure
produces no false success and no half file, and a resource budget is not
pagination.

**Tests**: Q10 stored and deflated entries, binary content containing NUL and
non-UTF-8 bytes, an original 041 container, a duplicate entry name or index
mismatch, a ZIP name `../x` that still writes only the selected `-o`, bad CRC or
size, unsupported compression or encryption, a bomb or limit, cancel, an
existing output with no-force, force, symlink, hardlink and input alias, and a
concurrent publication race. Every test uses a self-made temporary file and
touches no real user path. The fixed D068–D070 are accepted per each
invocation's entry, force and error kind and the process observation of §4.3.4.

**Acceptance**: G0 plus `cargo test -p diana --test product_contracts h10_` plus
the input and archive regression; Q10 compares the complete original payload bytes, an
unchanged target after failure and no temporary residue through the public
extraction interface; the test starts no CLI to check stdout or `-o` routing.
**Completion**: Evidence exists for the original bytes, CRC, race, alias and
limit safety, and the card is not merely a decompression into a Vec.

**Not in scope**: Batch unzip or automatic writing to an entry path, exporting a
compressed stream or the ZIP local header, DEX normalization, a CLI external
process, MCP extract or a release.
