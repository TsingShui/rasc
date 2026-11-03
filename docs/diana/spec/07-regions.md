# 07 — Regions and control structure

Code: `crates/diana-decomp/src/p7_regions/`; entry: `plan.rs::build`.

P7 reads the P6 storage product and chooses a plan for structured control flow,
a dispatcher, or protected regions. It also handles switch, exception and monitor
planning. Natural structure and dispatcher consume the same checked operations,
storage and copy plans.

P7 MUST preserve original reachable blocks, transfers, effect order and exception
and lock facts; selecting a prettier structure does not establish semantic
equivalence. If the current implementation cannot plan a shape, a downstream
decline is not proof that the original DEX is invalid.

Existing recognition, protected-region and monitor rules remain in
[ALGORITHMS §6](../ALGORITHMS.md#6-ordinary-structures-and-the-dispatcher-t09t13t14)
and [§10–11](../ALGORITHMS.md#10-trycatch-and-protection-domains-t16t18).
Older proof-based output gates in those sections still need reconciliation.
