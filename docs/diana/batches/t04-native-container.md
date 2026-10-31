# T04 native logical DEX views

Goal: read the original D065 041 container and select its logical programs without
rewriting bytes or merging their table namespaces. Contracts: TASKS T04,
ALGORITHMS §1.1/§1.5 and DESIGN §2/§3.2/§5. Producers: checked byte cursors,
035 table/declaration readers and original method snapshots. Reuse unchanged
TASKS §3 draft-7 B01–B04 input certification, including D065's published-format
projection and independent Java mirrors.

The public core seams are container admission, program selection, declaration
and code retrieval, method-layout checking and original debug snapshots. Existing
header tests own ordinary size/range/quota/cancellation behavior; existing method
tests own foreign-image rejection. Extend those where the same control path
applies. New cases target the 041 header chain, shared forward physical ranges,
logical namespace isolation and references into or across another header.

Implementation supplements and verification evidence belong here. TASKS.md remains
unchanged under the operator's instruction. The annotation-ownership work item
is committed as `c51dcae`; T04 remains incomplete until its remaining obligations
have their required evidence.

Source review: AOSP [DEX header](https://source.android.com/docs/core/runtime/dex-format#header-item)
and the unchanged D065 builder/projection define original physical coordinates,
header-chain steps and permitted forward sharing. `DexContainer::open_with`
validates the physical chain once without allocation. `program` selects an actual
header; selection and item-boundary scans traverse only the checked chain.
`read_member_header/check_map` retain header fields and validate logical tables
against the physical container. For 041, the old data fields do not bound data.
`item_end/check_data_range` exclude every header from fixed and variable item
ranges. The selected map and link range also bound item reads. No global
cross-member disjointness rule rejects shared tables.

`ProgramIndex::from_container/item` retain the borrowed container and resolve
all references within the selected logical catalog. `RawMethodView` carries its
original header offset and DEX version. `check_method_layout` rejects a view from
another logical member even when both borrow the same physical slice. Original
snapshots and word excerpts now include `header_offset`; decoding uses the
selected version. Known newer reference domains retain their width and explicit
Unsupported information rather than reporting an internal invariant failure.

Red/green: the D065 public retrieval initially failed as Unsupported. The first
native implementation passes that case. A namespace regression then exposed
missing logical snapshot identity, and an overlap mutation exposed a map inside
a class-def range; both now fail or retain facts as required. A newer-version
reference case exposed an incorrect BUG classification and now yields explicit
Unsupported while retaining subsequent instructions. The table-driven cases
cover chain sizes/steps/magic, invalid member selection, ignored 041 data fields,
forward sharing, earlier references, fixed/map overlap and a string whose
terminator would consume the next header. Both D065 members' constructors and
scalar bodies pass the public structural checker, and foreign-member checking
fails as Malformed. Physical image admission is tested with two logical views,
zero request allocation and an exact cumulative-work boundary. The existing
header cancellation regression also exercises the 041 chain.

The Rust tests consume the certified `.cache/e2e/prepared/D065/container.dex`;
a missing preparation artifact is a test failure with an explicit preparation
message. They do not download, regenerate or fingerprint fixtures. Independent
expectations remain D065's frozen source, words and published-format projection.
A release public-library probe compares all four methods across both members to
`container.dex.layout.json`, including original identity, complete signatures,
flags, code offsets/words, counts and handlers. All comparisons pass. Evidence:
`.cache/e2e/t04-native-container/{probe.rs,probe,snapshots/,comparison.json}`.
The probe uses `rustc --edition=2024 -C panic=abort -C lto=thin`, the active
release library and the repository's original input; it writes no normalized DEX.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t04_native_container)' --status-level fail --final-status-level fail
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t04_native_versions)' --status-level fail --final-status-level fail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked --status-level fail --final-status-level fail
git diff --cached --check
python3 tools/audit/desensitize_check.py --range HEAD
python3 tools/audit/desensitize_check.py --staged
```

The workspace gate passes all 110 core tests without skips. The complete P1 seal,
newer reference-table support and T05 Engine integration retain their existing
requirements. D065's Java behavior and the application seam remain subject to
its fixed E2E acceptance; original-fact agreement is not task-wide acceptance.
