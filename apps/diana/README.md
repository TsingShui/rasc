# Diana CLI

Diana is the native command-line front end of the
[Diana engine](../../crates/diana-decomp/README.md). `get-class` and `get-method`
deliver the existing P0–P9 Engine result, without a second analysis or renderer.
The binary is `diana` (`diana.exe` on Windows). The `0.0.0` version is an
unpublished development marker.

The [approved architecture](../../docs/diana/PIPELINE_DESIGN.md) owns product
boundaries. This package owns input snapshots, APK/ZIP access, selection,
command routing and file publication. Decompilation uses only the analyzed input;
external classpaths, declarations and runtimes are not required.

## Commands

```text
get-class   Decompile one class
get-method  Decompile one method
entries     List physical archive entries
extract     Extract one physical entry
strings     List original string values
xref        Find original reference sites
manifest    Decode the binary AndroidManifest.xml
classes     List class definitions
fields      List defined fields
methods     List defined methods
help        Print help for a command
```

Commands accept `--format text|json` and `-o <file>`. Extraction always writes
the requested payload to `-o`, with no stdout result. Queries return every match,
without pagination or previews. An empty match is a normal empty result.
Independent query results arrive in completion order, not a guaranteed sequence.

| Command | Presentation |
|---|---|
| `entries` | `name, compressed_size, uncompressed_size, compression` |
| `classes`, `fields`, `methods` | `at` |
| `strings` | `at, value, entry` |
| `xref` | `entry, context, match, use` |
| `manifest` | XML, or JSON with `entry, status, xml, warnings` |
| `get-class`, `get-method` | P9 Java-like text, or JSON schema 3 |
| `extract` | One output file |

`classes` supports `--filter`, `--extends`, `--implements`, `--subinterface`
and `--kind`. Hierarchy filters inspect direct original superclass/interface
edges. `fields` and `methods` accept `--class` as a descriptor or dotted name,
and `--filter` as a case-insensitive literal name substring.

Query `--jobs` bounds concurrent physical DEX roots. Logical headers within a
root retain their own identities. `get-class --jobs` instead sets Engine's
method worker count; its default is one. Explicit job counts must be positive.

## Original identity and selection

```text
class-at  = e<entry>:l<logical>:c<class-def>::<descriptor>
method-at = e<entry>:l<logical>:c<class-def>:m<method-id>::<owner>-><name><proto>
field-at  = e<entry>:l<logical>:c<class-def>:f<field-id>::<owner>-><name>:<type>
string-at = e<entry>:l<logical>:s<string-id>
```

`e` is the physical ZIP central-directory record index. A bare DEX uses `e0`.
`l` is the original header-chain ordinal within that physical container: zero
for an ordinary DEX, potentially several ordinals for DEX 041. `c`, `m`, `f`
and `s` are original table indices. Engine's request-local `DexId` is a separate
coordinate; it is not a physical entry or logical-header ordinal. A canonical
locator opens only its physical root. Unrelated DEX payloads are not decompressed
or required to be readable. An unqualified class name searches all roots to
establish a unique match.

`get-class` accepts a class-at, descriptor or dotted class name. `get-method`
accepts a method-at or the complete `Lowner;->name(args)return` reference.
Use locators printed by `classes` and `methods`, shell-quoting dollar signs,
spaces and arrows. An ambiguous class or method reference is not guessed.

Class locators must agree with their checked descriptor. Method selection
consumes the same Engine's cached P0 directory and checks membership, original
MethodId, name and complete prototype. A reference in `method_ids` alone is not
a definition in the selected class. An unavailable declaration or unfinished
directory does not establish that a method is absent.

Class retrieval analyzes all known original members. Single-method retrieval
calls `Engine::analyze_method` with the selected ID and uses its standalone
P8/P9 display; it does not analyze siblings or slice text out of a class result.

`xref` takes `string`, `type`, `method` or `field` and the corresponding
exact value/reference. Its `--class` restricts the referencing class by
descriptor or dotted name. It searches operand sites without requiring the target
to have a definition in the input. A reader failure is reported, not silently
counted as an empty method.

