# 10 — CLI and application surface

Code: `apps/diana/src/cli/`.

## 6. CLI contract

The eleven commands, fields, selectors and exit codes are maintained only in this section; a task card MUST NOT duplicate another copy of the specification.

#### 6.1 Top-level help and command table

The command section of the target help:

```text
Commands:
  get-class   Decompile one class
  entries     List APK/ZIP entries (columns: name, compressed_size, uncompressed_size, compression)
  extract     Extract original entry bytes to -o
  strings     List DEX string literals (columns: at, value, entry)
  xref        Search code references across every DEX (columns: entry, context, match)
  manifest    Extract decoded AndroidManifest.xml
  classes     List classes across every DEX (columns: at)
  get-method  Decompile one method
  fields      List fields across every DEX (columns: at)
  methods     List defined methods (columns: at)
  help        Print this message or the help of the given subcommand(s)
```

The table below is the command contract; usage shows the argument form and does not mean the implementation already exists.

| Command | Summary | Columns | Usage |
|---|---|---|---|
| get-class | Decompile one class | — | diana get-class <input> <class> |
| entries | List APK/ZIP entries | name, compressed_size, uncompressed_size, compression | diana entries <input> |
| extract | Extract original entry bytes to -o | — | diana extract <input> <entry-name> -o <file> |
| strings | List DEX string literals | at, value, entry | diana strings <input> |
| xref | Search code references across every DEX | entry, context, match | diana xref <input> <kind> <target> |
| manifest | Extract decoded AndroidManifest.xml | — | diana manifest <input> |
| classes | List classes across every DEX | at | diana classes <input> |
| get-method | Decompile one method | — | diana get-method <input> <method> |
| fields | List fields across every DEX | at | diana fields <input> |
| methods | List defined methods | at | diana methods <input> |
| help | Print command help | — | diana help [command] |

The core form is uniformly **`diana <command> <input> [selector] [options]`**. `help`, `--help` and `--version` do not require input. input is an APK, a ZIP containing a root DEX or a bare DEX; the narrower input ranges of the entry commands and manifest are described below. Diana MUST NOT invoke d8 automatically and MUST NOT handle ordinary JVM `.class` / JAR bytecode directly.

#### 6.2 Common output and options

- The default table is **UTF-8 TSV**, whose first line holds the specified column names exactly in the order of the table above; the CLI MUST NOT add a row number, an index table, color or a debug column. Record order is not guaranteed and records are emitted as they complete or arrive.
- `--format text|json`, default text; a table in JSON is the object `{ "items": [...] }` with fields equal to the column names. The JSON of decompilation/manifest is an explicit domain report, see §6.5/§6.7; it is not an MCP envelope, and no protocol tool is designed.
- `-o, --output <file>`: text/JSON commands may write to a file, and on success the file holds the same payload bytes as stdout for that run; `extract` is the exception, writing only binary to the file and no copy to stdout.
- `--filter <literal>`: applies only to `classes` / `fields` / `methods` / `strings`; a case-insensitive literal substring that matches the semantic name/descriptor inside the complete `at` expression (the opaque e/l/c/m/f/s index prefix is not matched), while for strings it matches value. It is not a regex and does not cut records.
- `--class <class>`: the defining-class selector of fields/methods; the referencing-class selector of xref. It MUST use the exact class selector of §6.3, MUST NOT default to fuzzy matching, and MUST NOT replace the xref target.
- `--jobs <N>`: the concurrency configuration for scanning independent DEX files, and N MUST be greater than 0; Diana does not promise stable output and does not require the engine to start a separate thread pool per method. Diana MUST NOT expose an ineffective option for a command without related parallel work.
- `--debug`: diagnostics/trace go to stderr and MUST NOT pollute TSV, JSON, Java, XML or extracted files. Diana MUST NOT ship an option whose switch changes but whose implementation never consumes it.
- **There is no `--offset` / `--limit` / cursor / preview / stable-output**. A successful query returns every match. Safety input/work/output budgets are retained: exceeding a budget is an error, and the CLI MUST NOT present returning only the first N items as success. Internal batching/backpressure is not pagination.

