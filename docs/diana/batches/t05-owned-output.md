# T05 owned declaration and raw results

Goal: connect the T04 method-input producer to an actual public Engine request,
owned member results and rendering with original-member spans. Contracts: TASKS
T05, DESIGN §2.2–§3.3 and ALGORITHMS §7/§12. Producers: `84c1354` method inputs,
existing raw snapshots and request resource accounting. Reuse unchanged draft-7
preparation and existing core checks.

The operator requested T04/T05 implementation before E2E integration/execution.
This batch adds production consumers, not new tests, fixtures or auxiliary-tool
tests. TASKS.md remains unchanged. Executable Java recovery remains T06 and its
named producers; a raw result never substitutes for required Java acceptance.

Implemented `Engine::open/decompile`, original DexId/ClassKey/MethodKey and
caller-owned image origins. Open reuses the checked chain for repeated physical
slices, preserves separate logical catalogs and rejects conflicting origins or
duplicate logical identities. The engine retains no mutable control. Method
selection uses the complete name/prototype and does not resolve an unrelated
method's prototype before comparing its name.

The new owned `Report`, member facts and typed diagnostics retain original
identity and exact UTF16 declaration data. Per-member input failures produce
Unavailable with available original facts; supported raw output uses the existing
snapshot producer. Unknown widths remain original-word excerpts, never decoded
instructions. A recorded Malformed/BUG has priority over later result-allocation
failure; cancellation remains terminal. Request/normal/fallback charges stay on
the existing ledgers. Owned vectors, UTF16 copies, text growth and formatting use
fallible charged allocation and cancellable loops. No mutable graph or borrowed
input is retained by a report.

`p8_java::declaration` produces a private-constructor `JavaMethodDecl` for admitted
no-code declarations: checked owner/method modifiers, ASCII Java8 names and
primitive/primitive-array descriptors. Unhandled annotations, source names and
reference-type declaration evidence retain raw output. Directly determined JVM
parameter-word and ASCII MUTF8 name/descriptor limits produce typed JavaTargetLimit
with the actual item/count/limit. A declaration has no fabricated body. The class
result remains Mixed with an explicit class-assembly reason, rather than a fake
JavaUnit. Executable members have an explicit JavaAnalysisUnavailable reason.

`render` consumes only owned report data, displays the Mixed marker and actual
member reasons, and returns original-member spans in typed UTF8 byte coordinates.
The public `examples/inspect_class.rs` emits status, typed diagnostic codes, text
and spans after dropping both Engine and original bytes. It uses serde_json 1 for
presentation; production has no process/filesystem dependency.

Manually exercised this public operation on unchanged prepared D059, D060, D061,
D065 and the two D075 abstract-declaration boundary inputs. Observations retain
D059's original-word excerpt, D060's distinct return-only overload identities,
D061's complete original snapshot, the selected 041 member and the admitted versus
JavaTargetLimit declaration outcomes. Artifacts are under
`.cache/e2e/t05-owned-output/`. D059 uses `mutated.dex`; the initial `case.dex`
lookup failed and the corrected operation used the prepared original artifact.
These operation observations are not E2E comparison/compilation acceptance.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo build -p diana-decomp --release --example inspect_class --locked
cargo nextest run --workspace --all-features --locked --status-level fail --final-status-level fail
target/release/examples/inspect_class .cache/e2e/prepared/D059/mutated.dex 0 0 f '()V'
target/release/examples/inspect_class .cache/e2e/prepared/D060/case.dex 0 0
target/release/examples/inspect_class .cache/e2e/prepared/D061/case.dex 0 0 f '(I)I'
target/release/examples/inspect_class .cache/e2e/prepared/D065/container.dex 120 0 f '()I'
target/release/examples/inspect_class .cache/e2e/prepared/D075/abstract254.dex 0 0
target/release/examples/inspect_class .cache/e2e/prepared/D075/abstract255.dex 0 0
git diff --check
python3 tools/audit/desensitize_check.py --range HEAD
git diff --cached --check
python3 tools/audit/desensitize_check.py --staged
```

All 110 existing core tests pass without skips. No test functions or fixtures were
added. Format, Clippy and release builds pass. T05's thin existing-runner binding,
fixed semantic/raw comparisons and compilation evidence remain pending; the
operator-ordered E2E execution has not started. This is a completed production
consumer work item, not T04/T05 task-wide acceptance or an executable-Java claim.
