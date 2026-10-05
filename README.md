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

For several queries over the same compressed APK/DEX, rasc can run as a persistent stdio MCP
server. It copies an authorized input into a private immutable snapshot and reuses only bytes that
were actually produced by ZIP deflate decompression. Bare DEX and ZIP-stored entries borrow the
snapshot directly; parser/decompiler state, source, manifests, and query results are not cached.
The server exposes exactly `open`, `close`, `status`, `classes`, `strings`, `findrefs`, `getclass`,
`manifest`, and `entries`.

Configure rasc once for all Pi projects in the user-level `~/.pi/agent/mcp.json`:

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

Replace the root with the directory containing inputs that rasc may open, then run `pi mcp list`
to verify the connection. A project-level `.pi/mcp.json` override is optional when one project needs
a narrower root or different limits; it is not required for normal use.

Call `open` once, retain its process-local `target_id`, consume complete typed results, and call
`close`. The server does not paginate or provide generic list filters: code mode should filter,
sort, slice, map, aggregate, and combine `structuredContent` in JavaScript. Results exceeding the
configured item or serialized-byte budget fail atomically with `RESOURCE_LIMIT`. `classes`,
`strings`, and `findrefs` reuse one process-wide Rayon pool and can scan independent physical DEX
entries concurrently when internal admission thresholds predict enough parallel work; smaller or
highly skewed workloads fall back to ordered serial traversal. Results are always committed in
physical-entry, logical-member, then row order, and logical members of a DEX 041 container remain
serial. This is a bounded capability, not a general latency guarantee. `--analysis-threads` sizes
that pool (default: available CPUs). This is separate from the fail-fast
`--max-concurrent-requests` request budget
(default 2), so excess parallel calls return `RESOURCE_LIMIT` rather than queueing unbounded
blocking work. Paths outside `--root` are rejected. MCP stdout is protocol-only and diagnostics use
stderr.

MCP is optional. Pi code mode can already invoke the normal CLI through `tools.bash()` (best for
one-shot, streaming, or CLI-filtered output), while a Pi extension can register Pi-specific tools.
Choose MCP for standard typed discovery, non-Pi clients, explicit long-lived targets, or repeated
queries that benefit from reusing deflated entry bytes. See
[the headless design and code-mode examples](docs/headless-mcp.zh-CN.md). The example's client-side
composition has a reproducible Pi QuickJS check: `node tools/verify_codemode.mjs`.

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
