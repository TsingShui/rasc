# Static preparation and final delegation

Goal: close T20 static preparation, final delegation and D079 related-unit
obligations through existing producers. Baseline: ca830b4 plus the verified
boolean/final-int slices. Contracts: DESIGN §4/§4.1; ALGORITHMS §12.1–§12.2;
TASKS T20. Preparation reuses accepted macOS arm64 B01–B04 and draft-7.

The operator approved P1 checked scalar/String field preparation, P4 own-clinit
static-final obligations, P8 per-method write summaries and original delegation
targets combined by P8 unit verification. P9 consumes sealed assignments only.
The engine keeps its existing per-member scheduling; no callee reanalysis or
new stage authority is added. D079's runner calls the existing public Engine
separately for each certified owner and compiles both recovered units together.

P1 metadata retains partial guarantees on damage; P4 declarations retain access,
constantness and original owner facts; P5/P6 supply checked operation uses and
storage. P8 rejects ambiguous/missing delegation, repeated writes, incompatible
preparation and unproved source DA. Traversals and source allocation retain the
existing request/method ledger and cancellation. Missing local evidence preserves
other members with classified Debug/Mixed results.

Independent evidence: tests/e2e/sources/D079/{fixtures,driver}, D045 and D046
certified projections, and existing initialization/public resource regressions.
Acceptance requires independent JVM initialization traces, original non-inlined
getstatic, final modifiers and original member/field coverage, not a source-only
or scaffolding pass.

## Execution

P1 FieldInput retains decoded scalar/String values with original encoded facts.
P8 preparation validates descriptor/value compatibility, bounds and exact FP bits;
noncanonical NaNs retain Unsupported. P8 class_unit emits all preparation blocks
in original field order after declarations. Static finals with original writes
require default preparation and checked exactly-once normal completion; absent
writes get one explicit encoded/default assignment in a static block, preserving
absence of ConstantValue. A non-default preparation plus a later write stays Mixed.

P4 admits only evidenced own-clinit static-final writes as initialization
obligations. P8 field_ops verifies the original operation and publishes an
unqualified own-static receiver; names colliding with synthetic local spellings
reject source admission. P9 prints that selected form. P7 adds a one-block
Sequence for constructor/final initialization using the same storage and P8
source-control replay. Existing dispatcher selection remains unchanged for ordinary
straight-line scalar methods.

P8 certificates retain a checked this-delegation target only for a constructor
with no own final write. Class-unit verification follows already-recovered
constructor certificates, checks original target identity, and rejects cycles,
missing/failed targets and own-write-plus-delegation. The existing entry-link
proof remains partial and is not used as a whole-body final certificate. No callee
is analyzed again. Final path support remains an unprotected Sequence/Diamond;
complex/cyclic/unrepresentable DA shapes retain Unsupported/Mixed.

The D079 adapter independently invokes the existing public Engine for each
certified class binding, records separate original-owner reports and submits only
both recovered JavaUnits plus certified Dep/Driver sources to joint compilation.
Each invocation runs in a fresh JVM. Original helper/target classfiles are absent
from the recovered target classpath. Limits and representation choices are passed
to each public call. This changes core-case execution, not comparator expectations.

Core fixtures in crates/diana-decomp/tests/fixtures/final-assignment/ include
copies of certified D045/D079 inputs and bounded declaration subsets from their
catalogs, plus self-made Delegated/Repeated/Preparation/Conflict Smali inputs.
Assemble them with pinned smali 2.5.2 using
java -cp <locked-smali-classpath> org.jf.smali.Main assemble
<fixture.smali> -o <fixture.dex>. The same-directory Case.java, Prepared.java
and Driver.java are independent references under the engine's existing license.
Compile reference and recovered targets separately with locked Temurin 21.0.8+9
javac --release 8 -d <classes> <Case.java> <Prepared.java> <Driver.java>, then
run java -cp <classes> fixtures.Driver in separate processes. Recover each
Smali target through the existing release inspect_class example with its tracked
catalog; compile the actual complete rendered unit, not an original target.

Independent reference/recovered observations agree for delegated values 7/11,
static final int/String/default boolean, char 65535, double 1.5, float negative-zero
raw bits, int -7, long minimum and String length/newline/quote units. The public
initialization tables retain malformed encoded suffixes, original field identities
and explicit/implicit values; add final delegation repetition, non-default-final
preparation/write conflict and missing/duplicate static write regressions. Missing
static writes acquire only the contract-defined explicit default; constructor
missing assignments remain Mixed without fabricated writes. Public resource tests
and the shared new-consumer test cover work/allocation/cancellation at proof and
preparation boundaries. No new auxiliary tooling tests were added.

Observed on macOS arm64 with unchanged draft-7 and pinned tools:

- cargo fmt --all -- --check: passed.
- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings:
  passed.
- cargo build --workspace --release --locked: passed.
- cargo nextest run --workspace --all-features --locked: 163 passed, zero skips.
- python3 tools/e2e.py run --case D041 --case D042 --case D043 --case D044 --case D045 --case D046 --case D047 --case D079 --case D080:
  9/9 cases and 12/12 seams pass. This includes both independently recovered D079
  units, joint javac, original member/final-field checks, actual getstatic sites
  and class-init/READ trace agreement.
- python3 tools/e2e.py run: 72/86 cases and 147/181 seams pass; all 177 invocations
  execute. Exit 1 reflects remaining T21–T23/application/core obligations. Only
  D079's two verdicts improve against the preceding final-int integration.
  All 2,379 previously observed original members retain identity, status, language
  and classified reasons. Source-only changes in constructor units and D015
  use the verified single-block Sequence; their existing compilation/Drivers pass.
  D046 retains original encoded tags and semantics; its constructor likewise
  changes to the verified Sequence.
- python3 tools/e2e.py run --representation dispatch_only --case D001 --case D002 --case D003 --case D004 --case D005 --case D006 --case D007 --case D008 --case D009 --case D010 --case D011 --case D012 --case D013 --case D041 --case D042 --case D044 --case D046 --case D047 --case D071 --case D082:
  20/20 selected cases and 48/48 seams pass. Required final Java remains in the
  accepted natural source profile; dispatcher does not acquire an alternate
  final-field recovery implementation.
- git diff --check and python3 tools/audit/desensitize_check.py --range HEAD:
  passed.

T20's supported initialization source profile and task-scoped fixed/supplemental
obligations pass. Unproved complex final/source profiles remain classified local
results as specified by ALGORITHMS §12.2. ART/other hosts are not run, and
EngineCoreRC remains unaccepted.
