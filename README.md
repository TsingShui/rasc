# rasc


## What rasc is for

rasc is a Rust implementation of [ASC](https://github.com/MG1937/ASC), built to analyze APKs and
DEX files at very high speed. Most of the implementation was carried out by agents with occasional
human intervention, and it differs from ASC in some implementations and optimizations. It ships as a native CLI.

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
rasc getclass --threads 16 -o Main.java app.apk 'Lcom/example/Main;'
rasc getclass --members app.apk com.example.Main       # source prefixed by per-member indices
rasc findrefs app.apk string Authorization             # references across every root DEX
rasc findrefs app.apk method onCreate --class com.example.Main
rasc findrefs app.apk field INSTANCE --class example --fuzzy-class
rasc classes app.apk                                   # class index
rasc manifest app.apk                                  # binary AndroidManifest.xml -> XML
rasc fields-plan app.apk --descriptor 'Lcom/example/Foo;'       # field layout + instance ref mask (JSON)
rasc member-by-index app.apk --descriptor 'Lcom/example/Foo;' --field-index 3
rasc member-by-index app.apk --descriptor 'Lcom/example/Foo;' --method-index 339
rasc skill                                             # install the rasc skill for coding agents
```

See `rasc --help` for more usage information, or `rasc <command> --help` for command-specific options.

### Headless MCP

MCP is a quick, standard way to connect rasc to Pi and other MCP-compatible coding agents. Agents
with code mode can call rasc tools and process their structured results in JavaScript.

Configure rasc globally for Pi in `~/.pi/agent/mcp.json`:

```json
{
  "mcpServers": {
    "rasc": {
      "command": "rasc",
      "args": ["mcp", "--root", "/absolute/path/to/authorized-inputs"],
      "exposure": "codemode",
      "description": "Persistent native APK/DEX analysis"
    }
  }
}
```

Replace `--root` with the directory containing the APK/DEX files that rasc may access, then run
`pi mcp list` to verify the connection.

Available MCP tools:

- `open` - open an APK or DEX and return a `target_id`
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
