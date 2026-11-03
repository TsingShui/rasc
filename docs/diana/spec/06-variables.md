# 06 — Storage and copies

Code: `crates/diana-decomp/src/p6_variables/`; entry: `allocate` in
`normal.rs`.

P6 consumes typed values and plans source storage, normal-edge parallel copies
and exception snapshots. Copy plans preserve the original transfer semantics
when sources and destinations overlap. Exception snapshots represent the
register state at the relevant throwing site, not a handler-global latest value.

Storage and copy construction MUST retain original definition identity, effect
order and classified failure/resource behavior. A synthetic source initializer
MUST NOT be presented as an original DEX definition.

Existing copy, shadow and storage rules remain in
[ALGORITHMS §5](../ALGORITHMS.md#5-locals-phi-copies-and-exception-shadows-t08t17).
