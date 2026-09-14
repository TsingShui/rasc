/**
 * Browser host for the rasc wasm module.
 *
 * The module needs byte ranges **synchronously**, and on the web only a worker can read a
 * `Blob` synchronously: `FileReaderSync.readAsArrayBuffer()` returns an `ArrayBuffer`
 * directly (see https://developer.mozilla.org/en-US/docs/Web/API/FileReaderSync).
 * So the module runs in a module worker, the page posts the user's `File` to it once, and
 * every range the wasm side asks for is served from `blob.slice(...)` without awaiting.
 *
 * Nothing here needs a core change; the only cost is one Blob slice plus one ArrayBuffer
 * per range. A remote URL instead of a local file would need prefetching, because fetching
 * cannot be made synchronous.
 *
 * This file cannot run in Node (no FileReaderSync), so it is exercised by the worker entry
 * in worker.mjs rather than by the Node test suite.
 */
import { Rasc, sourceFromBytes, withReadAhead } from './rasc.mjs';

/**
 * Synchronous request helper. `fetch` returns a promise and a wasm import cannot await, so
 * a worker-scoped synchronous `XMLHttpRequest` is the only way to answer a range request
 * from the network. Deprecated, but still supported in workers.
 */
function syncRequest(method, url, range) {
  const request = new XMLHttpRequest();
  request.open(method, url, false);
  request.responseType = 'arraybuffer';
  if (range) request.setRequestHeader('Range', range);
  request.send(null);
  return request;
}

/**
 * Synchronous range source over HTTP.
 *
 * The server should support Range so a window is one response; if it ignores the header the
 * whole archive arrives in one 200 response and is kept in memory instead (correct, but the
 * host then holds a copy of the archive).
 */
export function sourceFromUrl(url) {
  const head = syncRequest('HEAD', url, null);
  const size = Number(head.getResponseHeader('Content-Length') ?? 0);
  if (!size) throw new Error(`no Content-Length from ${url}`);
  let whole = null;

  return withReadAhead({
    size,
    read(offset, length, into) {
      if (whole) {
        into.set(whole.subarray(offset, offset + length));
        return length;
      }
      const response = syncRequest('GET', url, `bytes=${offset}-${offset + length - 1}`);
      const body = new Uint8Array(response.response);
      if (body.length === size) {
        // The server ignored Range and sent everything: keep it for the rest of the run.
        whole = body;
      }
      const wanted = Math.min(length, size - offset);
      into.set(body.subarray(0, wanted));
      return wanted;
    },
  });
}

/**
 * Synchronous range source over a File/Blob. Must be constructed inside a worker.
 *
 * One `blob.slice` plus one `FileReaderSync` call per window rather than per range: a raw
 * Blob read is a synchronous trip to the file backend and costs ~190 us, which is 18 s for
 * a single command's worth of ranges.
 */
export function sourceFromBlob(blob) {
  const reader = new FileReaderSync();
  return withReadAhead({
    size: blob.size,
    read(offset, length, into) {
      const part = new Uint8Array(reader.readAsArrayBuffer(blob.slice(offset, offset + length)));
      into.set(part);
      return part.length;
    },
  });
}

/** Loads the module against a local file, inside a worker. */
export async function loadFromBlob({ wasm, file }) {
  return Rasc.load({ wasm, source: sourceFromBlob(file) });
}

/** Loads the module against a remote archive, inside a worker. */
export async function loadFromUrl({ wasm, url }) {
  return Rasc.load({ wasm, source: sourceFromUrl(url) });
}

export { Rasc, sourceFromBytes };
