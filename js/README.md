# rasc wasm SDK (JS host)

Host side of the `wasm32-unknown-unknown` build. The module has **no filesystem**: it asks
the host for the byte ranges it needs and hands its output back, so the host decides where
the bytes come from and the archive never has to exist inside wasm.

| File | Role |
| --- | --- |
| `rasc.mjs` | Shared: the `Rasc` class, the five imports, `sourceFromBytes`. Imports nothing, so it loads in a browser too. |
| `node.mjs` | Node host: `sourceFromFile` (`fs.readSync`), `loadFromFile`, and the default module path. |
| `browser.mjs` | Browser host: `sourceFromBlob` (worker `FileReaderSync`), `loadFromBlob`. |
| `worker.mjs` | Ready-made module-worker entry: load a `File`, then stream command results back. |
| `bin/rasc-wasm.mjs` | Drop-in command line: same arguments, stdout, and exit code as the native binary. |
| `test.mjs` | Compares host output and exit code against the native binary, per command. |
| `cli-test.mjs` | The same comparison for the command-line wrapper, including the stderr contract. |

## Build

```sh
cargo build --release --target wasm32-unknown-unknown
```

## Command line

The module is also usable as a command line tool with the native binary's contract - same
arguments, payload on stdout and nothing else, errors on stderr, same exit code:

```sh
node js/bin/rasc-wasm.mjs manifest app.apk
node js/bin/rasc-wasm.mjs findrefs --threads 1 app.apk field INSTANCE
node js/bin/rasc-wasm.mjs skill --print
RASC_WASM=path/to/rasc.wasm node js/bin/rasc-wasm.mjs classes app.apk
```

`package.json` exposes it as `rasc-wasm`, so an installed package gets it as a bin. The
archive argument is found among the arguments (the first that names an existing file) and
served with `fs.readSync`; `--help`/`--version` and the archive-free `skill` command are
answered by the module.

```sh
APK=/path/to/app.apk node js/cli-test.mjs      # 16 checks against the native binary
```

One deliberate difference: with no archive argument the wrapper reports its own usage error
and exits 2, while the native binary would exit 1 on the unopenable path. Both leave stdout
empty; only a usage error differs this way.

## Node

```js
import { loadFromFile } from './js/node.mjs';

// Ranges come from the file on demand: the host holds a descriptor, not a copy.
const rasc = await loadFromFile({ apk: 'app.apk' });
const { code, output } = rasc.run(['findrefs', '--threads', '1', 'app.apk', 'field', 'INSTANCE']);

// Large payloads can be streamed instead of collected:
rasc.run(['classes', '--threads', '1', 'app.apk'], {
  onOutput: (bytes) => process.stdout.write(bytes),
});
rasc.close();
```

Check it:

```sh
APK=/path/to/app.apk node js/test.mjs
```

## Browser

