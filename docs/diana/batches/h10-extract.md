# H10 extract: original payload, decoded XML and safe publication

Baseline: 63446e6. Contracts: DESIGN §6.7 (`extract`), §7; TASKS H10.
`ALGORITHMS.md` is unchanged.

## Products

`diana extract <input> <entry> [--entry-index N] -o <file> [--raw] [--force]
[--strict]` operates on physical archive records only.

Selection is exact: a name with several physical records without `--entry-index`
is `Ambiguous` with the candidate indices and exit 4; a given index must name the
same entry, otherwise `IndexMismatch`; a missing name is exit 3. The payload is read
through the shared archive reader, so stored and deflate entries both work and the
size and CRC are validated over the **original decompressed bytes** before any
decode. Encryption and other compression methods keep their archive reasons.

A payload whose leading chunk kind is `RES_XML_TYPE` is decoded to text XML by
default with the same bounded reader as `manifest`; `--raw` keeps the original
payload bytes, and any other entry (DEX, resources, assets) is always written byte
for byte. A decoded partial document carries the manifest warning rules: in-place
comments, the warning list, and `--strict` making the run exit 1 after publication.

Publication is atomic: a private `.diana-extract-<pid>-<n>.tmp` file in the target
directory receives the payload and is renamed into place. An existing ordinary file
needs `--force`; a directory, a symbolic link, the input file itself or a hardlink
alias are `UnsafeTarget`. A failed write or rename removes the temporary file.

## Evidence

`cargo test -p diana --test product_contracts h10_` runs three public cases:

- Duplicate names are ambiguous with exit 4, an index selects the intended record,
  a mismatched index and a missing name are classified.
- A binary manifest entry decodes by default, `--raw` returns the exact original
  bytes, and a DEX entry is returned unchanged.
- Publishing refuses to overwrite without `--force`, replaces an ordinary file with
  it, rejects a directory, the input itself and a symbolic link, and leaves no
  temporary file behind.

Manual CLI checks on a self-made archive: duplicate name → exit 4 with indices;
`--entry-index` publishes the chosen record; a mismatched index exits 1; a
half-truncated binary manifest publishes balanced partial XML with two warnings; a
directory, the input file and a symbolic link are refused with exit 1; an existing
target without `--force` exits 1 and keeps the old bytes; `classes.dex` keeps the
`dex\n035` prefix; no `.diana-extract-*` file remains.

Gates: formatting, strict workspace Clippy and the locked release build pass;
`cargo nextest run --workspace --all-features --locked` passes 234 tests with zero
skips.

## Limitation recorded

`extract` decodes only binary XML. Style spans inside a decoded text string stay
unrendered, and a non-manifest AXML uses the same conservative warning rules.
