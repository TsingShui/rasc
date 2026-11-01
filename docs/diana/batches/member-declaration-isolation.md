# Member declaration failure isolation

Goal: retain an independently recoverable method when a locatable sibling has a
malformed name or prototype. Contracts: TASKS T02/T03/T06 and §4;
DESIGN §2.3/§4; ALGORITHMS §1.1/§9.4. Preparation reuses the accepted macOS arm64
profile and unchanged draft-7. Fixed regressions reuse D009 and the existing
input, binding and source-group cases.

P1 owns original declaration/name validation and exact method selection. P8
consumes those facts to check source-name/signature collisions. A sibling-local
failure belongs to its original MethodId; it does not prove that the selected
method is malformed. A same-name declaration with an unreadable prototype does
not establish a collision-free Java signature. Cancellation, resource failure
and internal invariants remain hard failures of their existing control scope.

The existing public catalog tests cover local name/prototype damage, and the
public Engine sibling-body tests do not exercise declaration damage through
selection and P8 source admission. The review reproduces
the missing boundary by changing only D009's constructor name or prototype
reference. Original input and before-change results are under
`.cache/e2e/t01-t15-review/`.

## Implementation and verification

`ProgramIndex::method_header` shares the original owner/name parser with
`method_declaration` and the independent `method_name` lookup. `resolve_method`
continues past locatable candidate declaration errors while preserving an error
when no valid exact match exists. Its duplicate-exact-match rejection is retained.
`p8_java::method::source_group_admitted` isolates unrelated declaration damage;
a matching name with an unreadable prototype rejects Java as Unsupported instead
of attributing the sibling's Malformed error to the selected method. Invalid
names do not supply a valid source competitor.

The added public Engine regression extends the existing self-made tiny DEX:
undamaged input, invalid sibling name, unrelated invalid prototype, same-name
invalid prototype and a duplicate exact signature. Method and class requests
check original identities, Java availability, classified sibling failures,
complete raw on source rejection and actual rendered member spans. Requests for
the damaged declarations retain their parse errors. The regression fails before
the production change with InvalidReference during exact method selection and
passes afterward. No new fixture builder or oracle is introduced.

The candidate scan retains its original work and metadata charges. Independent
name reads use the same checked cursor and cancellation points; the retained
local error occupies constant space. The normal/raw ledgers and single raw exit
are unchanged. Existing public resource/cancellation regressions are reused.

On macOS arm64 with Rust 1.98.0 and cargo-nextest 0.9.149:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
git diff --check
```

Rust gates pass with 130 tests and zero skips. Full `run-9zorol_p` executes the
unchanged required 86 cases, 177 invocations and 181 seams, with 51 cases and 117
seams passing. It exits 1 for the remaining capabilities. Its passing seam set
and all 2379 original member observations (status, language, failure kind/code/
cause) equal the T16 baseline `run-a3_g9k2k`. Fixed D009's real javac/JVM Driver
checks pass. The locked JVM toolchain remains Temurin 21.0.8+9, Java8 targeting;
ART and other host platforms remain not-run.

The release public example also rechecks both reviewed D009 mutations, through
method and class requests. All four retain `add` as Java; class requests retain
the damaged constructor as Unavailable with its original Malformed diagnostic.
Logs, public results and member comparison are under
`.cache/e2e/member-declaration-isolation/`. Diff and sensitive-input checks pass.

Decision: both reviewed member-declaration propagation defects are corrected.
Same-name uncertainty and exact ambiguity remain explicit rejection paths.
