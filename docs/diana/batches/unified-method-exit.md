# Unified method and region boundary

This slice removes the final heap method route and makes P7 consume shared P6
storage before P8 lowering. Contracts: DESIGN §4, ALGORITHMS §5–§7 and the
[architecture baseline](architecture-baseline.md).

The operator approved archiving the uncommitted static-field draft. Local
`.cache/e2e/frozen-static-field-draft-419790c/` retains the full modified files,
original baseline files, binary-safe patch, draft batch note and base commit.
The draft is withdrawn from active source, not committed as an accepted feature.

## Producers and exit

`engine::normal_member` now has one ordinary-method chain: shared P2/P3 → P4/P5
→ P6 → sealed P7 PlannedBody → `p8_java::verify_method` → P9. Declaration-only
input retains its separate declaration contract. The heap builder, verifier,
method AST and renderer are deleted. The extra fill alias/register classifier is
also deleted; component/definition/fill proofs remain in shared phases.

P7 projects only control/edge identity from read-only StorageBody. Its ordinary
loop/diamond candidates and verifier no longer import a P8 method AST. Switch
policy is selected in P7 and checked again against lowered keys. P8 creates and
checks one common block/statement/copy representation, then adapts it to natural
structure or dispatcher. The separate pure-diamond Java verifier/AST is deleted.
P9 reads those same validated statements and edge copies for either form.

Unsupported analysis reaches one NormalMember::Raw result. Its small read-only
reference classifier retains missing-call and external-constant-field reasons;
it neither tracks values nor retries analysis. Hard errors, cancellation and
reserved raw resources retain the existing request boundary.

## Evidence

- Fmt, strict workspace/all-target/all-feature Clippy, locked release and
  cargo-nextest pass: 122 tests, zero skips. The existing four M11 counterexamples
  remain; their test projects control facts from its public Engine result rather
  than passing a P8 AST to the production region planner. No tests were added.
- Full `run-lsd83kr4` retains 86 cases, 177 invocations and 181 seams. It exits 1
  with 51 cases/117 seams passing; no passing seam is lost/gained against
  `run-4a8u0jw8`. Dispatch-only `run-nz9fmy_8` passes the 12 selected existing
  cases and all 27 invocations/seams.
- The unchanged shared-fields Driver matches all 88 retained independent Java
  observations after current emitted Case/Reader/SpecialReader methods compile
  in their original helper context. Field/call/array failure prefixes, loop
  effects, unused reads, override/null and direct/super behavior remain observed.
- Every existing empty-fill member outcome is retained, including original
  InvalidArrayPayloadType and UndefinedRegister causes/coordinates.
- Comparison of public member states across the complete runs found one change:
  D044 get became Debug after withdrawing the static draft. Clean committed
  `419790c`, without that draft and with the same D044 catalog/control/input,
  already reports get as Debug. `clean-baseline-D044.json` records this check.
  No accepted-baseline Java member is lost. D020/D021/D041/D042 partial member
  states remain unchanged; Mixed is not accepted JavaUnit.

Artifacts are under `.cache/e2e/unified-method-exit/`. Diff and sensitive-input
checks pass. Fixed inputs/expectations, TASKS and denominators are unchanged.
Final historical architecture reacceptance is recorded separately.
