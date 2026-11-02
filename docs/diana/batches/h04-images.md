# H04 root images and definition-identity locators

Baseline: a4ca3f5. Contracts: DESIGN §3.1, §5.1, §6.3; TASKS H04. `ALGORITHMS.md`
is unchanged.

## Products

`analysis/selector.rs` parses the canonical locators of DESIGN §6.3 and the short
selectors:

- `e<entry>:l<logical>:c<class-def>::<descriptor>`
- `…:m<method-id>::<owner>-><name><proto>` and `…:f<field-id>::<owner>-><name>:<type>`
- `e<entry>:l<logical>:s<string-id>`
- `Lowner;`, a dotted class name, and `owner->name(proto)` / `owner->name:type`

The parsed locator keeps the original index and the expected descriptor/name/
prototype, so resolution can check both directions. Illegal syntax, a stale index,
a missing definition and an ambiguous short name stay distinguishable and carry the
DESIGN exit codes 2, 1, 3 and 4.

`analysis/images.rs` turns the leased roots into engine input: one `ImageRef` per
logical DEX with the physical-entry index, the logical ordinal, the container header
offset and the borrowed bytes. A bare DEX is `e0:l0`. A 041 container contributes
one image per logical header without copying the container. Resolution opens the
selected logical member, then verifies the descriptor at `class_def`, the name and
prototype at `method_id` and the name and type at `field_id`; an index that names a
different object is `InvalidIndex` rather than a silently wrong result.

`ReferenceFacts` and the public `DexString`/`Prototype` queries provide the
comparison, so the adapter decodes nothing twice. A string locator is validated
against the selected member's `string_ids`.

## Interface addition

The application needs the logical member offsets of a 041 container, and the
existing public surface exposed none. With operator approval,
`DexContainer::logical_headers` was added: it walks the already validated header
chain, pushes the original offsets under the request ledger and returns them in
physical order. It constructs no analysis product and copies no member bytes;
`apps/diana`'s image adapter is its only consumer.

## Evidence

`cargo test -p diana --test product_contracts h04_` runs five public cases on the
self-made fixtures `dynamic/poly.dex`, `annotations/field.dex` and the certified
`D065` container prepared by the E2E tooling:

- A canonical class locator resolves and a stale descriptor at a valid index is
  `InvalidIndex`; an out-of-range index, a foreign entry and a foreign logical
  ordinal are distinguished.
- A dotted short name resolves to the same definition; two physical entries that
  define the same class make the short name `Ambiguous` with exit code 4, while the
  canonical locator still selects one.
- Method and field locators check the name, prototype and field type; a wrong
  prototype or type is `InvalidIndex`. A string locator validates the id and an
  out-of-range one fails.
- Syntax failures (missing prefix, missing prototype, unknown kind) report exit 2.
- The 041 container yields two logical members at headers `0` and `0x78` of one
  physical entry; `l0` and `l1` resolve to their own images, the same class/method
  index in both is a different object, and `l2` is `NotFound`.

Gates: `cargo fmt --all -- --check`, strict workspace Clippy and the locked release
build pass; `cargo nextest run --workspace --all-features --locked` passes 222 tests
with zero skips.

## Remaining work

The engine-backed decompilation loop for a selected locator (`get-class`/
`get-method`) belongs to H05; this card stops at the borrowed image list, the
validated selection and the identity mapping back to the original entry/logical
member.
