# H09 defined symbols and xref sites in the CLI

Baseline: 2320e05. Contracts: DESIGN §6.4 (`fields`/`methods`), §6.6 (`xref`
four columns); TASKS H09. `ALGORITHMS.md` is unchanged.

## Products

`diana xref <input> <kind> <target> [--class] [--jobs] [--format] [-o]` scans the
original code of every root image and prints `entry, context, match, use`:

- `entry` is the physical entry name of the referencing side.
- `context` is the referencing method's canonical `at` plus `@cu:0x<offset>`.
- `match` is the value actually referenced (UTF-16 string, type descriptor,
  complete method reference or field reference with its type).
- `use` is the original DEX mnemonic.

Kinds are `string`, `type`, `method` and `field`; a method or field target requires
the complete reference (owner, name and prototype or type), a type target accepts a
descriptor or dotted name, and an unknown kind is a usage error with exit 2. The
referencing definition is restricted by exact `--class`. Rows arrive in completion
order with no sorting, deduplication or pagination, and empty results are normal.

`fields` and `methods` (already wired) list only `class_data` definitions, include
abstract/native/constructor/clinit declarations, and print the canonical `at`.

`XrefRow` now carries the owner descriptor, so the context locator is built from the
original identity rather than from a display name.

## Evidence

`cargo test -p diana --test product_contracts h09_` runs three public cases:

- Two physical entries that contain the same fixture produce four sites per image
  (eight rows); equal class/method indices in the two entries stay separate and
  every `(image, method, code-unit)` identity is unique.
- A `string` target matches only the complete UTF-16 literal used by a
  `const-string` site; a shorter prefix of the same units matches nothing. The
  earlier text fixture keeps that literal in an annotation only, which is why a
  code-site scan must not report it.
- An interface method is listed as a real definition with its abstract flag, an
  unused string-pool value remains a string row, and neither is inferred as a
  reference.

Manual CLI checks: `xref type fixtures.Repo` lists `instance-of`, `check-cast` and
`const-class` sites; `xref method …->save()V` lists four `invoke-interface` sites
across two members; `--format json` returns the same rows under `items`; an absent
target prints only the header; an unknown kind exits 2.

Gates: formatting, strict workspace Clippy and the locked release build pass;
`cargo nextest run --workspace --all-features --locked` passes 231 tests with zero
skips.

## Limitation recorded

`xref` (like the query commands) scans the root images that the lease reports, which
currently means the first logical DEX of a 041 physical entry. Extending the scan to
every logical member reuses `ImageSet` and is a later change; the canonical `at`
already carries the logical ordinal for such a future scan.