Only a **worker** can read a `Blob` synchronously: `FileReaderSync.readAsArrayBuffer()`
takes a `Blob`/`File` and *returns* an `ArrayBuffer`
([MDN](https://developer.mozilla.org/en-US/docs/Web/API/FileReaderSync/readAsArrayBuffer)).
So the module runs in a module worker and every range is served from `blob.slice(...)`:

```js
const worker = new Worker('js/worker.mjs', { type: 'module' });
worker.postMessage({ wasm, file });                     // wasm: ArrayBuffer, file: File
worker.onmessage = ({ data }) => { if (data.ready) worker.postMessage({ argv: ['classes', '--threads', '1', 'app.apk'] }); };
```

That needs **no change to the core**. The cost is one Blob slice plus one ArrayBuffer per
range; the page stays responsive because the work happens in the worker, and output is
streamed back in chunks.

A remote archive works the same way: `sourceFromUrl` uses a **worker-scoped synchronous
XHR** with `Range` requests, which is the only way to answer a synchronous import from the
network (`fetch` returns a promise). The server should support Range; if it ignores the
header the whole archive arrives in one response and is kept in memory instead.

```js
worker.postMessage({ wasm, url: '/app.apk' });
```

Sync XHR is deprecated, so an environment that drops it would need prefetching or a
resumable core; local files are covered by `FileReaderSync` regardless.

**Measured in Chrome 149** (headless, same 343 MiB APK): the module's own wasm linear memory
peaks at **77 MiB**, identical in both modes, against a **939 MiB** blank-page Chrome
baseline. The process tree peaks at 1601 MiB with the file source and 1785 MiB with the URL
source, so the host's input strategy - not the module - is what decides browser memory. Prefer
a local `File` (or a prefetched buffer) and treat per-range synchronous XHR as the fallback
for remote archives.

That needs **no change to the core**. The cost is one Blob slice (or one HTTP Range
response) plus one ArrayBuffer per *window*, not per range - see below. The page stays
responsive because the work happens in the worker, and output is streamed back in chunks.

### Verify it

```sh
APK=/path/to/app.apk node js/browser-test.mjs      # CHROME= to point at another browser
```

That serves the module, the worker and the APK over `http://127.0.0.1`, drives the page in
headless Chrome, and compares each command's output with the native binary by SHA-256
(WebCrypto has no MD5). It runs **both source modes** - the local `File` and the HTTP URL -
and on Chrome 149 both are byte-identical to native: `manifest` (496,100 B), `classes`
(61,864,970 B), `findrefs field` (18,655,475 B) and a late-dex `getclass`, exit code 0.

`test.mjs` (Node) cannot cover this path itself - Node has no `FileReaderSync` - so it only
checks that `browser.mjs` imports cleanly. It also drives the module through a source that
rejects out-of-bounds ranges, short reads and undersized buffers.

### Hostile input

```sh
APK=/path/to/app.apk node js/mutation-test.mjs 40
```

The native binary has its own mutation harness; this is the wasm equivalent,
because there a panic is not a message but an unreachable trap that kills the instance.
Mutations (truncate, bit flips, zero runs, damaged central-directory entry, damaged
manifest payload) are applied in memory, deterministically (seed in the file), so a 343 MiB
sample costs no disk writes. Every command must finish with exit code 0/1/2, no trap and no
`PANIC:` line - which the panic hook in `rasc_run` is what makes visible.

Current result: 40 mutations x 5 commands = 200 runs, all clean.

A second corpus reaches the DEX parser, which the archive-level mutations cannot: a real APK
stores its DEX entries deflated, so `dex\n03` never appears in the archive and no byte flip
lands on a DEX header. `dex-corpus-test.mjs` unpacks `classes.dex` out of the sample, repacks
it as a *stored* entry and damages the fields the parser reads (header size, endian tag, map
offset, every table's size/offset, byte flips in the class table and string data, and five
truncation points).

```sh
APK=/path/to/app.apk node js/dex-corpus-test.mjs
```

Result: 27 mutations x 5 commands = 135 runs, 0 traps, 0 panics, 41 successes and 94
structured errors. It runs a control first - the unmutated archive must list classes and
decompile a class - so a broken fixture cannot make the check pass vacuously.

A third check builds the opposite attack: a deflate stream that expands to gigabytes.

```sh
node js/bomb-test.mjs 1.5      # GiB of expansion, from a 1.5 MB archive
```

That used to trap the wasm instance in ~0.25 s - a failed allocation with no message -
because the inflate buffer grows with whatever the stream really produces. All three
inflation paths (native libdeflate, wasm flate2, and the prefix decoder) now carry a per-entry
ceiling, 256 MiB by default: the same bomb reports `classes.dex inflates past the 256 MiB
entry limit` in 72-79 ms in wasm and 30-254 ms natively.

The ceiling is a host policy, not a constant: a host with a tighter budget sets it before a
command, and it applies per entry, so a small entry keeps working while a bomb-sized one is
refused.

```js
const rasc = await Rasc.load({ wasm, source, maxInflatedEntry: 64 << 20 });
rasc.setMaxInflatedEntry(0);   // back to the 256 MiB default
```

A fourth corpus targets the binary manifest decoder - vendored third-party code with a
documented panic history, and the one area neither of the other corpora reaches structurally:

```sh
APK=/path/to/app.apk node js/axml-corpus-test.mjs
```

24 structural mutations of the real manifest (chunk types and sizes, the string pool's
counts/flags/offsets, attribute counts and sizes, byte flips in the string data, five
truncations). Each case runs on the same archive through **both** hosts and has to agree on
the exit code, on the payload bytes when it succeeds and on the error message text when it
fails: 24/24 agree, no traps, no panics. That also asserts that the wasm build reports
byte-identical errors to the native CLI.

A fifth corpus attacks the container itself rather than the payload inside it: a ZIP reader
takes its entry list from the central directory but its data offset from the local header, so
an archive where the two disagree is where a crafted file can make a reader use the wrong
bytes or run past the end.

```sh
APK=/path/to/app.apk node js/zip-craft-corpus-test.mjs
```

15 hand-built archives over the sample's real `classes.dex` and `AndroidManifest.xml`
(local name shorter than the directory entry, local extra length past the end, unknown
compression method, stored sizes that disagree, an empty deflate stream, a ZIP64 placeholder,
a local offset into another entry, two entries sharing a header, a directory listing more
entries than exist, a directory offset pointing at a local header, a dex only in a
subdirectory, a name length past the end, the data-descriptor flag with zeroed local sizes,
duplicate names) x 4 commands = 60 runs, all agreeing with native - 29 of them error cases
whose message text is compared, not just the exit code.

## Interface the module relies on

Imports (all in the `env` module; a missing one fails the wasm link, not just the call):

| Import | Meaning |
| --- | --- |
| `rasc_host_archive_len() -> u32` | Total archive length, asked before anything is read. |
| `rasc_host_read(offset, len, ptr) -> u32` | Copy `len` archive bytes at `offset` into wasm memory at `ptr`; return how many arrived. |
| `rasc_host_write(ptr, len) -> u32` | Hand `len` payload bytes back to the host. |
| `rasc_host_arg(index, ptr, capacity) -> u32` | Write argument `index` at `ptr` (unterminated), returning its full length so the caller can grow and retry. |
| `rasc_host_now_ms() -> f64` | Host clock for `--debug` timings. `std::time::Instant::now()` panics on `wasm32-unknown-unknown`, so the clock has to come from outside. |

Export:

| Export | Meaning |
| --- | --- |
| `rasc_run(argc: u32) -> i32` | Runs one command with `argc` arguments (including `argv[0]`). Returns the CLI exit code. |
| `rasc_set_max_inflated_entry(bytes: u32) -> u32` | Sets the per-entry inflation ceiling and returns the previous value; `0` restores the 256 MiB default. |

A byte source must be **synchronous** and is
`{ size, read(offset, length, into) -> count }`. `test.mjs` drives the module through a
source that rejects out-of-bounds ranges, short reads and undersized buffers, which also
asserts what the core is allowed to ask for.

## Ranges, not a whole-archive copy

Measured on a 343 MiB APK: `classes` asked for 164 MiB in ~96,000 ranges - the DEX entries
it inflates plus the central directory, not the whole file.

Those ranges are cheap as wasm imports but expensive as backend calls: in a browser each one
was a `Blob.slice` + `FileReaderSync` round trip (~190 us), so a single command took 18-20 s.
`withReadAhead` serves a 4 MiB window at a time, cutting the backend calls to 5 for `manifest`
and 91 for `classes`:

| Command | Before | After |
| --- | ---: | ---: |
| `manifest` | 18,404 ms / 95,877 calls | **49 ms / 5 calls** |
| `classes` | 20,176 ms / 95,987 calls | **1,292 ms / 91 calls** |
| `findrefs field` | 21,069 ms | **2,575 ms / 192 calls** |
| `getclass` (late dex) | 19,321 ms | **1,110 ms / 285 calls** |

headless Chrome 149 on the same APK, with the output byte-identical to native throughout.
The window is part of the host, so the wasm side is unaffected.
