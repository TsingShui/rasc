# 01 — Input and P1 original facts

Code: `crates/diana-decomp/src/input/` and `src/p1/`. Application input/ZIP
handling lives in `apps/diana/src/analysis/`. Ownership and stage boundaries are
specified by [PIPELINE_DESIGN §§3–5.1](../PIPELINE_DESIGN.md).

## Shared checked reader

The application owns physical input bytes; `DexContainer` and `ProgramIndex`
borrow them. The engine does not open files, ZIPs, processes or networks.
`DexContainer` recognizes little-endian DEX 035, 037, 038, 039, 040 and 041.
Unrecognized versions and reverse-endian input retain unsupported classification.
Recognizing a header does not establish all member records or ART validity.

DEX 035–040 use a logical header at physical byte zero. DEX 041 can contain
several logical headers; original offsets address the physical container.
Header selection MUST NOT rewrite bytes, add the logical header offset to an
already absolute offset or merge equal numeric indexes from different logical
DEX namespaces.

`Cursor` is the shared bounded read implementation. Readers MUST check offset,
count, stride and end arithmetic before slicing or reserving memory. Fixed table
strides and minimum encoded-record sizes are format facts, not numeric quotas.
LEB values MUST retain their format width/sign constraints; malformed encodings
MUST NOT become zeros. MUTF-8 retains exact UTF-16 units, including isolated
surrogates; declaration syntax checks are distinct from readable string display.

Original DEX declarations and references are the only symbol facts. Missing target
class definitions do not erase a readable reference's owner/name/prototype.
Class location and P0 caches remain local to the selected logical index;
allocation failures MUST NOT be cached as member absence.

## Method source preparation

`ProgramIndex::method_source` consumes a method entry from P0 `ClassFacts`.
It uses the original code offset, delivered declaration and flags without walking
class-data again or resolving the method declaration a second time. It establishes
safe code/header/instruction/try/handler ranges and original register counts.

`MethodSource` separates a code-bearing member from a declaration-only member.
An omitted code offset and native/abstract flags are checked for consistency;
no-code declarations MUST NOT create empty executable graphs. The reader checks
incoming word totals against the declaration, try record ranges/order and handler
encoding. It does not know instruction boundaries before P1.

Method/parameter annotation reads are independent of code analysis. Original
annotation facts and local failures also remain available through P0. The code
view retains the original debug-info offset; the former debug-program interpreter
is not an active engine module. P1 does not require source names, generic metadata,
line numbers or debug local names to analyze code. Current range/metadata behavior
must be inspected in the reader rather than inferred from removed debug schemas.

## P1 disassembly facts

`decode_method_source` uses the method reader product and the selected index's
restricted original reference readers. One decoded instruction/payload directory
is shared by structural checks and subsequent products; P1 MUST NOT recover
boundaries by guessing widths or seeking a later return.

For established positions and facts, `DecodedMethod` retains:

| Facts | Consumer need |
|---|---|
| Identity, declaration and flags | Preserve the selected method and original signature. |
| Entry layout | Exact `this` and parameter slots, type identities and widths. |
| Instruction/payload directory | Original PC, opcode/format/width, register words, literal bits and borrowed payload data. |
| References | Instruction reference slots associated with checked string/type/field/method/prototype/call-site/handle facts. |
| Invoke layout | Receiver presence, ordered parameter widths and result category; wide argument words remain adjacent. |
| Structural associations | Branch/switch targets, payload ownership, handler entries and producer/result adjacency. |
| Protection | Original try boundaries and ordered catch clauses, with typed catches and catch-all distinguished. |
| Independent metadata state | Available method/parameter annotation ranges and recorded read issue. |

P1 checks all decoded instruction register spans and explicit targets, including
unreachable instructions. Structural reachability checks reachable fallthrough;
it does not prove conditional paths feasible. `move-result*` MUST follow its
checked producer and MUST NOT be an explicit branch/handler entry. Handler and
move-exception association are checked independently of normal branches.

P1 checks opcode requirements against original reference facts. It MUST NOT use
Java source legality, normal compiler templates, natural loops or complete external
hierarchies as input gates. Known format damage remains `Malformed`; unsupported
encoding/capability remains `Unsupported`. Instruction/reference association is
not a whole-method legality badge.

## Local availability and publication

P1 MUST follow [the P1–P9 contract](../PIPELINE_DESIGN.md#92-p1p9-统一消费局部结果).
The coverage obligations here define that full target; the current publication
floor and retained representations are specified under Outcomes below.
Its published product MUST describe code coverage as decoded instructions,
payloads, reliably bounded unrecovered ranges and ranges without established
instruction boundaries. A range is not an executable instruction.

P1 MUST retain each established operand, reference identity and structural
association independently. A reference read failure remains associated with its
original slot and reason; it MUST NOT erase known width or other independent
facts. Invoke word order, parameter grouping, result category and adjacency MUST
have explicit states when they cannot all be established; P2 MUST NOT guess them.

Only the reader/decoder that established an extent can authorize continuation at
its end. P1 MUST NOT guess an unknown width, scan for a later return or manufacture
a successor. It MUST preserve later independently established positions when safe.
P2 consumes the coverage and fact states through the same published product,
without retrying P1 or accepting its mutable construction snapshot.

## Outcomes and evidence

Published `DecodedMethod` retains reference-read results separately from opcode-use
issues. `reference_fact` distinguishes no reference site from a failed read;
`reference_use_issue` preserves a bad association without erasing a readable
owner/name/type/prototype. Operand and result issues retain their original PC.

Invoke binding consumes the same checked prototype facts, not another declaration
read. Parameter grouping and result category are independent: failed grouping
retains a readable return category, and a polymorphic prototype remains usable
when the separate target declaration is unreadable. `invoke_layout_at` exposes
failed grouping; `invoke_result_at` exposes the independent return state.

P1 currently publishes only with a complete instruction directory, entry layout
and established control/protection associations. Unknown width or unreliable
control still produces `DecodedOutcome` with retained items and a classified error;
this includes resource/invariant construction stops rather than dropping the prefix.
That snapshot MUST NOT be promoted or used to invent a graph. Local reference,
register-span, argument-grouping, result-use and return-kind issues do not themselves
prevent publishing a method when these minimum guarantees hold. Broader control
uncertainty handling remains governed by the target contract and the engine README.

`RawMethodView::visit_layouts` and `visit_instructions` are read-only original
inspection paths over safe ranges. They are not substitute analysis routes.
Engine development inspection uses the single `dbg_shell` example; it observes
retained products rather than starting a phase-specific retry.

P1 byte correspondence is inspected by `roundtrip`/`roundtrip-all`: the shell
independently re-encodes retained instruction/payload facts and compares original
bytes. This evidence does not validate P2 behavior or final Java semantics. P2's
own translation obligations are in [02-flow](02-flow.md).

## Application input safety

The application owns input snapshots, physical archive identity and decompression.
ZIP ranges, sizes, supported compression and payload CRC remain checked; extraction
publishes one selected original entry atomically and MUST NOT rewrite DEX headers
or expose a partially written successful file. AXML decoding preserves usable
partial content with explicit diagnostics rather than silently removing records.
See the [CLI README](../../../apps/diana/README.md) for current selectors,
publication options and output behavior.

All native reads, growth, decompression and publication MUST preserve classified
failures and backpressure. There are no numeric resource quotas.
Changes require concrete original-input and semantic evidence under
[CONTRIBUTING](../CONTRIBUTING.md); removed test/fixture contracts are not active
acceptance jobs.
