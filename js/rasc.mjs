/**
 * Host side of the rasc WebAssembly module - the shared part.
 *
 * The `wasm32-unknown-unknown` build has no filesystem of its own. It asks the host for the
 * exact byte ranges it needs and hands its output back, so a host is a byte source plus the
 * five imports below.
 *
 * Imports the module requires (`env` module):
 *
 *   rasc_host_archive_len() -> u32                total archive length
 *   rasc_host_read(offset, len, ptr) -> u32       copy `len` archive bytes to wasm memory
 *   rasc_host_write(ptr, len) -> u32              hand `len` output bytes back
 *   rasc_host_arg(index, ptr, capacity) -> u32    write argument `index`, return its full
 *                                                 length (grow and retry when short)
 *   rasc_host_progress(done, total)               entries walked, out of the walk's total
 *   rasc_host_now_ms() -> f64                     host clock, for `--debug` timings
 *
 * Export the host calls:
 *
 *   rasc_run(argc) -> i32                         exit code, as the CLI would report it
 *
 * A byte source is `{ size, read(offset, length, into) -> count }` and must be
 * **synchronous**: the wasm side cannot await. See node.mjs (file descriptor) and
 * browser.mjs (a worker's FileReaderSync over a Blob).
 *
 * This file deliberately imports nothing: it has to load in a browser too.
 */

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Copies and concatenates the chunks a run produced. */
function concat(chunks, length) {
  const out = new Uint8Array(length);
  let at = 0;
  for (const chunk of chunks) {
    out.set(chunk, at);
    at += chunk.length;
  }
  return out;
}

export class Rasc {
  /**
   * Instantiates the module against a byte source.
   *
   * @param {{wasm: Uint8Array|ArrayBuffer, source: {size: number, read: Function}}} options
   */
  static async load({ wasm, source, maxInflatedEntry, onDiagnostic }) {
    if (typeof source?.read !== 'function' || !Number.isFinite(source.size)) {
      throw new TypeError('source must be { size, read(offset, length, into) }');
    }    const state = {
      memory: undefined,
      argv: [],
      chunks: [],
      length: 0,
      sink: undefined,
      progress: undefined,
      reads: 0,
      bytes: 0,
      diagnostic: undefined,
    };

    const imports = {
      env: {
        rasc_host_now_ms: () => performance.now(),
        rasc_host_archive_len: () => source.size,
        rasc_host_read: (offset, length, pointer) => {
          // Re-read `memory.buffer` on every call: growing the memory detaches it.
          const view = new Uint8Array(state.memory.buffer, pointer, length);
          state.reads += 1;
          state.bytes += length;
          return source.read(offset, length, view);
        },
        rasc_host_write: (pointer, length) => {
          const bytes = new Uint8Array(state.memory.buffer, pointer, length).slice();
          state.length += length;
          if (state.sink) {
            state.sink(bytes);
          } else {
            state.chunks.push(bytes);
          }
          return length;
        },
        rasc_host_write_err: (pointer, length) => {
          // Diagnostics, which natively go to stderr; a host without one takes them here.
          const bytes = new Uint8Array(state.memory.buffer, pointer, length).slice();
          if (state.diagnostic) {
            state.diagnostic(bytes);
          } else {
            console.error(decoder.decode(bytes).trimEnd());
          }
          return length;
        },
        rasc_host_progress: (done, total) => {
          // Only the host build's walk is ordered, and it is the only one that asks; a
          // host that installed no handler simply does not hear about it.
          state.progress?.(done, total);
        },
        rasc_host_arg: (index, pointer, capacity) => {
          const value = encoder.encode(state.argv[index] ?? '');
          const written = Math.min(value.length, capacity);
          new Uint8Array(state.memory.buffer, pointer, written).set(value.subarray(0, written));
          return value.length;
        },
      },
    };

    const { instance } = await WebAssembly.instantiate(wasm, imports);
    state.memory = instance.exports.memory;
    if (typeof instance.exports.rasc_run !== 'function') {
      throw new Error('module does not export rasc_run; build with --target wasm32-unknown-unknown');
    }
    const rasc = new Rasc(instance, state, source);
    if (maxInflatedEntry !== undefined) {
      rasc.setMaxInflatedEntry(maxInflatedEntry);
    }
    if (onDiagnostic !== undefined) {
      state.diagnostic = onDiagnostic;
    }
    return rasc;
  }

  #instance;
  #state;
  #source;

  constructor(instance, state, source) {
    this.#instance = instance;
    this.#state = state;
    this.#source = source;
  }

  /** Size of the archive the host is serving. */
  get archiveLength() {
    return this.#source.size;
  }