## Decompilation text and JSON

Text is exactly the existing P9 UTF-8 text. The notation is
`diana-java-like-v1`, described in [P9 rendering](../../docs/diana/spec/09-rendering.md).
Available expressions, original members, explicit operations and local recovery
gaps remain visible. Java legality or compilability is not an output gate.
Diagnostics are not hidden behind a verbosity flag.

Schema 3 has these top-level fields:

- `schema_version: 3` and `command: "get-class" | "get-method"`.
- `selection`: physical `entry` (`null` for bare DEX), `logical` header
  ordinal, container-relative byte `header_offset`, Engine `class` key,
  optional `method` ID, `class_at` and optional `method_at`.
- `result`: the native P9 `{kind, product}` publication. A `ready` product
  contains `class, notation, source, text, spans, issues`.

`product.text` is byte-for-byte the text presentation. Spans are half-open UTF-8
byte ranges within that text, not offsets into JSON. The frozen `source` keeps
the same P8/P7 products and original facts, including original UTF-16 units,
stopped members and retained bytes. Inspection-formatted diagnostic fields are
not an alternate executable representation. JSON serialization does not run
another renderer or choose a different recovery mode.

`ready` describes publication completion, not semantic verification. Local
member failures remain content of that publication. A shared rendering failure
retains P9's `stopped` product: JSON includes its source, error and text prefix;
text writes that same prefix. Both report the concrete stop on stderr and exit 1.
An unfinished prefix is never reported as a successful complete result.

The old schema 2, Java/Mixed status projections, whole-class method-text slicing,
`--verbose` diagnostic gate and declaration-catalog adapter are removed.

## Query and manifest output

Query text is UTF-8 TSV with the column header. Display fields escape backslash,
tab, CR, LF and NUL; isolated surrogates remain escaped instead of being replaced.
Query JSON uses `{schema_version: 1, command, items}`. `entries` reads the ZIP
central directory only; it does not open or decompress DEX payloads.

`manifest` uses XML or JSON with `schema_version, command, entry, status, xml,
warnings`. Recoverable AXML gaps remain XML comments, JSON warnings and stderr
diagnostics. `--strict` reports a partial document as failure after publication.

## File publication and exit codes

Result serialization writes directly to stdout or a private temporary file beside
the requested target. A file is renamed into place only after serialization and
flush complete. Writer/serializer/flush/rename failures do not replace the target.
A P9 rendering stop is a fully delivered failure report, not an interrupted file
write. Its text remains explicitly incomplete as described above.

A directory, symbolic-link target or input-file alias is refused. On Unix the
alias check also covers hard links. A closed stdout pipe is an output error,
not successful truncation. The CLI/host owns process termination; there are no
cooperative cancellation callbacks or numeric work/allocation/output quotas.

| Code | Meaning |
|---|---|
| 0 | A result was delivered, including local recovery gaps or a partial XML document |
| 1 | Input, unresolved selection, construction, rendering or output failure |
| 2 | Invalid command, flag or selector syntax |
| 3 | The requested single object or entry does not exist |
| 4 | The selector matches several definitions |

`extract <input> <entry> -o <file>` writes one physical record. Duplicate names
require `--entry-index`; name and index must agree. Size and CRC validation
applies to the original decompressed bytes. Binary XML is decoded by default;
`--raw` keeps the payload. An existing extraction target needs `--force`.
Extraction refuses directories, symbolic links and input aliases.

## Development

Build from the repository root:

```sh
cargo build --locked -p diana
cargo run --locked -p diana -- get-class classes.dex 'Lexample/Main;'
cargo run --locked -p diana -- get-method classes.dex 'Lexample/Main;->run()V' --format json
```

Use the engine's [`dbg_shell`](../../crates/diana-decomp/README.md) for P0–P9
inspection. The CLI does not expose separate phase constructors or a retry chain.
