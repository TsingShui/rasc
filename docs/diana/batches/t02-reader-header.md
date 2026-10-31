# T02 checked reader and DEX header

Goal: implement checked byte/LEB reads and the DEX 035 header/fixed-table/map
envelope through the public input module. Producer: T02; contracts: DESIGN §3.2,
§3.7 and §10.3, ALGORITHMS §1.1/§1.5 and §13. The T01 resource-control producer
is available. Accepted B01–B04 evidence is reused unchanged.

Fixed specification: schema 2, draft-7. D055/D059 remain downstream malformed
input obligations; this slice exercises the F01/M01 checked-reader/header subset
and does not claim their complete engine acceptance. The independently authored
Rust tiny-DEX recipe and malformed variants live in `tests/input/`.

Not in scope: declaration lookup, method-local isolation, instruction decoding and 041.

## Implementation and verification

Start with public input tests exposing the missing reader/header implementation.
Files: `src/p1_input/{mod,cursor,header}.rs`, `tests/input/`, `tests/contracts.rs`.
Borrowed reads use checked range arithmetic; LEB accepts non-shortest encodings
and rejects fifth-byte overflow. Header parsing preserves checksum/signature as
fields without hash verification, and checks fixed tables plus the map envelope
before any input-derived allocation. Variable map item extents remain the owning
item parser's obligation.

Request work, image-byte limits and cumulative metadata charges use RunContext.
Metadata currently charges map records; later item parsers charge their own records.
The header parser allocates no input-sized storage. Changed functions include
`checked_range`, `Cursor` reads, `read_header`, `section`, `check_fixed_overlap`,
`check_map`, `RunContext::{check_image_bytes,charge_metadata}` and `InputError::kind`.

## Execution evidence

Platform: macOS arm64, rustc 1.98.0, cargo-nextest 0.9.149.

The initial command below failed to compile because the public input module and
input limits were absent. After implementation it ran 14 passing tests; adding a
non-empty table/unaligned string-section fixture produced the final 15-test suite.
The final run passed all 15 tests with no skips:

```sh
cargo nextest run -p diana-decomp --test contracts --locked -E 'test(t02_)'
```

The final active-workspace gates passed:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo build --workspace --release --locked
```

The all-features nextest run passed all 15 tests with no skips, both before and
after the portability review. Default-feature tests and all-features tests cross
the public input library. Source review checked range-before-slice ordering,
allocation-free map type uniqueness, distinct alignment rules and signed LEB
extension. Large malformed test counts remain representable before the range
check on both 32-bit and 64-bit hosts; only macOS arm64 was executed.

Recipes are in `crates/diana-decomp/tests/input/{header,reader}.rs`; build artifacts
are under `target/debug/` and `target/release/`. No frozen expected values,
certificates or reference source were changed. D055/D059 engine invocations remain
unexecuted; declaration and method-local validation remain T02 obligations.

Next: consume these checked table ranges in declaration indexing, bind original
class/method identity, then validate each method range independently.