Any string field of TSV is escaped for display: `\\`, `\t`, `\r`, `\n`, `\0`; a lone UTF-16 surrogate uses `\uXXXX`, and Diana MUST NOT change it to a replacement character or let one record spread over multiple lines. The CLI MUST NOT rely on lossy splitting for a value that contains the delimiter. The JSON serializer likewise retains the necessary Unicode escapes; the concrete representation of control characters and lone surrogates MUST be accepted by F03/Q09. `at` itself can be passed back to the CLI verbatim and needs shell quoting when it contains a space or `$`.

A streaming text query may have written some records before a failure; the CLI MUST then exit non-zero and state the error clearly on stderr; such partial output is not a complete success. Diana MUST build a JSON report, a decompilation display (including Mixed), manifest and extract files as complete artifacts and commit them atomically, and MUST NOT leave a falsely complete file on failure; an intentionally marked Mixed is not a truncated JavaUnit. A large table in JSON is subject to the complete-result budget; the user may switch to streaming text with external shell filtering, and Diana MUST NOT solve this by adding pagination.

#### 6.3 `at`: an original-identity locator, not a result row number

A listing MUST provide an exact locator that can be passed back, and the locator MUST NOT depend on result order. **All indices are of the original input**, not sorted row numbers, synthetic LocalIds or Java names.

##### 3.1 Canonical format

```text
class-at  = e<physical-entry>:l<logical-member>:c<class-def>::<class-descriptor>
method-at = e<physical-entry>:l<logical-member>:c<class-def>:m<method-id>::<owner>-><name><proto>
field-at  = e<physical-entry>:l<logical-member>:c<class-def>:f<field-id>::<owner>-><name>:<type>
string-at = e<physical-entry>:l<logical-member>:s<string-id>
```

The numbers are non-negative decimal; `e` is the physical ZIP central-directory record index, `l` is the logical DEX ordinal inside that entry (0 for an ordinary DEX), `c` is the class_def index, and `m/f/s` is the index in the corresponding DEX's original id table. A bare DEX is treated as the single entry `e0:l0`. A canonical class descriptor is such as `Lcom/example/App;`; a method proto MUST contain the parameters and the return type, such as `(ILjava/lang/String;)Z`.
Examples (example indices, not a real corpus):

```text
e0:l0:c7::Lcom/example/App;
e0:l0:c7:m23::Lcom/example/App;->check(I)Z
e0:l0:c7:f9::Lcom/example/App;->count:I
e0:l0:s15
```

Diana MUST check the descriptor/name/proto in the canonical format and the indices in both directions: after the input is modified the original locator may become invalid and Diana MUST return an error, and it MUST NOT skip the name check when the index points at a different object. Diana MUST NOT merge equal names across different inputs/DEX files. "No duplicate and no omission" means that each original identity is recorded once, not that records are deduplicated by textual name.

##### 3.2 Readable short selectors

- get-class / `--class` accepts a class-at, `Lowner;` or a dotted `com.example.App`; Diana normalizes a dotted name only as an explicit class name and MUST NOT trigger a fuzzy search.
- get-method accepts a method-at or the complete `Lowner;->name(args)return`; the owner may be dotted as well, but Diana MUST NOT rewrite descriptors inside the proto from dots to slashes arbitrarily. **There is no bare method name and no separate `--method` / `--proto` combination**; one complete selector already expresses the identity.
- The xref field/method target accepts an original complete reference descriptor; it also accepts a canonical field-at/method-at, but then Diana MUST validate the locator and resolve it to a reference first, and references are then searched across every DEX. Diana MUST NOT let selecting a definition as the target restrict the referencing entry automatically.
- If a short name has multiple definitions, Diana MUST return `AMBIGUOUS_SELECTION` (exit4) with canonical at locator information and MUST NOT pick one arbitrarily from the first finishing worker. An exact at can distinguish duplicate class-defs with the same descriptor in the same DEX.
- A query over an empty set exits 0 normally; fetching or extracting a single object that is missing exits 3. An invalid at or proto syntax is a usage error, exit2; a stale range or a name-index inconsistency is an input/selector error, exit1, and Diana MUST NOT emit the wrong object.

