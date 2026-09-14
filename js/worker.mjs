/**
 * Ready-made worker entry for the browser host.
 *
 * The page does:
 *
 *   const worker = new Worker('js/worker.mjs', { type: 'module' });
 *   worker.postMessage({ wasm, file });                     // wasm: ArrayBuffer, file: File
 *   worker.postMessage({ argv: ['manifest', 'app.apk'] });
 *
 * and receives `{ ready }`, `{ chunk }` and `{ code, error }` messages. Output is streamed
 * back in chunks, so a 61 MiB payload never has to exist twice.
 */
import { loadFromBlob, loadFromUrl } from './browser.mjs';

let rasc = null;

self.onmessage = async (event) => {
  try {
    if (event.data.file || event.data.url) {
      rasc = event.data.file
        ? await loadFromBlob({ wasm: event.data.wasm, file: event.data.file })
        : await loadFromUrl({ wasm: event.data.wasm, url: event.data.url });
      self.postMessage({ ready: true, size: rasc.archiveLength });
      return;
    }
    const started = performance.now();
    const { code, reads, bytes, fetches } = rasc.run(event.data.argv, {
      onOutput: (chunk) => self.postMessage({ chunk }, [chunk.buffer]),
    });
    self.postMessage({
      code,
      reads,
      bytes,
      fetches,
      ms: performance.now() - started,
      wasmMemoryBytes: rasc.wasmMemoryBytes,
    });
  } catch (error) {
    self.postMessage({ error: String(error) });
  }
};
