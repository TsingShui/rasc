# 05 — Types and use conversions

Code: `crates/diana-decomp/src/p5_types/`; entry: `normal.rs::solve`.

P5 consumes the P3/P4 products and propagates value types, primitive views,
reference shapes and call-use conversions. Its initialization products track
allocation and constructor use. P5 does not alter the original control-flow graph.

The original bit pattern and register width MUST remain recoverable. Integer,
floating-point and reference interpretations are not interchangeable facts.
Unknown hierarchy or use conversion MUST NOT be reported as a proven conversion.

The current solver can decline when a type or use conversion is not supported;
a declined result currently prevents later Java lowering. Readable output under
uncertainty is a product requirement but is not yet generally implemented.

Existing type and conversion rules remain in
[ALGORITHMS §4](../ALGORITHMS.md#4-types-and-java-expressions-t10t11t15).
