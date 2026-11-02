# H03b xref sites and hierarchy filters

Baseline: a504309. Contracts: DESIGN §6.4 (`classes` relation options) and §6.6
(four-column xref), both revised by this batch with operator approval. TASKS H03
owns progress. ALGORITHMS is unchanged.

## Contract changes

`classes` gained four contradictory-free filters over the same `at`-only list:
`--extends`, `--implements`, `--subinterface` and `--kind`. Each reads only the
definition's own `super_class_idx` or `interface_list`; no closure is computed, so
a class that inherits a relation through another definition is not reported.

`xref` now has four columns. `use` carries the original DEX mnemonic of the
referencing instruction, taken from the checked opcode table, so a caller can tell
`check-cast` from `instance-of` from `const-class` without decoding the source
line. Sites remain instruction reference slots only: a declaration type, a `catch`
type or a hierarchy edge is never an xref row.

The engine additions remain read-only P1 facts approved for this work:
`ProgramIndex::reference_facts` resolves a slot to `String`/`Type`/`Method`/
`Field`/`Proto` facts or `Unsupported` (call-site and method-handle slots are not
expanded), and `ClassView::hierarchy` exposes the declared superclass and interface
descriptors. Neither starts analysis or constructs a stage product.

## Products

`analysis/xref.rs` scans each root image with the existing public
`MethodBody::Code(RawMethodView)` and `visit_layouts`: it collects the original
`(code unit, mnemonic, reference)` triples under the method/fallback ledger, then
resolves and compares them after the layout iteration, so no reference analysis
happens inside the read callback. Producers stream through the shared bounded
runner (`analysis/runner.rs`), which was extracted from the query core: one worker
per admitted image, channel capacity equal to the worker count, completion-order
delivery, explicit row bound, cancellation and no reorder buffer. `query.rs` now
uses the same runner, so both commands share one scheduling implementation.

Matching is exact: string targets compare complete UTF-16 units, type targets an
original descriptor, method targets owner plus name plus complete prototype, field
targets owner plus name plus field type. `--class` restricts the referencing
definition by exact descriptor. A row keeps the referencing class/method identity,
its name and prototype, the original code unit and the matched value.

## Evidence

`cargo test -p diana --test product_contracts h03_` now runs eight public cases.
The new fixture is self-made Smali (`tests/fixtures/hierarchy/src/*.smali`,
assembled by locked smali 2.5.2 at API level 26 into `hierarchy.dex`) with an
interface, a sub-interface, an implementing class, a subclass and a user method.

- xref `type fixtures.Repo` reports exactly three sites in `Use.sites` with
  `use` values `check-cast`, `const-class` and `instance-of`, distinct code units
  and the matched descriptor.
- xref `method fixtures.Repo;->save()V` reports the single `invoke-interface`
  site with the complete owner/name/prototype match.
- xref on an instance field reports the `iget` site in the defining class's
  reader method.
- `--class` keeps only the named referencing definition; a foreign class and an
  absent target both return an empty result.
- Hierarchy filters: `--extends MemoryRepo` → `DiskRepo`; `--implements Store` →
  `MemoryRepo`; `--subinterface Repo` → `Store`; `--kind interface` → the two
  interfaces; `--implements Repo` is empty because nobody lists it directly,
  which demonstrates the no-closure rule.

Gates: `cargo fmt --all -- --check`, strict workspace Clippy and the locked release
build pass; `cargo nextest run --workspace --all-features --locked` passes 209
tests with zero skips, including every engine regression after the two read-only
additions.

## Remaining work

H03c still needs the AXML manifest reader, the CLI layer with these columns and
exit codes, and the release measurement of the unordered query path. The
`entries`/`classes`/... text and JSON writers are not implemented yet, so no
column formatting is claimed beyond this module contract.
