# T19 class-field completeness correction

Goal: prevent JavaUnit admission when original fields have no source declaration,
while preserving available original methods in Mixed. Contracts: DESIGN §2.2/§4;
ALGORITHMS §12.1; TASKS §1/T19. Preparation reuses accepted draft-7 B01–B04 on
macOS arm64. The operator authorized this dependency correction before T20.

P1's Fields visitor owns checked field declarations and original static/instance
membership. P8 class_unit::verify consumes that product separately from method
headers before sealing a complete unit. P9 renders the existing result. The
shared ordinary method chain, public interfaces and independent expectations stay
unchanged. Existing visitor work, metadata and cancellation accounting apply.

The D042 baseline at b321329 incorrectly labels source as
JavaUnit despite omitting field n; independent javac rejects that source. The
AllHeaders selection never delivers MemberEntry::Field, so its field-rejection
arm cannot run. The regression extends the existing public class-qualification
test with self-made static and instance field declarations that have legal DEX
names but Java-keyword names. Their method bodies remain independently available;
removing those fields cannot manufacture a complete JavaUnit.

## Execution

The extended public regression fails before the correction: an omitted static
field is incorrectly admitted as JavaUnit. P8 now performs a separate Fields
visit, propagates declaration failures and rejects a field-bearing unit before
method-header qualification. Both static and instance counterexamples retain
their original constructor as Java in Mixed. The inherited-binding negative
and D040 positive remain covered by the existing core tests.

Reproduce the core regression with
`cargo nextest run -p diana-decomp --all-features --locked t19_complete_method_bodies`.
Run `python3 tools/e2e.py run` for the full fixed suite.
The new self-made FieldShell.smali and StaticFieldShell.smali inputs use Java
keyword field names, implicit default values and an ordinary Object constructor;
the locked Smali 2.5.2 assembler produces their checked-in DEX fixtures. Their
catalog reuses the existing Object/JDK declaration evidence.

On macOS arm64, fmt, strict workspace Clippy, locked release build and all
147 cargo-nextest tests pass with zero skips. Diff and desensitization gates pass.
The full E2E run retains 65/86 cases and
138/181 seams passing; outstanding cards keep the command exit nonzero.
Comparison with the b321329 baseline covers 2,379 original member observations, with
identical member identity, status, source and diagnostics and no passing-seam
regression. Only D042's class result changes from false JavaUnit to Mixed; both
constructors remain Java. Required full-unit D042 still fails until its original
field can be represented under T20.
