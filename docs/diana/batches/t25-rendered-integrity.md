# T25 rendered-result integrity

Baseline: f0aef75. Owners: Engine scheduling and P9 report rendering under
DESIGN §4/§4.1 and ALGORITHMS §7/§8. T25 consumes the accepted T23/T14
products. Preparation reuses macOS arm64 B01–B04 and unchanged draft-7 D085
sources, certificates, independent dexlib2/layout projections and expectations.

The full required report leaves D085's engine-render seam not implemented:
`tests/e2e/runner/adapter.py` does not dispatch that named seam, although the
public example already calls Engine and render. This is an unexecuted core
obligation, not an observed renderer failure. D065's engine-original-view seams
pass; their application counterparts remain unimplemented. The previous summary
that all remaining failures were application-only was therefore too broad.

This batch first connects the existing Engine/render call to the fixed D085
seam, without changing expected values, selector denominators or analysis
interfaces. Its invariant is that a local unsupported invoke-custom preserves
independent Java members, exact original dynamic-call/string facts and UTF-8
member spans. The original bytecode and independent projections are the facts;
a Mixed result does not establish whole-unit Java or ART execution. Existing
core tests are inspected before adding a public regression for an uncovered
obligation. Auxiliary tooling receives no tests.

Resource and cancellation behavior stays with existing RunContext/method and
fallback budgets. P9 consumes existing Java products and original snapshots;
it does not retry analysis or reconstruct values. Any actual engine failure
will be assigned to its owning producer before implementation. ALGORITHMS is
not edited.

The first fixed-case run now executes both engine-render requests. Both fail at
Engine::open with InputUnsupported(MapType(7)), rather than the required local
InvokeCustomUnsupported. Inspection also finds that P1 reference checks reject
CallSite/MethodHandle identities, EncodedValues rejects their tags and P9 lacks
resolved dynamic-reference facts. Merely accepting the map tag would bypass
missing validation and would not satisfy the raw-integrity oracle.

The operator approved the P1 checked read-only dynamic-reference product before
its implementation. `p1_input/dynamic.rs` now checks call-site and method-handle
encodings, referenced declarations and the first bootstrap/name/proto values.
Its guarantee is checked original metadata, not bootstrap execution or linkage.
Existing EncodedValues retains extra arguments as flat events. Method layout
validation uses the call-site prototype for argument words, wide adjacency and
move-result kind; normal raw validation checks the referenced metadata. P2 then
returns InvokeCustomUnsupported, without another analysis route. P9 serializes
the checked facts into the existing snapshot and keeps its final source spans.

Map kinds 7/8 are version-gated fixed tables, not variable data items. Their
strides, ranges and overlap with the next section are checked. Method-handle
unused words are retained but not assigned an invented zero requirement: the
[DEX format](https://source.android.com/docs/core/runtime/dex-format#method-handle-item)
labels them unused, and ART's `CheckIntraMethodHandleItem` checks kind/member
bounds without requiring zero. D085 itself contains a nonzero second unused word;
its input and expectations were not changed to accommodate the implementation.

The earlier T04 const-method-handle counterexample has no method-handle table.
Its known-width instructions stay readable, but reference zero is now a known
out-of-bounds reference, so the public regression expects Malformed rather than
the former capability-level Unsupported. The original words remain checked.
This strengthens the classified failure and does not change any D-case truth.

`tests/pipeline/dynamic.rs` crosses Engine, P1 validation, P2's unsupported exit
and P9 rendering using the unchanged D085 DEX and declaration facts narrowed from
independent preparation. It checks whole-class/single-method retrieval, original
method identities, Java after the failing member, exact dynamic-call references,
UTF-16, raw words, handlers and source slices. The same regression checks 039/040
const-method-handle/type prefixes as raw-only encodings with complete references.
Fixed mutations cover missing
references, illegal handle kinds/targets, wrong metadata tags/counts, invocation
words/results and overlapping fixed tables. Extra-value cases cover scalar,
string, proto/handle, FP bits and nested-container representations. Linkage of
mutated bootstrap arguments is not executed or certified. Cancellation and
allocation/work/output charging remain on existing method/fallback ledgers.

Execution on macOS arm64:

- `cargo fmt --all -- --check`, strict workspace Clippy and locked release build
  pass. Workspace nextest passes 190 tests with zero skips.
- `python3 tools/e2e.py run --case D085` passes both engine-render invocations;
  both application-default-text seams still fail as not implemented.
- The full required E2E run passes 158/181 seams and 79/86 whole cases. Only the
  two D085 engine-render seams change against T23. The whole-case count does not
  rise because D085 still requires its application seam.
- Original member identities, Java availability and classified failures on
  previously executed public reports have no regressions. ALGORITHMS and the
  fixed specification remain unchanged.

T25 and EngineCoreRC remain open for the rest of the card's artifact-boundary,
mutation/replay, resource, source-license and acceptance evidence. This batch
closes the identified D085 engine-render gap, not the complete T25 rollup.