#### 6.4 Listing commands

##### entries

`diana entries <apk-or-zip> [--format text|json] [-o file]`.

The columns are exactly `name, compressed_size, uncompressed_size, compression`; sizes are non-negative byte counts, and compression is the numeric ZIP method (for example 0 stored, 8 deflate), not a label that varies with the language. The entries command MUST NOT decompress all entries to list the directory and MUST NOT deduplicate or sort by name. Every physical record is listed, and an unknown compression method may be listed as metadata, which does not mean that extract supports it.

The specified columns contain no extra entry_index. Exact extraction of a duplicate name goes through `extract --entry-index`, and details and ambiguity diagnostics may carry the index; Diana MUST NOT add a fifth column to the entries default columns on its own. Diana MUST NOT treat a bare DEX as a ZIP directory; that is an explicit input type error.

##### classes / fields / methods

```sh
diana classes sample.apk --filter 'example'
diana classes sample.apk --implements 'com.example.Repo'
diana classes sample.apk --extends 'com.example.Base'
diana classes sample.apk --subinterface 'com.example.Repo'
diana fields sample.apk --class 'com.example.App'
diana methods sample.apk --class 'com.example.App' --filter 'check'
```

`classes` restricts its definition list with these options, which MAY be combined
as a conjunction:

- `--extends <class>`: the definition's own superclass is that class.
- `--implements <interface>`: the definition is a class whose own interface list
  contains that interface.
- `--subinterface <interface>`: the definition is an interface whose own
  interface list contains that interface.
- `--kind class|interface`: the definition is or is not an interface.

Each relation uses only the definition's own direct `super_class_idx` or
`interface_list` edge. Diana MUST NOT compute a transitive superclass,
implementation or sub-interface closure, and MUST NOT report a class that merely
inherits the relation through another definition. Missing relations are an empty
result, not a guessed one.

Each command has only the `at` column. classes enumerates class_defs definitions; fields enumerates the static/instance fields actually defined in each class_data; methods enumerates direct/virtual **defined methods**, including abstract/native/constructor/clinit declarations, and the command MUST NOT treat every external reference in method_ids as a defined method.

The original DEX name/proto/index is used, including the original methods involved in a synthetic/bridge flag (the flag is retained in the original declaration facts and adds no list column); Diana MUST NOT run its SSA/Java backend just to list symbols. Duplicate definitions each have an independent at, and the CLI only lists their original identity; an invalid structure is still an error.

##### strings

`diana strings <input> [--filter literal] [--format text|json] [-o file]`.

The columns are `at, value, entry`. The default here is the **string_ids value enumeration** of the original query (which covers code literals as well as names/descriptors/annotation values), and Diana MUST NOT claim to scan only the strings that appear in `const-string`. Actual code use sites are queried with `xref ... string ...`, and Diana MUST NOT confuse the two. The complete UTF-16 value is retained and escaped per §6.2.

The entry column is the physical entry's original name; the 041 logical/member identity lives in at, and Diana MUST NOT fabricate another real ZIP name. For a bare DEX the entry uses the basename of the caller-supplied input as a display label, and Diana MUST NOT use that label for identity judgements.

#### 6.5 get-class / get-method: deliver the decompilation result by default

```sh
diana get-class sample.apk 'com.example.App' -o App.txt
diana get-class sample.apk 'e0:l0:c7::Lcom/example/App;'
diana get-method sample.apk 'Lcom/example/App;->check(I)Z'
diana get-method sample.dex 'e0:l0:c7:m23::Lcom/example/App;->check(I)Z'
```

