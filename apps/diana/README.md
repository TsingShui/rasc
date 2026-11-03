# Diana CLI

Diana is the command-line front end of the native Diana decompiler. The binary is
`diana` (`diana.exe` on Windows); it never delegates to rasc, dexdec or an external
runtime. Old command names and flags (`getclass`, `findrefs`, `skill`,
`--require-java`, `--class-id`, `--members`, `--offset`, `--limit`) do not exist and
are rejected as unknown arguments.

The engine lives in [diana-decomp](../../crates/diana-decomp/README.md) and borrows
DEX bytes supplied by this package. See
[specification](../../docs/diana/spec/00-overview.md) for the interface. The `0.0.0` version is an
unpublished scaffold marker.

## Commands

```text
get-class   Decompile one class
entries     List physical archive entries
extract     Extract one physical entry
strings     List original string values
xref        Find original reference sites
manifest    Decode the binary AndroidManifest.xml
classes     List class definitions
get-method  Decompile one method
fields      List defined fields
methods     List defined methods
help        Print help for a command
```

Every command accepts `--format text|json`, `-o <file>` and the explicit limit
options below. Output order is completion order, not a stable sequence. There is no
pagination, cursor or preview: a successful query returns every match, and an empty
match is a normal empty result.

| Command | Columns |
|---|---|
| `entries` | `name, compressed_size, uncompressed_size, compression` |
| `classes` | `at` |
| `fields` | `at` |
| `methods` | `at` |
| `strings` | `at, value, entry` |
| `xref` | `entry, context, match, use` |
| `manifest` | text XML, or JSON `{schema_version, command, entry, status, xml, warnings}` |
| `get-class` | text display, or JSON schema 1 |
| `get-method` | text display, or JSON schema 1 |
| `extract` | one file written to `-o`; stdout stays empty |

`classes` can be restricted with `--filter`, `--extends`, `--implements`,
`--subinterface` and `--kind`; the three hierarchy options follow only the
definition's own direct `super_class` or interface edges. `fields` and `methods`
accept `--class` (descriptor or dotted name) and `--filter`. `--filter` is a
case-insensitive literal over the semantic name/descriptor, not a regular
expression. `--jobs` bounds how many independent DEX images are scanned at once and
must be positive.

## Original identity: `at`

```text
class-at  = e<entry>:l<logical>:c<class-def>::<descriptor>
method-at = e<entry>:l<logical>:c<class-def>:m<method-id>::<owner>-><name><proto>
field-at  = e<entry>:l<logical>:c<class-def>:f<field-id>::<owner>-><name>:<type>
string-at = e<entry>:l<logical>:s<string-id>
```

`e` is the physical ZIP central-directory record index, `l` the logical DEX ordinal
inside that entry (`0` for an ordinary DEX, several for a 041 container), and `c`,
`m`, `f`, `s` are the original table indices. The suffix after `::` is the original
descriptor or the complete reference, so a stale index that names another object is
rejected instead of resolved. A bare DEX is `e0:l0`. An `at` printed by one command
can be passed back to another verbatim; shell-quote it when it contains `$` or a
space.

## Selectors

`get-class` and `--class` accept a class-at, `Lowner;` or a dotted class name.
`get-method` accepts a method-at or the complete `Lowner;->name(args)return`; a bare
method name and a separate proto option do not exist. `xref` takes the kind
`string`, `type`, `method` or `field` plus the exact target: a complete UTF-16
string, a type descriptor or dotted name, and a complete method/field reference.
A short class name with several definitions is reported as ambiguous (exit 4)
instead of picking one.

## Text and JSON

Text is UTF-8 TSV with the exact column header, or XML for `manifest`/`extract`.
Fields escape `\\`, tab, CR, LF and NUL, and an isolated surrogate is printed as
`\uXXXX`; no value is replaced or split across lines. A table in JSON is
`{schema_version, command, items:[{column: value}]}`; `get-class`/`get-method` use
the decompilation report schema 1, and `manifest` uses its own report with a
`status` of `complete` or `partial`.

`manifest` and a decoded `extract` keep the document usable: a recovered gap is
reported as an in-place `<!-- diana: ... -->` comment, in the warning list of the
JSON result and on stderr. `--strict` makes such a partial document exit non-zero
after it is published.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | A result was delivered, including a partial document |
| 1 | Input, decode, budget, cancellation, internal or I/O failure |
| 2 | Unknown command/flag or an invalid selector |
| 3 | The requested single object or entry does not exist |
| 4 | The selector matches several objects |

## Extract

`extract <input> <entry> -o <file>` writes exactly one physical record. A duplicate
name requires `--entry-index`; the index and the name must agree. The payload is
validated (size and CRC) over the original decompressed bytes; a binary XML entry is
decoded to text by default and `--raw` keeps the original payload. Publication is
atomic from a private temporary file in the target directory, an existing file needs
`--force`, and a directory, a symbolic link, the input itself or a hardlink alias are
refused. Nothing is printed to stdout.

## Limits and cancellation

Every limit is unset by default; only an explicitly configured bound applies and an
overrun is an error, never a truncated success. `--max-input-bytes` and
`--max-inflated-bytes` bound the input side; `--max-request-work-steps`,
`--max-request-allocation-bytes`, `--max-work-steps`, `--max-method-output-bytes` and
`--max-report-output-bytes` bound the engine and the delivered payload. Cancellation
is cooperative through the request ledger; a process killed by a signal is not a
catchable cancellation guarantee.

## Agent usage

This section is ordinary documentation, not an installed agent skill, and the binary
provides no skill-install command.

1. Run `diana entries app.apk` to see the physical entries and pick the DEX or XML
   you need. `classes`, `fields`, `methods` and `strings` list original symbols
   without decompiling.
2. Read an `at` and pass it back: `diana get-class app.apk 'e0:l0:c7::Lcom/example/App;'`
   or `diana get-method app.apk 'e0:l0:c7:m23::Lcom/example/App;->check(I)Z'`.
   Mixed output is the normal display: every member is present and each failed one
   carries its reason and original facts. Do not treat Mixed as a valid Java
   compilation unit.
3. Use `--format json` for machine reading of the same result. Warning lists,
   statuses and spans are part of that report.
4. `xref` finds definition-independent usages: `diana xref app.apk method
   'Lcom/example/Dep;->f()V'` with `--class` to restrict the referencing class.
5. `diana extract app.apk 'AndroidManifest.xml' -o manifest.xml` decodes the binary
   manifest; `--raw` keeps the original AXML bytes. Extraction writes only the
   requested file.
6. Convert DEX to a JVM-executable form with an external tool such as the Android
   `d8`, then reuse that existing conversion for later checks. Diana itself neither
   performs that conversion nor runs recovered code.

Do not delete or move a user's installed rasc skill when adopting Diana; this
document is additive.
