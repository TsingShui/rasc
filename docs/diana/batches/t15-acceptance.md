# T15 native contract review

Goal: check every T15 obligation against the current shared producers and
independent evidence before starting T16. Contracts: TASKS T15 and §4;
DESIGN §4; ALGORITHMS §2.4/§4/§9.4. Specification: unchanged draft-7.
The preparation profile remains the accepted macOS arm64 profile.

## Obligation review

| Obligation | Current producer and independent evidence |
|---|---|
| Descriptor/opcode and word widths | P1 field target checks; P2 declaration/read/result products; P3 word ownership; the public malformed heap table checks wrong kind, partial stores and overwritten loads. |
| Field binding, flags and initialization | P4 FieldAccess retains original owner/name/type/reference and rejects missing/contradicted facts, final writes and constant-inlined reads. Public tables check missing flags, volatile contradictions, qualifier/target evidence and final constantness. |
| Original order and unused effects | Shared ordered operations and P6 storage feed one P8 statement form in both control representations. Original Drivers compare returned values, identity, mutations, helper failures and null/bounds/store exceptions; bytecode retains unused field reads and casts. |
| Narrow, boolean, FP and references | P5 uses original producer ranges, Known01, numeric families and exact/null/Object-widening assignment views. Independent primitive Drivers cover raw FP bits and signed/unsigned boundaries. Unsupported narrow reference assignments gain no invented cast. |
| Arrays and fill | P5 closes descriptor facts before load typing. Existing phase-owned FreshArray and LengthOnly proofs retain D027/D073/D074 behavior and raw rejection; zero count still performs the throwing length read. Object[] and nominal/multidimensional allocation, filled-array/result binding and bounded component projections use the shared products. The derived-array batch checks missing component type IDs, copy cycles, reference store checks and the corrected unsupported float-filled form. |
| Original explicit casts and instanceof | P2/P5 retain original targets and definition/use facts. P8 checks source target scope and every original opcode/register/reference; the type-check Drivers observe unused checks and failure prefixes. |
| Local failures, raw identity and resources | Public Engine/native tests and the full fixed suite retain classified local failures, raw completeness, original member identity, method spans, checked ranges, request/method quotas and cancellation. Every new traversal/owned string uses the existing ledger. |

The implemented source profile admits supported primitive/reference types and
known binding facts. Unknown hierarchy/assignment facts, unsupported source
names or types, unproved fill shapes and final/initialization obligations outside
this profile remain explicit Unsupported results. This review does not count
such results as required Java passes. The initialization of complete source
units belongs to T20; D044's improved get member does not pass that whole case.

## Fixed and supplemental execution

Full `run-2s0a_s5e` runs the unchanged required denominator: 86 cases,
177 invocations and 181 seams. T15's D024–D028, D073/D074 and external-constant
D080 rows all pass: twenty executed invocations/seams. The complete run exits 1
with 51 cases/117 seams passing for genuine remaining task obligations. The
passing seam set and Java member availability equal `run-q7bywtu6`. The derived-array
review checks 2379 original member observations: D050 `$values` remains
Debug/Unsupported while its reason refines to BranchJavaUnsupported; all other
states and reasons are unchanged.

The current release is independently rechecked with the existing static-field
Driver and original helper classfiles: all ten initialization/erroneous-class/
unused-final observations match in each representation policy. The existing
shared-fields Driver likewise matches all 88 observations in each policy after
current Case/Reader/SpecialReader methods compile in the original context.
The helpers and expected observations are reused unchanged. These checks retain
call/field/array order, null and helper-failure prefixes, loops and receiver
binding beyond isolated access methods.

The preceding [type-check batch](t15-nominal-type-checks.md) also runs the seven
self-made primitive/reference/type-check Drivers in both policies. Its compiler
logs, original projections, bytecode and reference/actual observations remain
under `.cache/e2e/t15-nominal-type-checks/`. The final static/shared recheck is
under `.cache/e2e/t15-acceptance/`; it introduces no new comparator or tooling test.

G0 passes on macOS arm64: fmt, strict workspace/all-target/all-feature Clippy,
locked release build and all 125 cargo-nextest tests with zero skips. Diff and
sensitive-input checks pass. Exact commands and fixture/tool versions are in the
owning implementation batch records. JVM evidence uses locked Temurin 21.0.8+9
and Java8 source targeting. ART and other host platforms remain not-run.

## Decision

T15's task-scoped source profile and required fixed cases are verified.
[Object[] allocation](t15-reference-array-allocation.md),
[filled-array producers](t15-filled-arrays.md) and
[derived array types](t15-derived-array-types.md) close the reviewed producer gaps.
The derived-array batch adds independent JVM checks for bounded component types,
copy cycles and original type-check targets, and records the operator-approved
correction of the supplemental float oracle. Unsupported results are not counted
as required Java passes. T16 can consume the checked heap operations and their
effects; protected Java output remains the T18 obligation.
