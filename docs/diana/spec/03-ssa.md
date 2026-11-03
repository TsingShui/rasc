# 03 — SSA

Code: `crates/diana-decomp/src/p3_ssa/`; entry: `normal.rs::build`.

P3 consumes the P2 flow product and associates register uses with static
definitions. Block-entry Phi values represent incoming register-word state;
normal and exceptional edges carry their respective environments. A throwing
operation's exceptional edge uses the state before its result is committed.

A definition identity is local to the current method analysis. P3 MUST preserve
word widths and distinguish an unavailable definition from an uncertain source
type. Missing definitions and inconsistent wide registers retain classified
failures rather than becoming default values.

The fixed-entry Phi and verification details remain in
[ALGORITHMS §3](../ALGORITHMS.md#3-fixed-entry-phi-construction-bind-once-do-not-iterate-on-valueid-guesses-t07t16).
