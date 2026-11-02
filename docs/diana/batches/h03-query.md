# H03a lightweight query core

Baseline: 15f39f3. Contracts: DESIGN §6.3–§6.4, §7 and TASKS H03; ALGORITHMS
unchanged. The operator approved the engine read-only query facts in the
preceding discussion; this record documents their use.

## Interface addition consumed

The application cannot read a name or a defined field from the existing public
engine surface, so the engine gained five read-only facts. None of them starts
analysis, constructs a stage product or opens a scheduling entry:

- `DexString::units(&mut RunContext) -> Result<Vec<u16>, InputError>` copies the
  complete original UTF-16 value under the request ledger.
- `Prototype::visit_parameters(...)` exposes the original parameter descriptors.
- `ClassView::visit_fields(...)` visits original `class_data` field definitions
  through the existing internal member visitor.
- `ProgramIndex::string_ids_len()` exposes the original string table size.
- `FieldDeclaration`, `FieldEntry` and `FieldStorage` became public types so the
  visitor result is usable; `FieldStorage` also derives the usual comparison
  traits. `MethodBody::Code(RawMethodView)` was already public, so xref needs no
  further access.

These facts are P1 checked reads. They do not expose P2+ products, do not create
alternative analysis entries, and are consumed only by the application query
module.

## Products and behaviour

`analysis/dex.rs` turns one root image's original tables into rows that keep the
original identity: class-definition rows carry `ClassDefId`, descriptor and flags;
method rows carry only `class_data` definitions with `MethodId`, name, complete
prototype descriptor and flags; field rows carry `FieldId`, name, type, flags and
storage; string rows carry the exact `string_ids` index and UTF-16 units.
`analysis/query.rs` resolves the root images once, exposes their display labels,
and streams rows in worker completion order.

- `entries` lists every physical central-directory record with `name`,
  `compressed_size`, `uncompressed_size` and numeric `method`, without
  deduplicating or sorting. A bare DEX is `NotArchive`.
- Concurrency: one worker per admitted image, bounded by `jobs`. Each worker owns
  one image at a time and sends batches through a channel whose capacity equals the
  worker count, so pending memory is bounded by workers times one batch. With no
  configured batch size a batch is one row.
- Delivery order is completion order. There is no ordinal wait, no global sort and
  no reorder buffer.
- Bounds are explicit: `max_rows` and `max_batch_rows`. Exceeding `max_rows` is a
  `Budget { limit: Rows }` error; it never returns a truncated successful prefix.
  A configured `jobs` of zero is rejected instead of silently clamped.
- Cancellation is an `AtomicBool` shared with the engine read; setting it stops
  admission, wakes the consumer and returns `Cancelled`.
- Filters are literal UTF-16 substring matches over the primary column text, and
  `class` is an exact descriptor restricting methods/fields. `class_descriptor`
  converts a dotted name to a descriptor without rewriting `$`.

Local failures keep their kind: an engine read error stays `Engine`, an
application input/path error stays `Input`, and each maps to its own failure kind.
The DEX listing path never builds a `ProgramIndex` for analysis, never decompiles
and never caches between requests.

## Evidence

`cargo test -p diana --test product_contracts h03_` runs five public cases on the
self-made fixtures `annotations/field.dex`, `annotations/text.dex` and
`dynamic/poly.dex`:

- Defined members and strings: the two class definitions, exactly four defined
  methods with their complete prototypes (no external `method_ids` reference such
  as `Object.<init>` appears), the single instance field `n:I`, and string rows
  covering every `string_ids` index exactly once, including the
  `[0,55296,65,56320,92,117,48,48,48,97]` literal.
- Filters: a method-name substring returns one row with the same original identity
  as the unfiltered enumeration; an exact class descriptor restricts fields; a
  supplementary-plane UTF-16 literal matches as units, not bytes.
- Entries: a ZIP with two physical `classes.dex` records lists both with their own
  indices in directory order; a bare DEX is `NotArchive`.
- Multi-image completeness: an archive with `classes.dex` and `classes2.dex`
  produces the same sorted identity multiset with one and four workers, seven
  methods in total, and image labels stay in directory order.
- Bounds and cancellation: `max_rows = 1` fails with `Rows` and publishes at most
  the admitted batch; a pre-set cancel flag returns `Cancelled`; batched delivery
  produces no duplicated row.

No test asserts output order, table headers, CLI flags or private call counts.

## Gates and remaining work

- `cargo fmt --all -- --check`, strict workspace Clippy and the locked release
  build pass.
- `cargo nextest run --workspace --all-features --locked` passes 206 tests with
  zero skips, including every engine regression after the read-only additions.
- `xref` (H03b) and `manifest` plus CLI columns (H03c) remain. H03b reuses the
  already public `MethodBody::Code(RawMethodView)` and its `visit_layouts`
  reference slots.