- The CLI MUST display the existing Report directly as default text and MUST NOT use VerifiedJavaUnit as a whole-class switch. When get-class yields a JavaUnit it outputs complete Java; when it yields Mixed it outputs clearly marked decompiled text: the real shell/fields, readable members and their uncertainty markers, and the Debug or Unavailable reason annotated per original method. Diana MUST retain every member and MUST NOT discard the other results because one method failed.
- get-method displays the exactly selected method: Java outputs the readable reconstruction with its uncertainty reasons and original identity; DeclarationOnly outputs the code-free declaration only, with no body; Debug outputs readable original DEX facts and coverage with the reason; and Unavailable outputs the original identity/reason with the bounded raw excerpt that is available. Diana MUST NOT call debug information executable or reassemblable source and MUST NOT fabricate Java with an empty body or a throwing stub. A method fragment needs its owner context, and Diana MUST NOT claim it compiles on its own.
- text/json only choose the representation of the same result, and they MUST NOT choose the algorithm, the fallback or the recovery threshold; Diana MUST NOT require switching to JSON to see Mixed/raw, and no `--require-java` switch is added. fields/methods remain independent symbol queries, and Diana MUST NOT prefix the source with an index table.
- When only a local Unsupported is present (including missing catalog, invoke-custom or Java target representation limits), a fully displayed Mixed/Debug/Unavailable result MAY exit0, meaning that the decompilation request and its display completed normally, not that every method was recovered as Java. Malformed, BudgetExceeded, InternalInvariant, Cancelled and I/O still fail per §6.8; a Report that can already be rendered completely MAY be kept for display with a non-zero exit, and Diana MUST NOT use a non-zero code to conceal the error.
- The top of Mixed and each failed member MUST carry the status/original identity/reason; the language section and OriginSpan can be located exactly. The marker is display metadata, and the CLI MUST NOT add an executable statement to a failed method. Diana MUST write the same display payload for an explicit `-o`; Diana MUST NOT filter out Mixed by suffix and MUST NOT automatically choose a `.java` file name for Mixed; only JavaUnit eligibility means a complete Java compilation unit.
- JSON uses the local report schema below, not an MCP envelope or serialized AST/arena. Acceptance MUST NOT golden-test format or layout; complex members, retention, raw, UTF16 and original identity/spans are accepted through the public Module interfaces.

A required readable-Java case MUST NOT pass solely because Debug was displayed. Semantic cases require independent behavior evidence where specified.
A successful compilation does not mean semantic equivalence; a readable local failure does not mean that a request error was swallowed, and the application MUST NOT treat it as a reason to re-analyze or to start another engine.

##### Decompilation JSON schema 1

The schema applies only to delivered get-class/get-method results. All listed
fields are present; empty collections are `[]`, and `null` has only the meanings
specified below. A request that cannot produce a complete payload yields an error,
not an invented empty report. JSON object-key and independent-member order are
unconstrained.

| Object / field | Type and contract |
|---|---|
| Report `schema_version` / `command` | Integer `1` / `get-class` or `get-method` |
| Report `selection` | `{class_at: string, method_at: string or null}`; canonical original locators per §6.3, with null method only for a class request |
| Report `status` | `java_unit` or `mixed` for a class; `java`, `declaration_only`, `debug` or `unavailable` for a method |
| Report `source` | Complete rendered UTF8 display string, including Mixed/failure markers; it is not automatically a Java compilation unit |
| Report `methods` | Array of Method records; every selected original member occurs once, including Generated and Unavailable |
| Report `diagnostics` | Array of Diagnostic records, including class and hard-error reasons retained for display |
| Report `spans` | Array of Span records whose ranges refer to Report.source |
| Method `at` / `status` | Canonical original method locator / one of the method statuses above |
| Method `emission` | `explicit`, `generated` or null; non-null only for Java, with Generated checked per §3.6 |
| Method `language` | `java`, `dex_debug`, `raw_excerpt` or `none`; declaration/Generated source is Java, and an excerpt never claims complete decoded coverage |
| Method `source` | Java declaration/body, code-free declaration, readable JSON debug snapshot, or available raw excerpt; null only for Generated owner syntax or Unavailable without an excerpt |
| Method `source_range` | Half-open `[start,end]` in Report.source, or null when no source representation exists; Generated points to its owner's syntax and may share a range with other Generated members |
| Method `spans` | Span array relative to this Method.source; empty when source is null |
| Method `reason` | Diagnostic for Debug/Unavailable; Java includes applicable uncertainty reasons in the report diagnostics |
| Diagnostic | `{code: string, kind: string, severity: string, method_at: string or null, code_unit: integer or null, phase: string or null, detail: string}`; kind is `malformed`, `unsupported`, `budget_exceeded`, `internal_invariant` or `cancelled`, severity follows §2.3, and a non-null code_unit requires method_at |
| Span | `{range: [start,end], member_at: string or null, sites: [{method_at: string, code_unit: integer}], synthetic_reason: string or null}`; declaration spans can have no instruction sites |

