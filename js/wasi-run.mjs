/**
 * Runs the WASI build of rasc as a program: the reference host.
 *
 * This is the whole host contract now. The module is `wasm32-wasip1`, so it is a program
 * like any other - it gets `argv`, an environment, real file reads through the preopened
 * directory, and it writes stdout and stderr - and this file only has to hand those over.
 * Nothing here knows a command name, an output format or a byte source: a caller that can
 * start a WASI program can run rasc.
 *
 *   node --experimental-wasi-unstable-preview1 js/wasi-run.mjs manifest app.apk
 *
 * Every argument is passed through untouched (`argv[0]` is set to `rasc`, which is the name
 * the CLI calls itself), and the exit code is the program's own.
 *
 * `preopens` is the guest-to-host directory map: a browser hands the instance a virtual
 * filesystem over the user's file here instead. `RASC_WASM` overrides the module path.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { WASI } from 'node:wasi';

export const DEFAULT_WASM = fileURLToPath(
  new URL('../target/wasm32-wasip1/release/rasc.wasm', import.meta.url),
);

/** Starts the module once and returns its exit code. One call is one process lifetime. */
export async function runProgram({
  wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM,
  args,
  env = {},
  preopens = { '/': '/' },
  stdin,
  stdout,
  stderr,
} = {}) {
  const wasi = new WASI({
    version: 'preview1',
    args: ['rasc', ...args],
    env,
    preopens,
    // Return the code instead of ending this process: a host that runs more than one command
    // has to survive the first one.
    returnOnExit: true,
    stdin,
    stdout,
    stderr,
  });
  const wasm = readFileSync(wasmPath);
  const { instance } = await WebAssembly.instantiate(wasm, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });
  return wasi.start(instance);
}

// Run as a command: `argv[2..]` is the rasc command line, the environment passes through so
// the invocation in a gate script looks like the invocation of any other program.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const passthrough = {};
  for (const name of ['RASC_MAX_INFLATED_ENTRY']) {
    if (process.env[name] !== undefined) passthrough[name] = process.env[name];
  }
  process.exitCode = await runProgram({ args: process.argv.slice(2), env: passthrough });
}
