---
name: rasc
description: Analyze APK and DEX files with the rasc CLI, or ordinary Java JAR files after explicit one-time d8 conversion - decompile one class, find code references (string, type, method, field), list classes, or decode AndroidManifest.xml. Use for Android reverse engineering, for locating a class or caller, or when a task mentions rasc, ASC, jadx, androguard, dex, smali, JAR or d8.
---

# rasc

Fast native CLI for APK/DEX analysis, a re-implementation of ASC. Every command writes its
payload to stdout and diagnostics to stderr, so results pipe cleanly into `grep`, `awk` or
`-o FILE`.

## Commands

```sh
rasc getclass app.apk com.example.Main                # one class -> Java-like source
rasc getclass --members app.apk com.example.Main      # the same, prefixed by its member indices
rasc findrefs app.apk string Authorization            # references in every root DEX
rasc findrefs app.apk type Gson
rasc findrefs app.apk method onCreate --class com.example.Main
rasc findrefs app.apk field INSTANCE --class example --fuzzy-class
rasc classes app.apk [-f substring]                   # class index
rasc manifest app.apk                                 # binary AndroidManifest.xml -> XML
rasc fields-plan app.apk --descriptor 'Lcom/example/Foo;'      # field layout + instance reference mask
rasc member-by-index app.apk --descriptor 'Lcom/example/Foo;' --field-index 3
rasc member-by-index app.apk --descriptor 'Lcom/example/Foo;' --method-index 339
rasc mcp --root /authorized/apks                     # persistent stdio MCP server
```

## Ordinary Java JAR files: convert once, reuse the DEX

rasc does not invoke d8 automatically and cannot directly analyze `.class` bytecode in an
ordinary JAR. Use an external d8 from Android SDK Build Tools (requires Java) to convert the
whole JAR **once**, then reuse the resulting DEX ZIP for every query. A JAR/ZIP already holding
root `classes*.dex` entries can be passed directly to rasc without conversion.

For temporary analysis, put conversion output under `/tmp/rasc/<random-directory>/` rather
than in the project directory. Create one private workspace per input/analysis session:

```bash
# d8 must be on PATH, or replace it with /path/to/android-sdk/build-tools/<version>/d8.
mkdir -p -m 700 /tmp/rasc || exit 1
# Refuse a shared base directory owned by another user or replaced with a symlink.
[ -d /tmp/rasc ] && [ ! -L /tmp/rasc ] && [ -O /tmp/rasc ] || exit 1
chmod 700 /tmp/rasc || exit 1
workdir=$(mktemp -d /tmp/rasc/XXXXXX) || exit 1
dex_zip="$workdir/library-dex.zip"
d8 --debug --min-api 1 --output "$dex_zip" library.jar || exit 1
printf 'Reuse this DEX ZIP: %s\n' "$dex_zip"

rasc classes "$dex_zip" -f example
rasc getclass "$dex_zip" com.example.Scanner
rasc getclass "$dex_zip" com.example.Winnowing
rasc findrefs "$dex_zip" method scan --class com.example.Scanner
```

- Keep conversion explicit: do not rerun d8 for each class or query. Reuse the same workspace
  and DEX ZIP for all queries over this input. Shell variables may not survive separate tool
  calls; record the printed absolute path and pass that path to subsequent rasc commands.
- Reconvert when the input JAR, d8 version, library inputs, or conversion options change, or
  when the temporary output has been removed; rasc does not manage this cache.
- After analysis finishes, remove only the workspace created above with `rm -rf -- "$workdir"`
  (or its recorded absolute path), not the whole `/tmp/rasc` directory. Temporary files may also
  be removed by the system. For long-term repeated analysis, use a user-selected persistent
  output directory instead. These are external d8 instructions, not automatic rasc behavior.
- For Android API references, add `--lib /path/to/android-sdk/platforms/android-<api>/android.jar`
  to the d8 command. Supply any other required dependencies with d8's `--lib` or `--classpath`
  options as appropriate; check conversion diagnostics rather than assuming success.
