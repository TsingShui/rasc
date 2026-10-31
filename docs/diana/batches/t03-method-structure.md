# T03 method structure checks

Goal: check whole-method instruction/register/reference and control-flow
boundaries through public input APIs, with local failure preservation and real
method-budget/cancellation behavior. Producers: typed T02 views and the shared
T03 instruction-layout helper. Contracts: DESIGN §2/§3.7/§5.1 and ALGORITHMS
§1.2–§1.3/§1.5/§13. Reuse unchanged schema 2, draft-7 preparation evidence.

Fixed obligations: D055/D056/D059/D062/D063/D086 and the F02 boundary cases.
Expected values remain in `e2e_cases.json`; Rust tests add independent structural
recipes rather than refreshing truth from engine output.

Not in scope: complete invoke/result type binding, debug metadata validation, SSA and Java recovery.

The product is a sealed CheckedMethodLayout, not ValidatedRawMethod: stronger
metadata and binding guarantees retain their owning validators. Shared opcode
facts drive register spans, branch/switch behavior and potential throwing sites.
Two-pass boundary checks distinguish executable instructions from payloads;
reachable fallthrough checks keep dead padding after exits legal.

Scratch allocation is derived from the checked code range and charged before
fallible exact reservation. Every scan, payload, handler and reachability loop
charges the normal method and request ledgers. A read-only raw-layout visitor
uses the independent fallback budget; cancellation remains terminal.

Files/functions: `p1_input/structure.rs` owns
`ProgramIndex::check_method_layout`, `RawMethodView::visit_layouts`, boundary
storage, register checks, switch targets, handler checks and reachability.
`formats.rs` supplies shared register spans, flow/result roles and may-throw
facts. `cursor.rs` shares the existing byte/LEB rules with the actual request
and normal-method charging callers. `control.rs` enforces the existing DESIGN
§10.3 normal/raw code-unit fields. Test fixtures are shared by the existing
catalog cases and new `tests/input/structure.rs` cases.

Source review used cached AOSP `dalvik-bytecode.html` and
`dex-constraints.html`, especially B17/B19–B22, and SOURCE_MAP §3/§6.
The boundary table uses one nibble per checked code unit. The pending stack
reserves at most the counted instruction starts and marks before enqueueing,
so cycles cannot grow it repeatedly. Try lookup uses the checked sorted original
intervals, and original handlers retain their encoded priority. Source identity
prevents applying another image's reference namespace.

The reviewed rustc 1.98.0 standard-library `alloc::raw_vec::RawVecInner::grow_exact`
sets logical capacity to the requested element count. Both scratch buffers are
fresh non-ZST Global vectors with exact reservation; an unexpected capacity is
a hard invariant error. Initialization checkpoints use bounded chunks and later
pushes remain within the precharged capacity. The ledger remains cumulative
logical allocation, not RSS.

Red: the public structural selector failed compilation while the owning APIs
were absent. The shared-fixture extraction first preserved all 42 existing
tests. Green: 63 core tests pass with no skips, including 21 new structural and
resource/cancellation cases. This is functional public input-operation evidence,
not acceptance of the fixed Engine invocations.

Commands executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t03_method) | test(t01_)'
cargo nextest run -p diana-decomp --test contracts --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
python3 tools/audit/desensitize_check.py --range HEAD
```

Artifacts: `target/debug/` and `target/release/`. The audit uses a temporary
Git index to include new files without staging the real index. Unchanged
preparation evidence is reused without SHA-256 verification.

Remaining obligations: T03 original-information snapshots/rendering and full
invoke/result binding; T04 metadata completeness and the final P1 seal; T05
public Engine/E2E integration. No T/H acceptance or fixed Engine pass is claimed.
