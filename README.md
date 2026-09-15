# rasc

[中文说明](README.zh-CN.md) · **Source: <https://github.com/TsingShui/rasc>**

## What rasc is for

rasc is a Rust implementation of [ASC](https://github.com/MG1937/ASC), built to analyze APKs and
DEX files at very high speed. Most of the implementation was carried out by agents with occasional
human intervention, and it differs from ASC in some implementations and optimizations. It has two
core targets, the native CLI and `wasm32-wasip1` (a program, for hosts without a filesystem of their own), with the CLI as the primary one.

## Performance and trade-offs

Across 11 test scenarios on a 343 MiB APK, rasc achieves a geometric mean speedup of
**7.1×** over ASC. The measurement method and detailed results are in the collapsed
section below.

That speed is not free. As optimization progressed, some of rasc's designs have moved away
from ASC; it is no longer a line-by-line translation. It favors throughput and is willing to
spend more memory for speed: for example, multithreaded reference searches have a higher peak
memory footprint than ASC. This is a trade-off, not a claim of lower resource use everywhere.

The speedup should first be understood in the context of moving from Python to a native
Rust implementation—not as proof that Rust beats other languages or that agents beat
human developers. Algorithms, parallelism, and memory strategies also affect the result.
An implementation in Zig or C++ might go further.

<details>
<summary>Benchmark results and memory usage</summary>

### Test conditions

- Apple M3 Pro (6 performance + 6 efficiency cores), 36 GiB RAM, macOS 26.5.1.
- ASC: CPython 3.12.14, Androguard 4.1.4. rasc: Rust release build with thin LTO
  (`[profile.release]` in `Cargo.toml`), Java output from the vendored `dexdec`.
- Input: 56 root DEXes, 567,192 classes. ASC runs with 8 workers; rasc defaults to
  one worker per logical CPU (12 on the machine above, `--threads` overrides it).
- End-to-end wall time: fresh process per sample, randomized execution order, median of
  at least 3 runs. Output is discarded, but formatting and writing are included.
- Exit status and output are checked before timing; result sets are compared where applicable.
- Measured 2026-09-14, on the revision released as `v0.1.0`.

### Execution time

| Scenario | rasc | ASC | Speedup |
|---|---:|---:|---:|
| `findrefs string Authorization` | 132 ms | 638 ms | 4.8× |
| `findrefs string okhttp` | 154 ms | 747 ms | 4.8× |
| `findrefs type Gson` | 166 ms | 1,047 ms | 6.3× |
| `findrefs method onCreate` | 174 ms | 1,379 ms | 7.9× |
| `findrefs method onCreate --class androidx --fuzzy-class` | 165 ms | 1,285 ms | 7.8× |
| `findrefs field INSTANCE` | 198 ms | 1,762 ms | 8.9× |
| `getclass` early class (`classes.dex`) | 62 ms | 137 ms | 2.2× |
| `getclass` late class (`classes56.dex`) | 87 ms | 323 ms | 3.7× |
| `getclass` missing class | 55 ms | 357 ms | 6.4× |
| `manifest` | 17 ms | 334 ms | 20.1× |
| `classes` | 97 ms | 2,733 ms | 28.2× |
| **Geometric mean** | | | **7.1×** |

ASC has no CLI command for `manifest` or `classes`; the benchmark calls the underlying
functions used by its GUI. Search semantics also differ: rasc uses literal queries and
instruction-boundary scanning, so arbitrary queries need not produce identical results.

### Memory

Peak RSS on the same APK with 8 workers (measured 2026-09-13, before the emitter switch;
the shapes are unchanged, the absolute numbers are from that revision):

| Scenario | rasc | ASC |
|---|---:|---:|
| `findrefs string Authorization` | 343 MiB | 220 MiB |
| `findrefs field INSTANCE` | 364 MiB | 239 MiB |
| `getclass` early / late | 105 / 151 MiB | 208 / 425 MiB |
| `manifest` | 8 MiB | 16 MiB |
| `classes` | 233 MiB | Not measured |

rasc uses more memory for parallel reference searches, but less for class decompilation
and manifest decoding in these measurements. Reducing workers trades speed for memory:
a separate `findrefs field INSTANCE` run with `--threads 2` used 256 MiB and remained
3.3× faster than ASC.

</details>

## Build and install

Prebuilt macOS binaries (Apple Silicon and Intel) are on the
[Releases](https://github.com/TsingShui/rasc/releases) page. To build and install from source:

```sh
cargo install --path . # built with cargo build --release
rasc --help
```

## Usage

```sh
rasc getclass app.apk com.example.Main                 # one class -> Java-like source
rasc getclass --threads 16 -o Main.java app.apk 'Lcom/example/Main;'
rasc findrefs app.apk string Authorization             # references across every root DEX
rasc findrefs app.apk method onCreate --class com.example.Main
rasc findrefs app.apk field INSTANCE --class example --fuzzy-class
rasc classes app.apk                                   # class index
rasc manifest app.apk                                  # binary AndroidManifest.xml -> XML
rasc skill                                             # install the rasc skill for coding agents
```

See `rasc --help` for more usage information, or `rasc <command> --help` for command-specific options.

### Coding agents

`rasc skill` installs a single-file [skill](skill/SKILL.md) - rasc's scope plus one line per
command - so a coding agent knows when to reach for rasc and how to call it:

```sh
rasc skill                        # every agent that looks installed, else ~/.agents/skills
rasc skill pi codex claude        # or pick agents explicitly
rasc skill --dir .claude/skills   # any skills directory (a project's, for example)
rasc skill --print                # the skill text on stdout, nothing written
```

## Libraries used and open-source projects drawn on

| Source | Upstream | What it does here |
|---|---|---|
| `ASC` | [MG1937/ASC](https://github.com/MG1937/ASC) (Apache-2.0) | The reference implementation and the benchmark baseline (Python + Androguard): rasc started from it. |
| `crates/dexdec` | [asLody/dexdec](https://github.com/asLody/dexdec) (Apache-2.0) | The Java decompiler behind `getclass`: DEX → CFG/SSA → regions → Java source. Fork notes: [`FORK.md`](crates/dexdec/FORK.md); the deltas: [`PATCHES.md`](crates/dexdec/PATCHES.md). |
| `crates/rusty-dex` | [rusty-rs/rusty-dex](https://github.com/rusty-rs/rusty-dex) (Apache-2.0) plus this repository's extensions | The DEX parser `dexdec` reads bytes with. The lazy string and id-pool decoding lives here, as does the smali-ready instruction layer. |
| `vendor/axmldecoder` | [axmldecoder](https://crates.io/crates/axmldecoder) (Apache-2.0 OR MIT) | Binary AndroidManifest.xml → text XML for `rasc manifest`. |

## License

Apache-2.0. The full text is in [LICENSE](LICENSE); [NOTICE](NOTICE) records the attribution of
the components bundled in the tree (`vendor/axmldecoder`, `crates/dexdec`, `crates/rusty-dex`).