- d8 may desugar bytecode and generate lambda classes or synthetic methods. Results and member
  indices describe the converted DEX, not the original JVM bytecode or a different APK build.
- `--d8`, `--no-d8`, and `RASC_D8` are not supported by rasc. Conversion belongs outside rasc;
  only pass the resulting DEX or DEX ZIP to its code commands. `manifest` does not apply to an
  ordinary JAR or this generated DEX ZIP.

## Indices a runtime trace hands back

A trace from a runtime carries indices, not names. These three commands turn one into the
member behind it:

```sh
rasc fields-plan app.apk --descriptor 'Lcom/example/Foo;'
rasc member-by-index app.apk --descriptor 'Lcom/example/Foo;' --field-index 3
rasc member-by-index app.apk --descriptor 'Lcom/example/Foo;' --method-index 339
```

- `fields-plan` prints one JSON record: `dex_class_def_idx`, `dex_type_idx`,
  `class_access_flags`, the instance and static fields in DEX declaration order, and
  `instance_ref_mask` (bit *j* = instance field *j* is a reference). It is the one command
  whose consumer is a program, which is why it is JSON.
- `getclass --members` prefixes the source with one line per member, so a runtime index can be
  matched to the member in the source in the same call:
  `# members fields: field_ids=175 slot=0 static=false flags=0x12 type=[Ljava/lang/String; name=mArgs`
  and `# members methods: method_ids=339 proto=(…)V flags=0x10001 name=<init>`. The prototype is
  what identifies one method among overloads, and on an obfuscated build the name is no help at
  all. Without the flag the output is byte-for-byte what it has always been.
- `member-by-index` takes exactly one index, because the two live in different spaces.
  `--field-index` is the position a runtime reports - instance fields first, then statics -
  which is **not** the `field_ids` index, so the row prints `field_ids=` too.
  `--method-index` is the `method_ids` index, which is what a runtime stores.
- Exit status is the verdict for both index commands: `0` one answer, `3` none (no input
  defines the class, or it declares no such index), `4` more than one input defines the class.
  On `4` `member-by-index` prints every candidate, and `fields-plan` prints nothing: a plan
  decides which instance slots a collector may dereference, so picking one of two layouts
  would be arbitrary.

## Notes

- `getclass` accepts a dotted name or a Dalvik descriptor (`Lcom/example/Main;`); a class
  that does not exist is an error (exit 1), not empty output.
- `findrefs` queries are literal, not regex. Rows: `classesN.dex | referencing member | matched=(referenced targets)`.
- `classes` rows: `dex | descriptor | dotted name | package=... | class=...`. The full index
  is tens of MiB, so filter it (`-f`) or pipe it.
- `-o FILE` writes exactly the stdout bytes; `--threads N` caps parallelism (default: one
  worker per CPU); `--debug` prints timings to stderr.
- For several queries over the same compressed input, optionally configure `rasc mcp --root DIR`
  as a typed stdio MCP server. Call `open` once, pass its `target_id` to `classes`, `strings`,
  `findrefs`, `getclass`, `manifest`, and `entries`, then `close` it. Lists and text are complete,
  not paged; filter/sort/slice/map/aggregate `structuredContent` in code mode. The server snapshots
  inputs and caches only ZIP-deflated entry bytes, not parser/decompiler state or results. MCP
  `--analysis-threads N` sizes a shared Rayon pool that `classes`, `strings`, and `findrefs` can use
  across physical DEX entries when internal admission thresholds predict enough parallel work;
  smaller or skewed workloads use ordered serial traversal. This is a capability, not a latency
  guarantee, and is separate from the fail-fast `--max-concurrent-requests` request budget. Output
  order stays physical entry, logical member, then row. Analysis concurrency and
  complete response size are bounded; excess work fails with `RESOURCE_LIMIT`. Paths outside
  configured roots are rejected. For one-shot or streaming work, code mode can call the
  normal CLI with `tools.bash()`; a Pi extension tool is another Pi-only integration option.
- Update this skill with `rasc skill` (pi, Codex, Claude Code; `--print` for anything else).
