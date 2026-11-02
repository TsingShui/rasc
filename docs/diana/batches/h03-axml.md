# H03c AXML manifest reader

Baseline: 3cc01da. Contracts: DESIGN §6.7 (`manifest`), §7 (application limits) and
TASKS H03. ALGORITHMS is unchanged; the reader implements the checked contract
itself instead of reusing the archived reader under `ref-fork/`.

## Products

`analysis/axml.rs` decodes one binary `AndroidManifest.xml`: the file chunk, the
string pool (UTF-16 and UTF-8 variants), the optional resource map, namespace
records and start/end element records with their typed attributes. Every chunk
size, header size and string offset is bounded by the remaining bytes; nesting is
bounded by the actual input; a configured node, string-unit or output bound is
enforced and never truncates into a success.

Decisions that keep the output faithful:

- Attribute names come from the original string pool, not from a hand-written
  resource-id table. A guessed table mislabels real attributes, so the resource
  map only undergoes shape validation.
- Attribute prefixes come from the namespace records, and the root element
  declares every active namespace, so the text is parseable XML with the original
  `android:`-style names.
- Typed values cover null, reference, attribute, string, float, dimension,
  fraction, decimal/hex integer, boolean, colour-int and the dynamic reference
  forms. A dimension or fraction is converted with the AOSP complex-mantissa
  radix table `{1, 1<<7, 1<<15, 1<<23}` and its unit, so a layout bound decodes
  as `1dip` rather than raw bits. Any other type is `UnsupportedValue` with the
  attribute index instead of a fabricated value.
- A repeated qualified attribute name inside one element is rejected, because the
  result would not be valid XML.
- A non-AXML container, an unknown record kind, a truncated chunk, an
  out-of-pool string index and an unbalanced element sequence keep their own
  classified reasons.

`analysis/manifest.rs` selects the **last physical** `AndroidManifest.xml` entry in
directory order, reads it through the shared archive reader (stored or deflate) and
decodes it. A bare DEX is `NotArchive`, a missing entry is `Missing`, an entry read
failure keeps its archive reason and a decode failure stays `Decode`.

## Evidence

Self-made fixtures: the AXML builder in `axml.rs` unit tests and
`support::axml_manifest` for the ZIP-level case; both are test fixtures, not
product output.

- `h03_axml_decodes_elements_attributes_and_android_ids` checks the decoded text
  including the declared namespace and `android:versionCode`.
- `h03_axml_rejects_damaged_or_unsupported_documents` covers a truncated
  container, a non-AXML signature, an unknown record kind and an out-of-pool
  string index.
- `h03_axml_node_and_output_bounds_are_explicit` checks the three explicit bounds.
- `h03_manifest_uses_the_last_physical_entry_and_keeps_classified_failures` covers
  duplicate manifest entries (the later one wins), a missing entry, a bare DEX, a
  damaged document and a deflated manifest entry.

Manual validation on five authorized local APKs (not committed, no fixtures and
no identifiers recorded): sizes from 152 MB to 360 MB, ZIP entry counts from
about 9k to 20k, manifests from 89 KB to 823 KB of XML output. Every decode
completed with exit 0 in 22–110 ms including the input lease, and one sample was a
truncated archive whose manifest still decoded.

Two comparisons ran. The first used a Python chunk decoder written for this check;
the second used the third-party `androguard` 4.1.4 `AXMLPrinter`, installed in a
temporary virtual environment outside the repository, so the result does not
depend on a reader authored here. Element paths, attribute names and semantic
values were compared after normalizing hex/reference spelling, numeric and
dimension formatting:

| Comparison | APK samples | elements | attribute names | semantic values |
|---|---|---:|---|---|
| self-written chunk decoder | 5 | identical | identical | all equal |
| `androguard` 4.1.4 | 5 | identical | identical | all equal |

`androguard` prints dimensions as `1.000000dip` where Diana prints `1dip`, and
spells references as `@7F120167`; those are presentation differences, not value
differences. After normalizing them no semantic difference remained.

Differences found and fixed during this validation:

1. `attributeStart` is relative to the attribute extension after the 16-byte node
   header, not to the chunk start.
2. Attribute naming through a hand-written resource-id table mislabelled real
   attributes; names now come from the original string pool.
3. UTF-8 string pools slice by the byte-length prefix, not the UTF-16 length; the
   earlier code corrupted non-ASCII names. A UTF-8, two-namespace, non-ASCII
   fixture now covers it.
4. The dimension radix multipliers were wrong (`1<<14` instead of `1<<15` for the
   third radix), so `0x00800021` decoded as `2dip` instead of `1dip`.
5. A repeated qualified attribute name (possible in damaged input) would have
   produced invalid XML; it is now `DuplicateAttribute`.

The first sample also produced 1484 elements and 3187 attributes, exactly the
binary counts of that manifest, and its text parses as well-formed XML with the
`android` namespace declared.

A bounded mutation campaign then exercised structural robustness. Three real
manifests were mutated 2000 times each with a fixed seed: a random offset and a
1–4 byte window were XOR-ed before decoding. Result: 6000 runs, **0 panics, 0
slow cases** (worst 6 ms, most 1–3 ms), every outcome either decoded or returned a
classified `ManifestError`. A committed regression repeats the same idea on the
self-made UTF-16 and styled-pool fixtures (512 mutations each) and asserts that a
mutation never panics and that at least one is rejected. Mutation of payload
strings legitimately yields a different but valid document, so "accepted" is not
an error.

Known limitations, recorded rather than hidden: text/CDATA and DTD records are
rejected as `UnsupportedChunk`; string-pool style spans are bounds-checked but not
interpreted, so styled text is not rendered; the resource map is validated but not
used for naming. Attribute values that are dimensions or fractions are formatted
from the AOSP complex encoding, and a repeated qualified attribute name inside one
element is rejected.

## Gates

`cargo fmt --all -- --check`, strict workspace Clippy and the locked release build
pass; `cargo nextest run --workspace --all-features --locked` passes 213 tests with
zero skips.

## Remaining work

The CLI layer for these commands (columns, JSON, exit codes, `-o` publication) and
the release measurement of the unordered query path are still outstanding, so H03
is not yet complete.
