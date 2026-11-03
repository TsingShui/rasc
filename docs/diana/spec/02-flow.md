# 02 — Flow and effects

Code: `crates/diana-decomp/src/p2_flow/`; entry: `normal.rs::decode`.

P2 consumes a checked P1 method input. It decodes reachable operations, binds
instruction results, identifies block boundaries and produces normal and
exceptional control-flow facts. Calls, fields, arrays, casts, allocation and
fill participate in the shared operation/effect representation.

Instruction and argument order, throwing sites and original code-unit identity
MUST be retained. A reader MUST NOT guess an unknown instruction width. Malformed
layout and resource or cancellation failures retain their classifications.

Current P2 declines some legal shapes as `JavaAnalysisUnavailable`; a decline
does not prove the original method is invalid. Its `JavaCheckFailed` diagnostic
can identify the failed check and, for instruction-local checks, its code unit.

Detailed instruction and effect rules remain in [ALGORITHMS §1–2](../ALGORITHMS.md#1-reader-format-tables-and-raw-instructions-t02t04).
