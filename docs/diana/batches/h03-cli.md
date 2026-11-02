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

## Remaining work

`classes`, `fields`, `methods`, `strings` and `xref` still need their CLI
presentation (`at` columns, JSON rows, `--filter`, `--class`, the hierarchy
options and the four xref columns). They reuse the already-tested query and xref
modules, so that slice is presentation plus exit-code mapping.
