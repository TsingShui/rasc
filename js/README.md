# rasc under WASI

rasc's wasm product is a **program**, not a host module: `wasm32-wasip1`, with `argv`, an
environment, real file reads through the host's preopened directory, and stdout, stderr and
an exit code. So there is no SDK here and nothing to link against — a host that can start a
WASI program can run rasc, and the only thing it has to decide is what to mount.

| File | Role |
| --- | --- |
| `wasi-run.mjs` | The reference host: starts the module once with `argv`, env and preopens, and returns its exit code. Also usable directly as a command line. |
| `wasi-test.mjs` | The parity gate: the same command line through the native binary and through the module, compared byte for byte on both streams and on the exit code. |
| `torture-test.mjs` | Hostile input: a decompression bomb, deterministic mutations of a copy of the archive, and truncations — no trap, no panic, and the same answer as native. |

## Build and run

```sh
cargo build --release --target wasm32-wasip1
node --experimental-wasi-unstable-preview1 js/wasi-run.mjs manifest app.apk
APK=/path/to/app.apk node js/wasi-test.mjs
APK=/path/to/app.apk node js/torture-test.mjs 40
```

Node's WASI needs `--experimental-wasi-unstable-preview1`; a browser needs a WASI preview1
implementation and a filesystem of its own (an in-memory or OPFS-backed one), which is what
`browser_wasi_shim` gives — the same arrangement another engine in the family already uses.

## What a host owns

| Channel | Meaning |
| --- | --- |
| `argv` | The command line, unchanged: `rasc <command> [flags] <archive>`. There is no "label" argument — the archive path is a real path the guest opens. |
| the preopened directory | Where the archive and any `-o` output live. A host that holds the bytes in memory mounts them at a path instead of writing them. |
| `RASC_MAX_INFLATED_ENTRY` | The per-entry inflation ceiling in bytes, read once at startup. Host policy, not command-line policy, which is why it is not a flag. |
| stdout / stderr | Payload and diagnostics, split exactly as natively: redirecting stdout can never capture a `--debug` line or an error. |
| the exit code | `0` success, `1` a failed command, `2` a usage error — clap's own code, not a trap. |
| preemption | A WASI call cannot be interrupted, so a host that needs cancellation kills the instance. One run per instance is the cheap way to have that. |
