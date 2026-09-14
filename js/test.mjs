#!/usr/bin/env node
/**
 * Verifies the wasm module through the Node host against the native binary.
 *
 *   APK=/path/to/app.apk node js/test.mjs
 *
 * Needs both artefacts: `cargo build --release` and
 * `cargo build --release --target wasm32-unknown-unknown`. Skipped (exit 0) when the APK is
 * not provided, so it can sit in a repository whose sample is not checked in.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, existsSync, openSync, readFileSync, readSync, statSync } from 'node:fs';
import { DEFAULT_WASM, loadFromFile, Rasc, withReadAhead } from './node.mjs';

const apk = process.env.APK;
if (!apk || !existsSync(apk)) {
  console.log('skip: set APK=/path/to/app.apk to run the wasm SDK check');
  process.exit(0);
}

const native = process.env.RASC_BIN ?? 'target/release/rasc';
const wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM;
const hasNative = existsSync(native);

let checks = 0;
let failures = 0;

/** Asserts a plain condition. */
function assert(label, condition, detail = '') {
  checks += 1;
  if (!condition) failures += 1;
  console.log(`${condition ? 'ok  ' : 'FAIL'} ${label}${detail ? `  ${detail}` : ''}`);
}

const digest = (bytes) => createHash('md5').update(bytes).digest('hex');

/** Runs one command through both hosts and compares exit code and output bytes. */
function compare(rasc, label, args) {
  checks += 1;
  const host = rasc.run(args);
  const hostDigest = digest(host.output);
  let nativeDigest = null;
  let nativeCode = null;
  if (hasNative) {
    try {
      nativeDigest = digest(execFileSync(native, args, { maxBuffer: 1 << 30 }));
      nativeCode = 0;
    } catch (error) {
      nativeDigest = digest(error.stdout ?? Buffer.alloc(0));
      nativeCode = error.status ?? 1;
    }
  }
  const ok = nativeDigest === null || (hostDigest === nativeDigest && host.code === nativeCode);
  if (!ok) failures += 1;
  console.log(
    `${ok ? 'ok  ' : 'DIFF'} ${label.padEnd(24)} host=${host.code}/${hostDigest.slice(0, 8)}` +
      ` native=${nativeCode}/${nativeDigest?.slice(0, 8)} bytes=${host.output.length}` +
      ` reads=${host.reads} backend=${host.fetches}`,
  );
  return host.output;
}

const rasc = await loadFromFile({ wasmPath, apk });
console.log(`wasm: ${wasmPath}\narchive: ${rasc.archiveLength} bytes   native: ${native}`);

compare(rasc, 'manifest', ['manifest', apk]);
const classes = compare(rasc, 'classes', ['classes', '--threads', '1', apk]);
compare(rasc, 'findrefs field', ['findrefs', '--threads', '1', apk, 'field', 'INSTANCE']);

// A class in a late dex exercises lookup, prefix inflate and decompile, not an early hit.
// The dex name comes from the archive's own index rather than a hardcoded one, so the check
// works with any APK: `classes.dex` is 1, `classes2.dex` is 2, and so on.
function latestDexClass(index) {
  const rows = new TextDecoder().decode(index).split('\n').filter(Boolean);
  const rank = (name) => Number(/(\d+)\.dex$/.exec(name)?.[1] ?? 1);
  const names = [...new Set(rows.map((row) => row.split(' | ')[0]))].sort((a, b) => rank(a) - rank(b));
  const last = names[names.length - 1];
  return rows.find((row) => row.startsWith(`${last} | `))?.split(' | ')[1];
}

const late = latestDexClass(classes);
if (late) {
  compare(rasc, `getclass ${late}`, ['getclass', '--threads', '1', apk, late]);
} else {
  assert('late-dex class found', false, 'the class index has no usable rows');
}
rasc.close();

// The same module driven from an in-memory buffer: the shape a browser takes once it has
// the bytes.
const buffered = await Rasc.load({ wasm: readFileSync(wasmPath), source: bufferSource(apk) });
compare(buffered, 'manifest (buffer src)', ['manifest', apk]);
buffered.close();

// The host contract itself: a synchronous reader that refuses out-of-bounds ranges, short
// reads and undersized buffers. This is also an assertion about the core - it must never
// ask for a range outside the archive or rely on a short read.
const guarded = guardedSource(apk);
const strict = await Rasc.load({ wasm: readFileSync(wasmPath), source: guarded });
compare(strict, 'manifest (guarded src)', ['manifest', apk]);
strict.close();
assert('guarded source requests', guarded.requests() > 0, `${guarded.requests()} ranges`);
assert('guarded source stayed in bounds', guarded.violations() === 0, `${guarded.violations()} violations`);

// The readahead window is what keeps a browser responsive: it turns tens of thousands of
// backend calls into a few. Wrapping the same validating source proves the windows are
// still in bounds and still produce the same bytes.
const windowed = guardedSource(apk);
const readahead = await Rasc.load({
  wasm: readFileSync(wasmPath),
  source: withReadAhead(windowed),
});
compare(readahead, 'manifest (readahead)', ['manifest', apk]);
readahead.close();
assert('readahead shrinks backend calls', windowed.requests() < 32, `${windowed.requests()} calls`);
assert('readahead stayed in bounds', windowed.violations() === 0, `${windowed.violations()} violations`);

// browser.mjs must at least load outside a browser: its FileReaderSync use is inside a
// function, so importing it is safe everywhere.
const browser = await import('./browser.mjs');
assert('browser host module loads', typeof browser.sourceFromBlob === 'function');

console.log(failures === 0 ? `PASS (${checks} checks)` : `FAIL (${failures} of ${checks})`);
process.exit(failures === 0 ? 0 : 1);

/** Source over a file descriptor, used where the buffer case is being demonstrated. */
function bufferSource(path) {
  const data = readFileSync(path);
  return {
    size: data.length,
    read(offset, length, into) {
      const slice = data.subarray(offset, Math.min(offset + length, data.length));
      into.set(slice);
      return slice.length;
    },
  };
}

/** Source that validates the host contract and counts what the core asked for. */
function guardedSource(path) {
  const fd = openSync(path, 'r');
  const size = statSync(path).size;
  let requests = 0;
  let violations = 0;
  const note = (message) => {
    violations += 1;
    console.error(`  source violation: ${message}`);
  };
  return {
    size,
    requests: () => requests,
    violations: () => violations,
    read(offset, length, into) {
      requests += 1;
      if (offset + length > size) note(`range ${offset}+${length} past ${size}`);
      if (into.length < length) note(`buffer ${into.length} smaller than ${length}`);
      const read = readSync(fd, into, 0, length, offset);
      if (read !== length) note(`short read ${read} of ${length} at ${offset}`);
      return read;
    },
    close: () => closeSync(fd),
  };
}