Offsets are non-negative integers in UTF8 **source bytes**, before JSON escaping;
`0 <= start <= end <= source.len()`, with endpoints on UTF8 character boundaries.
CodeUnit is a method-relative instruction coordinate, not a source offset. A
Method.source_range indexes the top-level display; a Method.spans range indexes
only that method's source. The application MUST relocate top-level spans after
prefix/marker assembly and MUST NOT substitute an in-member range for it. A Mixed
marker or unavailable reason is never a fabricated executable Java body. For an
Explicit method, source_range identifies the source payload without a surrounding
failure marker; slicing that range yields Method.source exactly. When no raw
excerpt exists, an Unavailable method still has its identity/reason record, but
source and source_range are null. Top-level diagnostic/marker spans are separate.

Top-level status expresses source eligibility, not request success: a rendered
Mixed with only Unsupported may exit0, while a retained Malformed/resource/BUG
reason still forces a hard failure. A single-method selection has exactly one
Method record; a Generated method that cannot stand alone follows the Debug rule,
not a null successful standalone source. The serializer MUST preserve escaped
UTF-16 values and charge the final JSON bytes under §7. Test assertions about
files, behavior and raw integrity come from independent case expectations, not extra production
JSON fields such as `debugFactsExact` or `tempFilesRemaining`.

#### 6.6 xref

```sh
diana xref sample.apk string 'Authorization'
diana xref sample.apk type 'Ljava/lang/String;'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z'
diana xref sample.apk field 'Lcom/example/App;->count:I'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z' --class 'com.example.Caller'
```

kind is explicitly `string|type|method|field`; target is a literal exact value/descriptor (for string it is complete UTF16 string equality, not a regex; type accepts a dotted normalized class or a standard primitive/array descriptor). A method reference MUST contain the complete proto; Diana MUST NOT provide substring/fuzzy matching and MUST NOT guess an overload from a bare value.

By default Diana scans code references in every root DEX and 041 logical member of the input, using the original opcode/ref index; the search MUST NOT decompile the source first and then search. `--class` constrains only the referencing definition, and the ambiguity rules for readable short selectors are the same as §6.3.

The columns are exactly `entry, context, match, use`:

- entry: the physical entry name of the referencing side; a display label does not decide identity.
- context: `<referencing-method-at>@cu:<offset>`, where offset is the original code units of that method, displayed in hexadecimal such as `@cu:0x0012`; it is not an output line number.
- match: the string value, TypeRef, MethodRef or FieldRef that is actually referenced, escaped per §6.2; Diana MUST NOT merely echo the user target while ignoring the real reference.
- use: the original DEX mnemonic of the referencing instruction (`check-cast`, `instance-of`, `const-class`, `invoke-interface`, `iget`, `sput`, ...). It is the format-defined instruction name, not a Java expression or a source line kind.

A site exists only where an original instruction carries a reference slot of the
queried kind. Diana MUST NOT report a declaration type, a `catch` type or a
`super_class_idx`/`interface_list` edge as an xref site; those relations are
queried through `classes` per §6.4. When one instruction references the target
twice, both slots are separate lines with the same context and `use`.

