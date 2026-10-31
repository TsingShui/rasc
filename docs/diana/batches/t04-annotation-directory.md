# T04 annotation ownership

Goal: retain original class, field, method and parameter annotation associations
in the public snapshot. Producers: T02 declaration identity and checked metadata
ranges; T04 iterative encoded values. Contracts: DESIGN §2/§5 and ALGORITHMS
§1.4–§1.5/§7/§12. Fixed obligations: F03/M04, D050/D055/D061/D062/D063.
Reuse unchanged draft-7 B01–B04 preparation from TASKS §3.

Coverage review: existing tests own scalar bits, nested value grammar and ordinary
quota/output behavior. New table-driven inputs target annotation association,
ordering, visibility, missing definitions, invalid ranges and parameter counts,
with code-body isolation and local failures. Existing output-boundary and
cancellation tests now also consume annotation metadata.

Source review: AOSP [annotations_directory_item](https://source.android.com/docs/core/runtime/dex-format#annotations-directory-item),
annotation sets, set-reference lists and encoded annotations define ownership,
sorted IDs/types/names and visibility. The implementation copies no reference
code. `annotation_directory/annotation_offsets/annotation_parameters/next_annotation`
check borrowed tables and original annotations. `AnnotationMembers::open/check`
replay the two class-data groups alongside each directory table without opening
other code items. Field and method identities retain distinct types.
`EncodedValues::annotation` reuses the existing iterative value grammar;
`write_entries` is shared by static values and annotation items.
`write_directory/write_set/write_parameters/write_associations` emit original
facts and completeness, retaining checked prefixes and independent associations
after local input failures. Request resource failures and cancellation remain
terminal. Tables check byte bounds before charging counts; readers and emitters
charge the existing fallback ledger without allocating an ownership index.

Red: the three pending public annotation tests ran and all failed: snapshots
omitted the directory and did not classify malformed annotation input. Green:
the same tests pass with positive and malformed table-driven inputs, including
all visibility values, empty sets, zero parameter-set references, invalid counts,
alignment, missing declarations, duplicate IDs/types and retained set prefixes.

Independent comparison reuses locked Temurin 21.0.8+9, R8 8.3.37 and dexlib2 2.5.2.
The self-made `AnnotatedCase.java` compiles with `--release 8 -proc:none -g:none`;
D8 uses `--debug --min-api 23`. Its class, field, method and parameter annotation
facts include a wide parameter, an unannotated parameter, arrays, type references,
NUL and an isolated surrogate. Three class directories and 19 ownership/value
comparisons agree with dexlib2. The unchanged prepared D050 adds one directory
and 14 comparisons, including original system metadata. Prepared D061/D062/D063
also run through the public snapshot probe. These are original-fact comparisons,
not complete engine E2E acceptance.

Evidence: `.cache/e2e/t04-annotation-directory/` contains the Java source, compiled
classes/DEX, commands/logs, dexlib2 projection, snapshots and `comparison.json`.
The existing `.cache/e2e/t04-debug-program/probe.rs` calls only public library
operations. Its release compilation requires `-C panic=abort -C lto=thin` to match
the workspace profile and avoid Apple LLVM bitcode incompatibility. An initial
D8 invocation had no class inputs because its argument list was collected before
javac completed; the corrected invocation and independent projection succeeded.
The prepared D061 input is `case.dex`, not `dex/classes.dex`.

Executed on macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149:

```sh
cargo nextest run -p diana-decomp --test contracts -E 'test(t04_annotations)' --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
git diff --check
python3 tools/audit/desensitize_check.py --range HEAD
```

The closing workspace run passes all 104 core tests without skips. Format,
Clippy, release build, diff whitespace and the HEAD-range sensitive-input audit
pass. Unchanged preparation evidence is reused without fingerprint verification.

Remaining obligations: native 041, the complete P1 seal and T05 Engine integration
retain their existing acceptance requirements. The operator requested continued
small-batch development with a commit after each completed work item. TASKS.md
remains unchanged; implementation evidence and task supplements live in batch
records.
