/**
 * Node host for the rasc wasm module.
 *
 * Ranges are read with `fs.readSync`, so the host holds a file descriptor and never a copy
 * of the archive.
 */
import { closeSync, openSync, readFileSync, readSync, statSync } from 'node:fs';
import { Rasc, sourceFromBytes, withReadAhead } from './rasc.mjs';

/** Where `cargo build --release --target wasm32-unknown-unknown` leaves the module. */
export const DEFAULT_WASM = new URL(
  '../target/wasm32-unknown-unknown/release/rasc.wasm',
  import.meta.url,
);

/** Byte source over a file descriptor, with readahead. `close` frees the descriptor. */
export function sourceFromFile(path) {
  const fd = openSync(path, 'r');
  const size = statSync(path).size;
  return withReadAhead({
    size,
    read(offset, length, into) {
      return readSync(fd, into, 0, length, offset);
    },
    close() {
      closeSync(fd);
    },
  });
}

/**
 * Convenience loader: reads the module from disk and serves ranges from `apk`, or from
 * `apkData` when the caller already has the bytes.
 */
export async function loadFromFile({ wasmPath = DEFAULT_WASM, apk, apkData, onDiagnostic } = {}) {
  const wasm = readFileSync(wasmPath);
  const source = apkData ? sourceFromBytes(apkData) : sourceFromFile(apk);
  return Rasc.load({ wasm, source, onDiagnostic });
}

export { Rasc, sourceFromBytes, withReadAhead };
