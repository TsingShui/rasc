# Diana phases

Diana separates APK/file handling from the native decompiler. The application in
`apps/diana/src/analysis/` owns snapshots, archives, input selection and queries.
The engine in `crates/diana-decomp/src/` borrows DEX bytes and declaration facts.

| Phase | Engine directory | Current responsibility |
|---|---|---|
| P0 | `p0_knowledge/` | Declaration facts and request policy |
| P1 | `p1_input/` | Checked DEX tables, method input and original facts |
| P2 | `p2_flow/` | Instruction operations, effects and control-flow graph |
| P3 | `p3_ssa/` | Definitions, uses, Phi and exceptional environments |
| P4 | `p4_calls/` | Call, field and declaration evidence and obligations |
| P5 | `p5_types/` | Types, use conversions and initialization facts |
| P6 | `p6_variables/` | Storage, edge copies and exception snapshots |
| P7 | `p7_regions/` | Region, protection and monitor plans |
| P8 | `p8_java/` | Java declarations, method lowering and class assembly |
| P9 | `p9_emit/` | Render existing Java and original debug facts |

The ordinary-method scheduler is `engine.rs::normal_member`; it invokes the
P2–P6 chain through `pipeline/branch.rs`, then P7 and P8. P9 renders the resulting
product. Declaration-only and generated members have their own phase-owned
checks. A declined method currently goes to the classified raw exit.

The product direction is readable output with explicit uncertainty. The current
engine still declines many methods when declaration or representation evidence
is missing; the revised behavior is not implemented yet.

The [stage chapters](README.md) describe each phase and link to detailed existing
contracts in [DESIGN](DESIGN.md) and [ALGORITHMS](ALGORITHMS.md). Migration and
reconciliation with readable output remain in progress; do not infer
implementation coverage from a phase name.
