# rasc

## What rasc is for

rasc is a fast Rust CLI for analyzing Android and Java bytecode. It is a reimplementation of
[ASC](https://github.com/MG1937/ASC) with native APK/DEX parsing, reference search, class indexing,
manifest decoding, and Java-like class decompilation.

Supported inputs:

| Input | Code analysis | How it is handled |
|---|---:|---|
| APK | Yes | Root `classes*.dex` entries are analyzed natively |
| DEX | Yes | Analyzed natively |
| DEX archive/JAR | Yes | Root `classes*.dex` entries are analyzed natively |
| Conventional JAR | Yes | `.class` bytecode is converted with a system-installed `d8` |
| AAR | Yes | `classes.jar` and `libs/*.jar` are converted with a system-installed `d8` |

rasc does not bundle d8 or Java. APK/DEX analysis remains self-contained; JAR/AAR code analysis is
an optional CLI path that requires Android SDK Build Tools and a Java runtime.

## Build and install

Prebuilt macOS binaries (Apple Silicon and Intel) are on the
[Releases](https://github.com/TsingShui/rasc/releases) page. Install the latest version directly
from GitHub with Cargo:

```sh
cargo install --git https://github.com/TsingShui/rasc
rasc --help
```

To build and install from a local checkout instead:

```sh
cargo install --path . # builds with cargo build --release
rasc --help
```

## Usage

```sh
rasc getclass app.apk com.example.Main                 # one class -> Java-like source
rasc getclass library.aar com.example.LibraryClass     # AAR/JAR through system d8
rasc getclass --threads 16 -o Main.java app.apk 'Lcom/example/Main;'
rasc getclass --members app.apk com.example.Main       # prefix source with member indices
rasc findrefs app.apk string Authorization             # references across every root DEX
rasc findrefs library.jar method validate --class com.example.Validator
rasc findrefs app.apk field INSTANCE --class example --fuzzy-class
rasc classes app.apk                                   # class index
rasc classes -f service library.aar                    # filtered AAR class index
rasc strings --filter token --limit 100 app.apk
rasc manifest app.apk                                  # binary AndroidManifest.xml -> XML
rasc entries library.aar                               # original archive entries
rasc fields-plan --apk app.apk --descriptor 'Lcom/example/Foo;'
rasc member-by-index --apk app.apk --descriptor 'Lcom/example/Foo;' --field-index 3
rasc member-by-index --apk app.apk --descriptor 'Lcom/example/Foo;' --method-index 339
rasc skill                                             # install the rasc skill for coding agents
```

See `rasc --help` for more usage information, or `rasc <command> --help` for command-specific options.

### JAR and AAR inputs

Code commands automatically convert conventional Java bytecode to a temporary DEX archive before
running the existing analyzer:

- `classes`
- `strings`
- `findrefs`
- `getclass` (including `--members`)
- `fields-plan`
- `member-by-index`

For an AAR, rasc converts every non-empty `classes.jar` and direct `libs/*.jar`. `entries` and
`manifest` always inspect the original input and never invoke d8. Temporary JAR and DEX files are
removed when the command exits.

#### Requirements and discovery

Install a Java runtime and Android SDK Build Tools. rasc searches for d8 in this order:

1. `--d8 FILE`
2. `RASC_D8`
3. `$ANDROID_SDK_ROOT/build-tools/*/d8`
4. `$ANDROID_HOME/build-tools/*/d8`
5. the standard Android SDK directory for the current platform
6. `PATH`

When d8 belongs to an Android SDK, rasc also passes the newest available platform `android.jar` as
its library input. `RASC_ANDROID_JAR` can override that file.

```sh
rasc classes library.jar                            # automatic discovery
rasc --d8 "$ANDROID_HOME/build-tools/36.0.0/d8" classes library.aar
RASC_D8=/opt/android/build-tools/36.0.0/d8 rasc getclass library.jar com.example.Main
rasc --no-d8 classes app.apk                        # never invoke an external converter
```

Conversion uses debug mode and `--min-api 1`, and times out after 120 seconds. Set
`RASC_D8_TIMEOUT_SECS` to a positive number to change the timeout. d8 diagnostics go to stderr and
are limited to their final 64 KiB; query results remain on stdout.

Because d8 desugars Java bytecode, generated lambda classes and synthetic methods can appear in
results. JAR/AAR conversion is available only in the one-shot CLI; the MCP server currently accepts
APK and DEX inputs only.

### Headless MCP

MCP is a quick, standard way to connect rasc to Pi and other MCP-compatible coding agents. Agents
with code mode can call rasc tools and process their structured results in JavaScript.

Configure rasc globally for Pi in `~/.pi/agent/mcp.json`:

```json
{
  "mcpServers": {
    "rasc": {
      "command": "rasc",
      "args": ["mcp"],
      "exposure": "codemode",
      "description": "High-performance native alternative to jadx for APK and DEX analysis"
    }
  }
}
```

Run `pi mcp list` to verify the connection. By default, rasc can open inputs under the current
working directory; use `--root DIR` only when an explicit access boundary is needed.

Available MCP tools:

- `open` - open an APK or DEX and return a `target_id` (JAR/AAR conversion is not supported here)
- `close` - close a target and release its resources
- `status` - show server, target, and cache status
- `classes` - list classes
- `strings` - list strings
- `findrefs` - find string, type, method, or field references
- `getclass` - decompile one class
- `manifest` - decode `AndroidManifest.xml`
- `entries` - list APK archive entries

The usual flow is `open` → analysis tools using the returned `target_id` → `close`. See
[the headless MCP guide](docs/headless-mcp.zh-CN.md) for limits and code-mode examples.

### Coding agents

`rasc skill` installs a single-file [skill](skill/SKILL.md) - rasc's scope plus one line per
command - so a coding agent knows when to reach for rasc and how to call it:

```sh
rasc skill                        # every agent that looks installed, else ~/.agents/skills
rasc skill pi codex claude        # or pick agents explicitly
rasc skill --dir .claude/skills   # any skills directory (a project's, for example)
rasc skill --print                # the skill text on stdout, nothing written
```

## License

Apache-2.0. The full text is in [LICENSE](LICENSE); [NOTICE](NOTICE) records the attribution of
the components bundled in the tree (`vendor/axmldecoder`, `crates/dexdec`, `crates/rusty-dex`).
