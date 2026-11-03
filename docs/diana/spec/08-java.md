# 08 — Java reconstruction

Code: `crates/diana-decomp/src/p8_java/`; ordinary-method entry:
`branch.rs::verify`. Class assembly is in `class_unit.rs`.

P8 consumes the planned method, storage, types, original input and available
declaration facts. It constructs method and class representations and handles
declarations, calls, arrays, constructors, exceptions and generated members.

The product direction is to show readable reconstruction when possible, with
material uncertainties explicit. A local failure MUST NOT hide other members.
Neither readable Java nor a class-shaped output alone claims compilability or
semantic equivalence.

The current implementation still requires `VerifiedJavaMethod` for ordinary
method output. Failed checks decline to Debug/Unavailable rather than returning
a readable draft. P8 diagnostic checks can distinguish call source binding,
storage type naming, casts, array operations and other rejection points; their
category alone need not identify the root cause.

Existing representation and declaration rules remain in
[ALGORITHMS §8](../ALGORITHMS.md#8-deterministic-rules-for-representation-selection)
and [§12](../ALGORITHMS.md#12-class-initialization-and-compiler-generated-members-t20t23).
Those rules include older verification-only gates and have not yet been fully
reconciled with readable output under uncertainty.
