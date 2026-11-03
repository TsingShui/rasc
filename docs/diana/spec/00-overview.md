# 00 — Product and engine overview

Code: `apps/diana/src/analysis/`, `crates/diana-decomp/src/engine.rs`,
`crates/diana-decomp/src/pipeline/` and `crates/diana-decomp/src/substrate/`.

Diana accepts APK, ZIP and DEX input and exposes class, method and reference
queries. The application owns files, archive handling and presentation; the
engine borrows DEX bytes and caller-supplied declaration facts. The stage map
and the current ordinary-method entry are in [phases.md](../phases.md).

## Readable result

The product direction is to display available readable reconstruction by default,
including a method whose meaning cannot be fully established. The output MUST
identify material uncertainty and retain original member identity and available
DEX facts. A local failure MUST NOT discard other members. A readable result is
not a claim that source compiles or is semantically equivalent.

The current engine does not yet implement that entire behavior. In particular,
`engine.rs::normal_member` sends a declined method to original-fact debug output
rather than retaining an unverified Java reconstruction. Text and JSON present
the same underlying report; neither is a capability switch.

## Boundary and failure behavior

Input bytes are untrusted. The application and engine MUST preserve checked
ranges, explicit resource limits and cancellation. A malformed or unavailable
input MUST NOT be reported as successful reconstruction. Unknown instruction
widths MUST NOT be guessed; available original facts and coverage remain explicit.
P9 renders existing products and MUST NOT perform another analysis or repair
missing semantics.

## Migration

The chaptered specification is being built incrementally. Until a section is
migrated and reconciled, [DESIGN](../DESIGN.md) and
[ALGORITHMS](../ALGORITHMS.md) contain the detailed existing contracts. Their
older verification-only presentation gates conflict with the readable-result
direction above; resolving those clauses is still required before implementing
the new output behavior. Existing semantic acceptance evidence does not establish
coverage of arbitrary applications.