**One line per actual reference site**; multiple sites in the same method get separate lines, and the CLI MUST NOT silently merge them as the old method-level findrefs result did. When the same site may reference the parameter repeatedly, the record is expressed by the defined opcode operand-site identity, and the search MUST NOT deduplicate by name. This refinement is intentional and is accepted by H09/Q09. An empty result with header/empty items exits 0; a successful search returns every match, with no pagination and no guaranteed order.

#### 6.7 extract / manifest

##### extract: the original entry payload, not a rebuilt DEX

```sh
diana extract sample.apk 'classes.dex' -o original.dex
diana extract sample.apk 'classes.dex' --entry-index 4 -o original.dex
diana extract sample.apk 'res/xml/prefs.xml' -o prefs.xml
diana extract sample.apk 'res/xml/prefs.xml' -o prefs.axml --raw
```

`extract` handles only physical APK/ZIP entries. Original bytes means **the complete decompressed entry payload**: method 0 yields the original payload directly, and method 8 yields the decompressed original deflate stream; it is not the ZIP local header or the compressed stream, and extract MUST NOT modify the DEX header/checksum, MUST NOT slice a 041 logical view and MUST NOT do any Java/smali conversion. The file content MUST be byte-for-byte equal to the decompressed bytes of the original entry.

When the decompressed payload is binary XML (its leading chunk kind is
`RES_XML_TYPE`), extract decodes it to text XML by default, using the same bounded
reader as `manifest`; `--raw` selects the original payload bytes instead. The CRC
and size are validated over the original decompressed payload before any decoding.
A decoded document keeps the manifest warning rules of §6.7: content-affecting
defects appear in place as comments and in the warning list, and `--strict` makes
a partial document exit 1. A non-binary-XML entry is always written byte for byte,
and a DEX or any other entry MUST NOT be transcoded.

Diana MUST match `<entry-name>` exactly; a duplicate name without `--entry-index` exits 4 and lists the candidate physical indices; when an index is given, Diana MUST check that the name and index agree. Only one entry is extracted, `-o` is required and is a single file path, and Diana MUST NOT provide batch directory unpacking. In this way names such as `../` inside the ZIP cannot become output paths, which avoids ZipSlip.

By default an existing output is not overwritten; `--force` explicitly allows replacing an ordinary file, and Diana MUST reject an output directory, a symbolic link, the input file itself or a hardlink alias. Diana MUST create a private temporary file in the target directory first and commit atomically after bounded decompression and integrity validation; on cancellation, a decompression error or an insufficient budget, the implementation MUST clean up the temporary file and MUST NOT output a half product. Diana MUST NOT let a no-force publish have a check-then-rename overwrite race; acceptance MUST also cover force per platform semantics. The CLI MUST NOT print binary data or status lines to stdout; errors/`--debug` go only to stderr.

extract MUST report an unsupported compression method, encryption and a bad range/size/CRC as explicit errors, and Diana MUST NOT take a successful directory listing to imply that extraction is reliable. extract MUST verify CRC by traversing the actual payload per [01-input](01-input.md) and MUST NOT depend on codec default behavior. The public domain reasons distinguish AmbiguousEntry, TargetExists, UnsafeTarget, CrcMismatch, BudgetExceeded, Cancelled, UnsupportedCompression, EncryptedEntry and I/O; extract MUST NOT report an existing target in place of a CRC/budget/unsupported-format failure. Writing/copying checks cancellation and budget at most every 64KiB; the public cancellation callback may observe the final file state read-only, and no separate private publish hook is added. The budget is subject to the original input/inflate limits and is not pagination.

##### manifest

`diana manifest <apk-or-zip> [-o file] [--format text|json]`.

By default Diana outputs the decoded `AndroidManifest.xml`; the original binary AXML bytes are obtained with extract. The original project's rule of selecting the last physical entry with that name is retained (based on directory order, not on worker completion order), so the manifest selection does not change because of unordered display. A missing manifest exits 3; a bare DEX or an ordinary JAR without a manifest is an input error.

Attribute names come from the original string pool. When a file leaves them empty
and identifies attributes only through `RES_XML_RESOURCE_MAP`, Diana MUST use the
public Android attribute table for those ids and MUST otherwise emit the id itself
(`attr_0x...`), declaring the Android namespace it needs. A rewritten name is a
warning, never a silent change.

