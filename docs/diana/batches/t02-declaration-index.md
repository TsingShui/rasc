# T02 declaration index and local method ranges

Goal: resolve original classes and complete method signatures through a borrowed
035 index, retaining method-local failures while visiting other declarations.
Producer: T02; contracts: DESIGN §2/§3.2/§3.7/§5.1 and ALGORITHMS §1.1/§13.
The checked reader/header and T01 control producers are available. Accepted
B01–B04 draft-7 preparation evidence is reused without rebuilding or hashing.

Fixed obligations: schema 2, draft-7 D055/D059/D062/D063/D086 and the T02 F01/M02
catalog/range cases. Rust tests add self-made abstract/overloaded/referenced-only
and malformed declarations; frozen expected values remain unchanged.

Not in scope: instruction verification, Java recovery, 041 and complete annotation/debug decoding.

## Implementation and verification

Use borrowed string/prototype and class/method views, resolving metadata on demand.
MUTF-8 must preserve UTF16 and validate declaration syntax without an ASCII-only
substitute. Class-data delta groups remain distinct. A readable encoded-method
record publishes its original index even if its declaration or code is damaged;
resource/cancellation failures propagate immediately. Invalid variable-length
boundaries retain an explicit class-level error and already-delivered members.

Code views prove checked instruction/try/handler ranges, not verified bytecode.
Handler-offset membership uses the format's u16 offset domain, avoiding an
input-sized allocation. No production helper runs a process or accesses files.

Files: `src/p1_input/{catalog,code,strings}.rs`, existing cursor/header/control
modules and `tests/input/catalog.rs`. Changed functions: `ProgramIndex::open`,
`class`, `visit_classes`, `string`, table/prototype/field lookups;
`ClassView::visit_methods`, `Prototype::matches`, `PriorGroup::contains`,
`DexString` read/match/name/descriptor routines, `read_code`, `check_handlers`,
and `RunContext::check_string_units`.

Section lookup shares the header's format-kind mapping and uses fixed indexed
slots. Cross-group duplicate checks replay sorted delta streams in linear time.
The handler membership bitmap uses the actual u16 format domain, not a configured
quota. Metadata and string checks charge the same request; all byte/LEB loops
observe cancellation. Library indexing performs no input-sized heap allocation.

## Verification

Independent format evidence: the existing cached AOSP `dex-format.html` sections
SimpleName/MemberName, string_data_item, proto_id_item, class_data_item and
code_item, attributed in SOURCE_MAP. The Rust recipe sorts strings/types/method
IDs independently and rebuilds all affected offsets for length-changing handler
mutations. Unicode method order and delta-group resets were checked against the
published rules rather than engine output.

Platform: macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149.
The initial catalog selector failed to compile because ProgramIndex was absent.
The first implemented suite passed 21 tests; added encoding/resource/delta cases
passed 30. Final default and all-features runs each passed 34 tests, with no skips:

```sh
cargo nextest run -p diana-decomp --test contracts --locked
cargo nextest run --workspace --all-features --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
```

An intermediate Clippy run identified constant-size chunk iteration; the code now
uses fixed-size array chunks. Final fmt, Clippy and release gates passed. Artifacts
are under `target/debug/` and `target/release/`. Tests compare original code slices
and identity-linked classification, not private IR or phase invocation counts.

Source review checked checked-arithmetic bounds, map-section envelopes, complete
signature comparison, fifth-byte LEB rules, class-data prefix preservation,
resource propagation and i32::MIN handler counts. Frozen D-case engine invocations
remain unexecuted; the full variable-item/metadata and instruction checks retain
their T02–T04 obligations. T01/T02 task acceptance remains open.

Next: instruction-format decoding and original-information debug output, using
these borrowed views and retaining the same original method identities.
