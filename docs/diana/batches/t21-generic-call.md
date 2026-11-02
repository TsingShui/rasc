# Object-bound generic call binding

Goal: admit a static generic identity call with exact Object argument/result
views through the ordinary P4/P5/P8 chain. Baseline: 8efc1ad plus the verified
T21 slices. Contracts: DESIGN §4/§4.1; ALGORITHMS §9.4/§12; TASKS T21.
Preparation reuses accepted macOS arm64 B01–B04 and unchanged draft-7.

P0 supplies the original method Signature. P4 accepts only the single
Object-bounded identity signature with matching erased descriptor and static
public target. Fixed Object source formals select Object for the method variable;
other applicable generic competitors remain unproved. P5's existing Object use
conversions and P8 final binding remain authoritative. No new stage inputs,
entries or generic inference engine are introduced.

Independent evidence uses a self-made caller and generic helper through the
public Engine, javac and a fresh JVM. Missing/contradicted signatures retain
classified failures. Existing nominal overload and method resource regressions
remain applicable. New signature scans use existing work/allocation ledgers.

## Execution

P4 raw_signature::object_identity validates a public static target with the
single Object-bound Signature and exact erased Object argument/result descriptor.
The existing overload selector admits only that selected generic declaration;
other applicable generic competitors remain Unsupported. P5 fixes existing
Object use conversions and P8 uses the existing call binding and operation replay.
No class generic flag is globally relaxed.

The tracked Caller.smali, Caller.java, GenericDep.java and GenericDriver.java
under crates/diana-decomp/tests/fixtures/covariant-bridge/ supply independent
input/reference behavior. Assemble caller.dex with pinned smali 2.5.2. Recover
call through the release inspect_class example with caller-catalog.json; compile
the recovered Caller with the unchanged GenericDep/GenericDriver using locked
javac --release 8. Compile the original Caller separately with those same sources.
Fresh JVM executions both produce true, true, x for object identity, null and
String inputs. The recovered target classpath contains no original Caller class.
The public bridge.rs table rejects missing Signature and a contradicted String
bound, retaining the same original member instead of inventing a cast.

Observed on macOS arm64:

- cargo fmt --all -- --check: passed.
- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings:
  passed.
- cargo build --workspace --release --locked: passed.
- cargo nextest run --workspace --all-features --locked: 166 passed, zero skips.
- python3 tools/e2e.py run: 76/86 cases and 152/181 seams pass; all 177 invocation
  scenarios execute. Exit 1 retains outstanding requirements. No fixed expected
  value or case disposition changed for this supplemental capability.

The bounded generic call profile is verified. Generic parent/interface source
substitution and bridge argument-cast profiles retain their T21 obligations.
