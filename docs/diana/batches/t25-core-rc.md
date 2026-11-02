# T25 engine-gate rollup

Baseline: f0aef75 plus the verified working-tree changes recorded in
[rendered integrity](t25-rendered-integrity.md) and
[artifact/replay evidence](t25-artifact-replay.md). Contracts: DESIGN §4/§4.1,
TASKS T25. ALGORITHMS is unchanged by this batch.

This record rolls up the engine gate only. It does not accept ProductRC: the
application seams remain unimplemented and their required invocations still fail.
It also does not claim Windows, Linux or ART execution.

## Required-set accounting

The selector is computed from `docs/diana/e2e_cases.json`, not from a fixed ID
endpoint. Cases whose `acceptanceGate` is `engine` or `engine-and-application`
contribute every engine seam named by their invocations; an empty selection is
rejected by the adapter rather than reported as a pass.

Observed on macOS arm64 at the recorded full run:

| Item | Observed |
|---|---:|
| Required cases / invocations (whole specification) | 86 / 177 |
| Engine-gate seams | 156 |
| Engine-gate seams accepted | 156 |
| Engine-gate cases with every seam accepted | 80 |
| All required seams (including application) | 181 |
| All required seams accepted | 158 |
| Whole cases with every required seam accepted | 79 |
| Workspace nextest | 192 tests, zero skips |

The 23 unaccepted required seams are application-owned: `extract-entry` for
D068–D070, `query-defined-symbols`/`query-reference-sites` for D066/D067,
`application-original-view` for D065 and `application-default-text` for D085.
Their engine counterparts pass. No required item was removed, skipped, filtered
by implementation status or answered with Debug where the specification requires
Java; the comparison that produced these counts is the same one that fails them.

## Known unsupported inputs and their raw coverage

These are legal inputs whose source profile is not implemented. They stay raw or
Mixed and never become successful Java:

| Input | Observed classification | Coverage |
|---|---|---|
| D085 `invoke-custom` with complete bootstrap metadata | `InvokeCustomUnsupported`, Debug with complete snapshot | Exact words, call-site name/handle/proto, extra values, UTF-16 and spans |
| `tests/fixtures/dynamic/poly.dex` `invoke-polymorphic`/range | `InvokePolymorphicUnsupported`, Debug | Register words checked against the prototype reference; both references retained |
| 039/040 `const-method-handle` and `const-method-type` | `JavaAnalysisUnavailable`, Debug | Checked handle/prototype references; version gate retained |
| Unknown container versions and missing fixed tables | Malformed or Unsupported before member analysis | Checked header/map envelopes, no fabricated fallback |

The abstract/native declaration profile, target-format overruns (`parameter_words`,
`constant_utf8_bytes`, `method_descriptor`) and explicit resource limits keep
their own classified outcomes; none is reported as a Malformed input fault.

## Mutation, boundary and replay evidence

`t25_seeded_structural_mutations_end_in_a_classified_public_result` applies a
fixed-seed mutation list to the two committed dynamic fixtures: header size,
checksum, table counts, data offset/size, map offset and code words, plus a
seeded sweep over the D085 code region. Every outcome must be a classified error
or a report whose members are readable Java, a parseable snapshot, a declaration
fragment or an explicit Unavailable; no mutation may reach InternalInvariant or a
panic. A separate bounded run cancels during the request and must terminate as
Cancelled instead of publishing a partial success.

The remaining required mutation categories already have focused public coverage
and are not duplicated here: LEB and encoded-value widths
(`tests/input/values.rs`), payload and switch extents
(`tests/input/structure.rs`), handlers and try ranges (`tests/input/debug.rs`,
`tests/input/structure.rs`), types/definitions (`tests/pipeline/ssa.rs`), Phi and
edge copies (T07/T08 cases), call sites and conditions
(`tests/pipeline/conditions.rs`), loops and storage (T09/T13 cases), state and
initialization (`tests/pipeline/initialization.rs`) and output budgets
(`tests/input/debug.rs` output-limit cases). Target boundaries are observed on
actual javac artifacts: static 255 / instance 254 admitted, their over-limit
siblings classified `JavaTargetLimit`, MUTF-8 65534/65535 admitted, D077's
30029-byte method and D078's 2206-entry pool admitted.

Replay: PYTHONHASHSEED=1/jobs=1 and PYTHONHASHSEED=2/jobs=12 produce identical
per-seam verdicts and identical member identity/status/failure facts in all 160
public reports. Generated text, local names and completion order are not compared.

## Sources, tooling and remaining evidence

Source and license review is recorded in
[SOURCE_MAP §6.1](../SOURCE_MAP.md#61-current-reuse-register): no reference code
is copied, both packages declare Apache-2.0 with the repository LICENSE, and all
fixtures are self-made. The polymorphic fixture is hand-written Smali assembled by
locked smali 2.5.2; D085's is a self-made builder; annotation sources and bytecode
come from the locked JDK and D8.

## Clean-revision acceptance

Commit b39805f was built and run without further edits. The run record reports
`working_tree_dirty: false` and that commit as its engine revision.

- `cargo fmt --all -- --check`, strict workspace Clippy and the locked release
  build pass; workspace nextest passes 192 tests with zero skips.
- `python3 tools/e2e.py run` selects the full required set from the JSON: 86
  cases, 177 invocations, 181 seams. Engine-gate seams accepted 156/156 across
  80 cases, with no failing engine seam. All required seams accepted 158/181 and
  whole passing cases 79; the 23 unaccepted seams are the application-owned
  entries listed above.
- Fixed-case D085 engine rendering, D075–D078 target/declaration boundaries,
  D086 default-versus-explicit limits, D055/D059 rejection and the mutation,
  replay and license evidence in this and the two preceding batch records all
  pass at this revision.

Acceptance: EngineCoreRC for the `darwin-aarch64` profile on this self-made suite
is accepted at b39805f. ProductRC is not accepted; no application seam was
emulated to reach this count, and no required Java answer was replaced by Debug.
Not run: Windows, Linux, ART, representative APK/corpus performance and any
opt-in private corpus.
