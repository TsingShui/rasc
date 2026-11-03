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

## Coverage baseline

`apps/diana/examples/app_coverage.rs` samples classes from one input, runs the
ordinary request chain once per class and aggregates member outcomes with the
diagnostic codes. It is a development measurement caller, not part of the gates.

First recorded run on a 22 MB application, 120 sampled classes of 15,753:

| Member outcome | Count |
|---|---|
| debug (declined member) | 681 |
| declaration only | 96 |
| java | 2 |
| unavailable | 5 |

| Diagnostic code | Count |
|---|---|
| BindingEvidenceMissing | 420 |
| JavaDeclarationUnsupported | 267 |
| ClassAssemblyUnavailable | 159 |
| BranchJavaUnsupported | 141 |
| InitializationUnavailable | 61 |
| JavaAnalysisUnavailable | 40 |
| FinalAssignmentUnproved | 8 |
| InputMalformed(InvalidEncodedValue) | 7 |

No class reached `JavaUnit`. Median class time was 238 ms, maximum 5.0 s.
A reference-evidence experiment then added an opt-in `EvidencePolicy` that binds
calls from the original DEX reference instead of requiring catalog declarations.
It is kept behind the policy and stays off by default. Measured on the same
sample with the policy on:

| Member outcome | Declared | TrustedReferences |
|---|---|---|
| debug | 681 | 681 |
| java | 2 | 2 |

The per-code split explains the null result: `BindingEvidenceMissing` fell from
416 to 207 and `BranchJavaUnsupported` rose from 141 to 218, so the binding work
moved straight into structural decline. Declaration rendering (`JavaDeclarationUnsupported`,
23) is no longer the top cause, and `JavaAnalysisUnavailable` rose from 40 to 155
because more methods reached the flow decoder. No member reached Java: every
remaining path ends in a structural, initialization or analysis decline that has
nothing to do with declaration evidence.
