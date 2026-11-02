# H03c CLI wiring: entries and manifest

Baseline: 51888fd. Contracts: DESIGN §6.4 (`entries`), §6.7 (`manifest`), §6.8
(exit codes) and TASKS H03. `ALGORITHMS.md` is unchanged.

## What the binary now does

`apps/diana/src/cli/` parses the command line with the existing `clap` dependency
and calls the analysis modules; it adds no second decoding path. `main` returns
the classified exit code instead of the scaffold message.

`entries <input> [--format text|json] [-o file]` lists physical archive records in
directory order with the exact columns `name, compressed_size, uncompressed_size,
compression`. The value of `compression` is the numeric ZIP method; no labels. A
bare DEX is an input error.

`manifest <input> [--format text|json] [-o file] [--strict]` selects the last
physical `AndroidManifest.xml` and decodes it. Text output is the XML itself, with
in-place `<!-- diana: ... -->` comments where content was affected. JSON output is
`schema_version`, `command`, `entry`, `status` (`complete`/`partial`), `xml` and
`warnings[{code, offset, detail}]`. Warnings also go to stderr with their byte
offset.

Exit codes: 0 for a delivered result, including a partial manifest; 1 for input,
decode or publication failures; 2 for argument errors (clap); 3 when the archive
has no manifest entry. `--strict` turns a partial manifest into exit 1 after the
payload is published.

Publication: with `-o`, the payload is written to a fresh
`.diana-<pid>-<n>.tmp` file in the target directory and renamed into place, so a
failed write never leaves a half result; a failure removes the temporary file.
Without `-o`, the payload goes to stdout and a closed pipe ends the command
normally instead of reporting an error.

## Verification

No automated CLI test was added: repository rules forbid black-box CLI tests, and
the analysis behaviour is covered by the public library cases. The wiring was
exercised manually on authorized local APKs:

- `entries` on a 152 MB APK prints the header and physical records, and the JSON
  form carries the same rows; piping into `head` ends with exit 0.
- `manifest` on that APK prints complete XML (exit 0, no warnings); `-o` writes the
  file and it parses with an independent XML parser.
- A deliberately truncated copy of the same manifest produced balanced partial XML
  with three warnings on stderr and in the JSON array, exit 0, and exit 1 with
  `--strict`.
- A bare DEX for `manifest` reports the input error with exit 1; an archive
  without the entry reports exit 3.
- No `.diana-*.tmp` file remained after any run.

## Query commands

`classes`, `fields`, `methods` and `strings` reuse `analysis::query` and print the
canonical `at` locator of DESIGN §6.3: `e<entry>:l0:c<class_def>::<descriptor>`
for a class, with `:m<method>` or `:f<field>` and the complete `owner->name(proto)`
or `owner->name:type` suffix, and `e<entry>:l0:s<string>` for a string. A bare DEX
is `e0`. `fields`/`methods` accept `--class` as a descriptor or dotted name,
`--filter` is a case-insensitive literal over the semantic part of the row,
`strings` prints `at, value, entry` where `entry` is the physical entry label, and
`classes` accepts `--extends`, `--implements`, `--subinterface` and `--kind`.
`--jobs` defaults to `min(8, available_parallelism)`, zero is rejected with exit 2,
and the four commands share the bounded completion-order runner, so a slow image
does not block other images' rows.

TSV fields are escaped (`\\`, `\t`, `\r`, `\n`, `\0`, lone surrogate as
`\uXXXX`); JSON is `{schema_version, command, items:[{column: value}]}`.

Manual run on a 152 MB, ten-DEX APK with eight workers: 90646 class definitions,
689338 defined methods and 918458 string values were listed, and the JSON `items`
length equalled the TSV row count for the same selection. `--filter ONCREATE`
matched `onCreate` records, `--class` with an absent class returned the header
only, `-o` wrote the same payload with no leftover temporary file, and piping into
`head` ended with exit 0.

## Remaining work

`xref` stays with H09 (four columns), `extract` with H10, and `manifest` is
complete for this card. `get-class`/`get-method` belong to H05.
