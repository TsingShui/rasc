# 09 — Emission and presentation

Code: `crates/diana-decomp/src/p9_emit/`; public report entry:
`report.rs::render`. CLI presentation is in `apps/diana/src/cli/`.

P9 renders products and original facts already supplied by earlier phases. It
does not perform a second analysis, choose new types or repair missing semantics.
The report retains original identities and source spans. Text and JSON represent
the same result rather than selecting different recovery paths.

Debug output is an analysis-oriented snapshot of original DEX facts, not Java or
a promise of reassembly. Unknown or undecoded ranges remain explicit. P9 MUST
respect output quotas and cancellation; errors MUST NOT be disguised as a
complete result.

## Result presentation


P8 decides language eligibility, P9 only reads the existing result, and the application MUST NOT
refuse to call the renderer because a class is Mixed:
1. JavaUnit uses the complete Java printer; a single-method Java or DeclarationOnly result prints
   the corresponding declaration or body without an index prefix.
2. Mixed first states the original class identity and “not a complete Java compilation unit”, and
   keeps the shell/fields according to the real DeclarationOutput. Members are presented
   segment by segment according to their original MethodKey: Java keeps the validated declaration
   or body; Java retains its readable reconstruction and uncertainty reasons without claiming verification; Generated references only the proven owner syntax and the original key mapping and
   MUST NOT fabricate an independent method; Debug keeps the available original facts, their coverage and the reason;
   Unavailable keeps the identity, the reason and any available excerpt. An unexplainable shell is
   marked as such, and available methods MUST NOT be deleted because of it.
3. The language/reason marker and the source segment are charged UTF8 bytes separately, and
   OriginSpan is relocated to the actual text of this request; the implementation MUST NOT put raw
   into a Java comment or fake body that could close accidentally, and MUST NOT use stubs to fill
   out the class. Untrusted names and messages use the established display escaping, while string
   and payload contents are still preserved completely per language. No output ordering is
   required.
4. text and json share the same Report, and MUST NOT rerun the analysis or trigger a second
   fallback. Complete rendering happens within the original quota, and the payload is checked
   before writing; allowing Mixed does not allow truncation. A single Unsupported member is not a
   ban on presenting the whole class.
5. Result completeness/language eligibility is separate from error classification: a complete
   Unsupported result MAY be presented normally; Malformed, BudgetExceeded, InternalInvariant and
   similar keep their failure status even when a renderable Report already exists, and the
   achieved output is not cleared; Cancelled MUST NOT synthesize a Report and continue raw, and a
   render or I/O failure MUST NOT be passed off as complete delivery.

## Original DEX debug output

When readable Java is not available, Diana retains the original method identity, typed reason
and available original facts for an Agent to inspect. This is an analysis view,
not executable source, a standard Smali document or a promise of reassembly.
Production uses the checked original bytes and decoded facts; it does not run an
assembler, another engine or another recovery attempt.

### Facts and coverage

The debug snapshot contains only original facts, never an unverified Java AST, guessed
types, invented registers or reconstructed effects. It records:

| Fact | Required information when safely readable |
|---|---|
| Identity and declaration | Physical/logical origin, class and method indices, owner, full name/proto and raw flags; no-code declarations remain code-free |
| Code layout | Original code range, code-unit count, register/parameter-word counts and instruction PCs/widths |
| Instructions | Original mnemonic, ordered register words, exact literal bits and full references; invoke word order is not collapsed into source arguments |
| Control and data | Branch targets at original code units, switch keys/targets and array payload width/count/bits |
| Exceptions | Original protected intervals and handlers in priority order |
| Referenced values | Exact UTF16 units, field/method/type/proto references and used call-site/method-handle data |
| Context | Available hierarchy, fields, encoded values and their ownership; debug names/annotations may be included without requiring Smali directives |
| Missing information | Explicit undecoded ranges and the reason; safe original bytes may accompany them |

A complete decoded view retains every original instruction, including dead nops,
and the associated targets, payloads, handlers and referenced values. Optional
source/debug names and annotation syntax are not output-completeness gates.
A partial view MUST identify what is missing; it MUST NOT claim complete coverage.
Unknown instruction width stops decoding that range. The renderer MUST NOT guess
NOP, advance by a guessed width or invent a standard instruction. Local damage
MUST NOT erase facts already read safely or completed Java from other members.

### One read-only exit

After a normal failure, release unused analysis scratch and produce debug output
from the original checked facts within the remaining RunContext resources and any
explicit fallback quota. The existing fallback_* fields govern this debug work;
there is no separate recovery subsystem. Cancellation stops immediately. A known
Malformed or BUG remains a hard failure even when useful debug information exists.
Insufficient resources leave an explicit Unavailable reason and any safe excerpt;
Diana MUST NOT retry normal analysis, increase limits or silently truncate a view
marked complete. Omitted quotas remain None, not zero or an invented default cap.

### Readable snapshot, not an assembly language

Debug source is a readable JSON snapshot, tagged `diana.dex-debug/1`, with explicit
coverage. It exposes original declaration/code facts rather than SSA, CFG or AST
objects. Default text places that snapshot at the failed member; JSON exposes the
same member source with language `dex_debug`. No extra format switch is needed.
Object key order and whitespace are not stable. Instruction, argument, payload and
handler arrays retain program order. The acceptance adapter independently parses
the actual source slice and compares its facts with the certified original input.

The snapshot carries class/method declaration records, code records and referenced
call-site/method-handle data. A code record contains original register/count facts,
ordered instruction records and protected intervals. No-code declarations use a
null code record, not an invented body. Partial decoding identifies undecoded
ranges separately from decoded records. An unavailable method may instead show
only checked offsets and original byte/word values, clearly marked as an excerpt.
A method-word excerpt exposes its original member identity, `code_unit_start` and
`words` (u16 values) as readable JSON. A physical-byte excerpt instead names its
file-byte coordinate and byte values; neither assigns an unknown opcode width.

Strings and arbitrary UTF16 values use exact u16 vectors where UTF8 cannot carry
them; readable spellings never replace their units. Numeric literals and payloads
retain their original width/bits, without FP conversion. JSON escaping handles
quotes, controls, backslash-u and text containing */. Debug output is never put
inside a Java comment or a fake executable method. No Smali labels, directives,
class shell, assembler grammar or byte-for-byte rebuilt DEX is required.