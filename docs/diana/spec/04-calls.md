# 04 — Calls and declaration facts

Code: `crates/diana-decomp/src/p4_calls/`; entry: `normal.rs::collect`.
Declaration evidence is supplied by P0 `DeclarationCatalog`; original method
and field references come from P1.

P4 associates SSA call and field sites with original identities, invoke kinds,
signatures and available declaration facts. It also collects initialization,
throws, ownership and binding obligations for later stages. A DEX reference
contains an owner, name and prototype; it does not alone establish complete
overloads, access flags, hierarchy or declared exceptions.

The current default requires declaration evidence for many bindings. Missing
evidence can produce `BindingEvidenceMissing`; this is not a claim that the
referenced method is absent. A reference-trusting policy also exists in the
current code, but it does not yet establish a general readable-output path.

P4 MUST retain unknown facts as unknown and preserve original call identity and
argument order. The existing detailed binding and constructor rules are in
[ALGORITHMS §9](../ALGORITHMS.md#9-call-binding-objects-and-constructors-t11t12);
their older refusal gates still require reconciliation with the product direction.
