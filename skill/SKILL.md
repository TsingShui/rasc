---
name: rasc
description: Analyze APK and DEX files with the rasc CLI - decompile one class, find code references (string, type, method, field), list classes, or decode AndroidManifest.xml. Use for Android reverse engineering, for locating a class or a caller inside an APK, or when a task mentions rasc, ASC, jadx, androguard, dex or smali.
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
rasc field-by-index app.apk --descriptor 'Lcom/example/Foo;' --field-index 3
rasc method-by-index app.apk --descriptor 'Lcom/example/Foo;' --method-index 339
```

## Indices a runtime trace hands back

A trace from a runtime carries indices, not names. These three commands turn one into the
member behind it:

```sh
rasc fields-plan app.apk --descriptor 'Lcom/example/Foo;'
rasc field-by-index app.apk --descriptor 'Lcom/example/Foo;' --field-index 3
rasc method-by-index app.apk --descriptor 'Lcom/example/Foo;' --method-index 339
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
- `field-by-index` takes the index a runtime reports: instance fields first, then statics.
  That is **not** the `field_ids` index, so the row prints `field_ids=` too.
  `method-by-index` takes the `method_ids` index, which is what a runtime stores.
- Exit status is the verdict for all three: `0` one answer, `3` none (no input defines the
  class, or it declares no such index), `4` more than one input defines the class. On `4` the
  index lookups print every candidate, and `fields-plan` prints nothing: a plan decides which
  instance slots a collector may dereference, so picking one of two layouts would be arbitrary.

## Notes

- `getclass` accepts a dotted name or a Dalvik descriptor (`Lcom/example/Main;`); a class
  that does not exist is an error (exit 1), not empty output.
- `findrefs` queries are literal, not regex. Rows: `classesN.dex | referencing member | matched=(referenced targets)`.
- `classes` rows: `dex | descriptor | dotted name | package=... | class=...`. The full index
  is tens of MiB, so filter it (`-f`) or pipe it.
- `-o FILE` writes exactly the stdout bytes; `--threads N` caps parallelism (default: one
  worker per CPU); `--debug` prints timings to stderr.
- Update this skill with `rasc skill` (pi, Codex, Claude Code; `--print` for anything else).
