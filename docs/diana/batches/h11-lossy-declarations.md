# H11 — Anonymous ownership scope and lossy declarations

**Phase ownership**: P1 input ownership and P8 declarations. The application only
consumes the resulting products; no analysis path or product authority changed.

## Observed failures

A real application sample recorded two independent losses before any analysis ran:

- one anonymous class anywhere in an input aborted `p1_input/ownership.rs::collect`
  for the whole request, so every class reported `AnonymousOwnership`;
- declaration rendering required catalog class facts for every reference type, so
  abstract and native members of interfaces were declined as
  `JavaDeclarationUnsupported` even though their own DEX descriptors are complete.

## Changes

- An `InnerClass` entry whose `name` is null now marks that definition anonymous
  instead of returning an input error. Anonymous definitions keep their enclosing
  and member relations, are excluded from the named-nested list, and no longer
  deny ownership facts for unrelated classes.
- `p8_java/declarations.rs` renders a reference type from its own descriptor when
  the catalog has no class entry. The rendered name is lossy by construction; the
  original descriptor and member identity stay in the report and diagnostics.

Verified Java, malformed-input classification, budgets and cancellation are
unchanged. An anonymous definition is still not nameable as a source-level nested
class, and a missing class entry does not add access flags, hierarchy or throws.

## Evidence

`crates/diana-decomp/tests/pipeline/anonymous.rs` decompiles a class that has an
anonymous sibling and requires that no `InputUnsupported` diagnostic appears and
that a member is Java. Fixture `tests/fixtures/anonymous/classes.dex` was produced
with the locked `javac`/`d8` tools from a two-class source.

Real application, 25 sampled classes: Java members went from 0 to 5 across two
classes (`IMonitor` now renders
`public abstract void onAppLogEvent(java.lang.String p0, org.json.JSONObject p1);`).
Every class stays `Mixed`: the remaining losses are class-assembly annotation
gates, branch coverage and initialization. Those belong to the provisional
emission batch, not to this one.

Workspace gates at this point: 235 tests pass, strict Clippy and formatting pass.
