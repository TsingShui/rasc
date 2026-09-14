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
rasc findrefs app.apk string Authorization            # references in every root DEX
rasc findrefs app.apk type Gson
rasc findrefs app.apk method onCreate --class com.example.Main
rasc findrefs app.apk field INSTANCE --class example --fuzzy-class
rasc classes app.apk [-f substring]                   # class index
rasc manifest app.apk                                 # binary AndroidManifest.xml -> XML
```

## Notes

- `getclass` accepts a dotted name or a Dalvik descriptor (`Lcom/example/Main;`); a class
  that does not exist is an error (exit 1), not empty output.
- `findrefs` queries are literal, not regex. Rows: `classesN.dex | referencing member | matched=(referenced targets)`.
- `classes` rows: `dex | descriptor | dotted name | package=... | class=...`. The full index
  is tens of MiB, so filter it (`-f`) or pipe it.
- `-o FILE` writes exactly the stdout bytes; `--threads N` caps parallelism (default: one
  worker per CPU); `--debug` prints timings to stderr.
- Update this skill with `rasc skill` (pi, Codex, Claude Code; `--print` for anything else).