  /**
   * Current wasm linear memory size in bytes.
   *
   * This is the number that matters for a host budget: wasm memory never shrinks, so it is
   * a high-water mark for as long as the instance lives.
   */
  get wasmMemoryBytes() {
    return this.#state.memory.buffer.byteLength;
  }

  /**
   * Lowers the per-entry inflation ceiling (bytes); 0 restores the built-in default. Use it
   * before a command when the instance has a tighter memory budget than the default, e.g. a
   * browser tab. Returns the ceiling that was in force.
   */
  setMaxInflatedEntry(bytes) {
    const setter = this.#instance.exports.rasc_set_max_inflated_entry;
    if (typeof setter !== 'function') {
      throw new Error('module does not export rasc_set_max_inflated_entry');
    }
    return setter(bytes >>> 0);
  }

  /** Releases the byte source when it holds a resource (a file descriptor, say). */
  close() {
    this.#source.close?.();
  }

  /**
   * Runs one command.
   *
   * `args` are the CLI arguments after the program name, e.g.
   * `['findrefs', '--threads', '1', 'app.apk', 'field', 'INSTANCE']`. The archive argument
   * is only a label: the bytes always come from the host's source.
   *
   * `onOutput` receives `Uint8Array`s as they are produced, which keeps host memory bounded
   * on a large payload. Without it the payload is collected and returned.
   *
   * `onProgress(done, total)` hears how many entries of a walk have finished, when the
   * walk can say: it is a real fraction over a total the engine knew before it started,
   * never an estimate.
   *
   * **`output` is empty when `onOutput` is given**, because the payload went to the sink.
   * It used to be a zero-filled buffer of the payload's length - the right size and none of
   * the bytes - which is the worst thing a field can be: plausible. A host that streams
   * owns the concatenation, and this one is deliberately not a second copy of it.
   *
   * The result also reports how many range requests the core made (`reads`) and how many
   * bytes those asked for (`bytes`), which is the host-side cost of a command.
   */
  run(args, { onOutput, onProgress } = {}) {
    this.#state.argv = ['rasc', ...args];
    this.#state.chunks = [];
    this.#state.length = 0;
    this.#state.reads = 0;
    this.#state.bytes = 0;
    this.#state.sink = onOutput;
    this.#state.progress = onProgress;
    try {
      const code = this.#instance.exports.rasc_run(this.#state.argv.length);
      return {
        code,
        output: onOutput === undefined ? concat(this.#state.chunks, this.#state.length) : new Uint8Array(0),
        reads: this.#state.reads,
        bytes: this.#state.bytes,
        // How many calls actually reached the backend, when the source reports it.
        fetches: this.#source.fetches ?? this.#state.reads,
      };
    } finally {
      this.#state.sink = undefined;
      this.#state.progress = undefined;
    }
  }
}

/** Byte source over a buffer the host already holds. */
export function sourceFromBytes(data) {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  return {
    size: bytes.length,
    read(offset, length, into) {
      const slice = bytes.subarray(offset, Math.min(offset + length, bytes.length));
      into.set(slice);
      return slice.length;
    },
  };
}

/**
 * Adds a readahead window over a synchronous source.
 *
 * The parser asks for the archive in tens of thousands of tiny ranges (the central
 * directory alone is a 46-byte header plus a name per entry). Each of those is a wasm
 * import, which is cheap, but with a Blob or a file behind it each one is also a real
 * backend call - measured at roughly 190 us in a browser, i.e. 18 s for one command.
 * Serving a window at a time collapses those into a few hundred calls.
 *
 * `fetches` counts the calls that actually reached the source, which is the number that
 * matters when the backend is expensive.
 */
export function withReadAhead(source, window = 4 << 20) {
  let start = -1;
  let end = -1;
  let cache = null;

  const wrapped = {
    size: source.size,
    fetches: 0,
    read(offset, length, into) {
      if (offset < start || offset + length > end) {
        // The window has to cover the whole request, which can straddle a window boundary;
        // a request bigger than the window is served on its own rather than refused.
        const aligned = Math.max(0, Math.floor(offset / window) * window);
        const needed = offset + length - aligned;
        const want = Math.min(Math.max(window, needed), source.size - aligned);
        const buffer = new Uint8Array(want);
        const got = source.read(aligned, want, buffer);
        cache = buffer.subarray(0, got);
        start = aligned;
        end = aligned + got;
        wrapped.fetches += 1;
      }
      const begin = offset - start;
      if (begin + length > cache.length) {
        throw new Error(`source returned ${cache.length} bytes from ${start}, needed ${begin + length}`);
      }
      into.set(cache.subarray(begin, begin + length));
      return length;
    },
    close: () => source.close?.(),
  };
  return wrapped;
}
