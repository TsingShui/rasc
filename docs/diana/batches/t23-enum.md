# T23 canonical enum ownership

Baseline: 42b060b. Contracts: DESIGN §4/§4.1, ALGORITHMS §12.1/§12.3 and
TASKS T23. T20 initialization and T22 owner/result products are ready. Preparation
reuses accepted macOS arm64 B01–B04 and unchanged draft-7 D050 inputs. The operator
approved the enum declaration entries, sealed bridge/enum Generated distinction,
generated fields and enum-specific unit binding before implementation.

## Producers, consumers and guarantees

P4 `enums::collect` checks the original public/final enum class, its self-bound
Enum Signature, complete own declarations and required Enum constructor/valueOf
catalog contracts. It binds every constant field, the private synthetic array and
the five compiler-owned method roles. Field preparation must be complete/default;
unknown annotations, extra members and inconsistent facts do not acquire a source
proof. P1 signature reading separates method-level MethodParameters metadata from
Signature fragments without accepting unknown annotations.

P8 `enums::verify` independently checks validated original methods: hidden
constructor entry words, exact Enum super call, clinit names/ordinals and stores,
array helper construction/order, values clone/cast and valueOf class/string/cast.
All instructions and returns are consumed; extra effects, protection or reference
mismatches reject the owner. Original references in other executable methods are
scanned without SSA/type/alias recovery to reject backing-storage escape. Ordinary
Explicit members keep the existing shared P2–P8 chain.

MethodOutput::Generated now distinguishes bridge method ownership from enum class
ownership. GeneratedField distinguishes outer capture from enum constant/storage
roles. P8 unit verification checks complete original member/field identities,
flags, descriptors, unique coverage and ordinal ownership before publishing a
sealed enum binding. P9 prints that product, omits its generated declarations and
maps original generated methods to the actual enum source range. It does not sort
fields into declaration order or infer generation from flags.

A source mismatch retains Mixed and original members. A local parse error remains
a stored class issue while normal member retrieval continues; it is not converted
into an empty emission plan. Malformed frames remain Malformed. Hard resource,
cancellation and invariant failures propagate immediately. Proof traversal,
allocation, original snapshots and output retain request/method control, with no
default numeric cap. A standalone values request retains Debug rather than
fabricating an independently declared Java method.

## Independent evidence

The original `tests/e2e/sources/D050/fixtures/Case.java` declares `Z,A`.
Its tracked Driver checks names/ordinals, mutated/fresh values results and valueOf
success/unknown/null against `docs/diana/e2e_cases.json`. Original dexlib2 and
published-format projections establish field order `$VALUES,A,Z`, but clinit
creates Z/ordinal 0 before A/ordinal 1. No expected value comes from Diana output.

Native coverage is `crates/diana-decomp/tests/pipeline/enums.rs`. Its
`tests/fixtures/enums/d050.dex` is the unchanged certified D050 input. The catalog
retains original Object/Enum/Case declarations and Case field evidence rather
than copying the entire JDK catalog.

Fixed original method IDs are helper 0, clinit 1, constructor 2, valueOf 3 and
values 4. Independent instruction-byte starts are respectively 516, 596, 668,
480 and 560. Native mutations change an ordinal/name, helper array order, valueOf
class or constructor receiver; a values body reduced to sget/return remains an
unproved shared-array implementation. A changed ins_size remains Malformed.
Each case retains all original member IDs and renderable original code.

`Effect.java`/`effect.dex` add System.nanoTime to the constructor. The owning enum
stays Mixed, and the original call survives in the constructor snapshot and text.
`Probe.java`/`probe.dex` add a separate caller of Case.values. The independently
projected Probe instruction-byte start is 760; changing that body to sget of the
private storage plus return rejects the enum owner and retains the external
reference. The unmodified Probe input still admits the enum.

Supplemental sources use Temurin 21.0.8+9, `javac --release 8 -proc:none -g:none`
and D8 8.3.37 default DEX preparation. D8 reports the missing library-class warning
for Enum; independent projection confirms the required original code, and these
supplemental inputs exercise raw rejection rather than a source-JVM pass. Original
classfile projection supplies Effect declarations. Fixtures use the engine's
existing license. Work/allocation and cancellation regressions cross the public
Engine through the generated-owner path.

## Execution and next batch

- `cargo nextest run -p diana-decomp --all-features --locked t23_`: five nonempty
  public core tests pass.
- Workspace nextest: 180 passed, zero skips.
- Formatting, strict all-target/all-feature Clippy and locked release build pass.
- `python3 tools/e2e.py run --case D050`: the fixed javac/JVM Driver passes.
- Full draft-7 E2E: 78/86 cases and 155/181 seams pass. D050 is the only changed
  seam against T22. Its five Debug methods become Generated with original
  snapshots and owner spans; no prior Java availability or original identity is
  lost. Other classified member failures are unchanged.
- Diff and desensitization checks pass for the tracked diff.

The enum slice passes. T23 remains open for D051 and the annotation
retention/target/default/element and literal obligations. The next batch consumes
original annotation metadata and declaration evidence without weakening JavaUnit
classification or refreshing expected values.