Manifest decoding is allowed to deliver a **balanced partial** document. A record
that cannot be decoded, an unreadable text node, an out-of-pool name, an
unsupported typed value, a repeated attribute or a damaged tail MUST be reported
as a warning with its original byte offset, and the reader MUST close the elements
that are still open, so the emitted text stays well-formed XML. A warning that
removes or replaces original content (skipped text or record, dropped or repeated
attribute, truncated tail, closed element) MUST also appear as an in-place XML
comment at that position, because an agent that reads only stdout would otherwise
receive a shorter document without noticing. A naming substitution that keeps the
value (a resolved resource id, or a name rewritten into the XML name production)
is recorded in the warning list but MUST NOT emit a comment per attribute, so the
text stays proportional to the input. A text/CDATA record is rendered in place as
escaped character data; string-pool style spans are not rendered, and that
limitation is recorded rather than hidden. All warnings appear in the `warnings` array
of the JSON result and on stderr; they MUST NOT be silently dropped, and a
warning-only result exits 0 because the request and its display completed. Where
the strict policy is requested, a warning makes the exit non-zero. A non-AXML container, an unreadable or encrypted entry, an
exceeded explicit budget, cancellation and I/O remain hard failures with a
non-zero exit; the reader MUST NOT invent records to hide those.

text is XML; json is `{ "xml": "..." }` with no fake Java status. The application
uses a reviewed native AXML reader through the checked contract of [01-input](01-input.md)
§14.4, with no automatic external tool. Focused reuse of the patched vendored
reader requires review of its PATCHES, all reachable reads/allocations and depth
handling; a patch list or chunk-skeleton check is not a safety certificate. Diana
MUST NOT depend on the removed archived reader. Independently validate a decoder
change through the public manifest Module within the agreed core test scope.

#### 6.8 Help, errors and exit codes

- `diana help`, `diana --help`, `diana help get-method` / `diana get-method --help` succeed with exit0.
- exit0: a complete query/extract/manifest, or a get decompilation result without a hard error that was delivered completely; text/json follow the same rule, and a local Unsupported that yields Mixed/Debug/Unavailable does not by itself force failure.
- exit1: request failures such as I/O, Malformed, budget, internal BUG, cancellation, unsupported input or a canonical locator whose indices disagree with its descriptor/name/proto; an existing decompilation Report that can be delivered completely is not cleared because of that, but the exit is still non-zero. When global damage prevents locating the object, when Cancelled leaves no Report or when render exceeds its limit, Diana MUST NOT fabricate a report or truncate the payload.
- exit2: unknown command/flag, syntactically invalid selector/proto, missing input, missing `-o` for extract, or an inapplicable option combination.
- exit3: no result for a unique object/entry/manifest. An empty match for a list/xref is a normal exit0.
- exit4: the unique selection is ambiguous, for example a duplicate class/method/entry name, and Diana MUST reject taking the first item arbitrarily.

The CLI MUST end normally on BrokenPipe per the original Unix filter behavior and MUST NOT print a panic. The project name is uniformly Diana; Diana MUST NOT provide the previously proposed interfaces `getclass`, `findrefs`, `skill`, `--members`, `--require-java`, `--class-id`, `--offset` and `--limit`, because the canonical at already resolves exact identity.

`mcp` is deferred, the old implementation survives only in git history, and Diana MUST NOT restore it as an active interface or design a new protocol; Diana MUST NOT make the old agent skill installation logic migrate or delete user directories automatically. Agent usage instructions are ordinary documentation/release assets, and Diana MUST NOT add a twelfth public `skill` command.

#### 6.9 Interface validation

The basic conventions are checked per this chapter; complex behavior is validated through the public library/module interfaces, without starting a CLI subprocess.
A query compares original identity and content, extract compares the original payload and file safety, and decompilation validates values/exceptions/effects;
Diana MUST NOT use formatting or synthetic names as the semantic contract.
