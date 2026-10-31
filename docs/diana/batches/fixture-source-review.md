# Fixture source construction and review

Goal: construct and review the complete self-made sources before original-input
certification. Input: commit `8345dbf` and the unchanged draft-6 specification
identified by the B01 record. Contracts: TASKS B01–B04/§4, DESIGN §2/§5/§6 and
ALGORITHMS §1/§7/§12. Expected values are not copied into this record.

Construction covered D001–D086 in isolated worker worktrees. Two independent
reviewer runs timed out without usable reports; they supply no approval. The
parent completed the review and retained only self-made sources, recipes and
actual Drivers under `tests/e2e/sources/`; generated binaries remain in `.cache/`.

Reviewed corrections:

- D046 field_id layout and explicit-default binding; D055 opcodes/handler offset,
  constructor/proto references and rebuilt count faults; D056/D065 Object ctor
  binding; D085 proto/shorty uniqueness, constructor and method_id ordering.
- D075 type-list map code, direct/virtual grouping and delta reset; D076 shorty
  and actual returned-unit Driver; D078 unique strings and interface flags;
  boundary builders' method_id field ordering.
- D049 specified no-argument outer construction and per-object capture; D019/D048
  distinct null-string observations; D079 genuine getstatic/initialization groups;
  D081 removal of an unintended static-field effect.
- Actual locked Smali execution exposed an invalid API option and array payloads
  outside their methods in D027/D073/D074. Corrected syntax preserves the specified
  payload semantics.
- Locked D8 with D029 `-g:none` rematerialized the old value inside the handler,
  eliminating the tested definition. `-g:vars` retains that definition and the
  separate normal commit without changing source, invocation input or expectation.
- Concrete per-invocation full method selectors cover the malformed/unknown-width
  method targets; grouped calls retain individual original method bindings.

D065's recovered-output Driver calls the actual `fixtures.Case`, never the mirror
classes. D061 catch declarations have independent source evidence, and D085's
external bootstrap no longer falsely claims a class_def. Drivers are excluded
from DEX inputs. All required invocation IDs and symbolic targets agree with the
specification; actual binary identities are bound by the independent manifest.

Source construction and review are complete. Locked rebuilding, source/raw
observations, supported platform and B01–B04 acceptance evidence are owned by
[b02-b04-preparation.md](b02-b04-preparation.md), not a second progress counter.
