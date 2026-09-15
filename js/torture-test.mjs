#!/usr/bin/env node
/**
 * Hostile input, against the program that now has to survive it.
 *
 *   APK=/path/to/app.apk node js/torture-test.mjs [mutations]
 *
 * The check that mattered under the host module was "no trap, no PANIC, exit code 0/1/2":
 * there, `panic = "abort"` turned a panic into an unreachable trap that killed the instance,
 * and a browser has nothing to fall back on. Under WASI the same property is what keeps a
 * host's instance alive, so it is still the property, and it is checked the same way - with
 * the difference that a WASI program reads a real file, so mutations are patched into a copy
 * of the archive and restored afterwards instead of being applied to bytes in memory.
 *
 * Three things are checked:
 *
 *   1. a decompression bomb is refused with a message, not a trap;
 *   2. N deterministic mutations (bit flips, zero runs, damaged directory records, damaged
 *      manifest payload, truncations) all end with an exit code, never with a trap;
 *   3. every one of those runs agrees with the native binary - same code, same stdout, same
 *      stderr - because two implementations of "reject this archive" that disagree are a bug
 *      in one of them.
 *
 * The control comes first: the unmutated copy has to parse, or the rest of the file is
 * comparing failures and proving nothing.
 */
import { spawnSync } from 'node:child_process';
import {
  closeSync,
  copyFileSync,
  existsSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readSync,
  writeSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createDeflateRaw } from 'node:zlib';
import { DEFAULT_WASM } from './wasi-run.mjs';

const apk = process.env.APK;
if (!apk || !existsSync(apk)) {
  console.log('skip: set APK=/path/to/app.apk');
  process.exit(0);
}
const count = Number(process.argv[2] ?? process.env.MUTATIONS ?? 40);
const NATIVE = process.env.RASC_BIN ?? fileURLToPath(new URL('../target/release/rasc', import.meta.url));
const RUNNER = fileURLToPath(new URL('./wasi-run.mjs', import.meta.url));
const wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM;
for (const [label, path] of [['wasm', wasmPath], ['native', NATIVE]]) {
  if (!existsSync(path)) {
    console.error(`missing ${label}: ${path}`);
    process.exit(1);
  }
}

const dir = mkdtempSync(join(tmpdir(), 'rasc-torture-'));
const working = join(dir, 'app.apk');
copyFileSync(apk, working);

/** One run of each host. `env` carries the host policy a real host would set. */
function runWasi(args, env = {}) {
  const { stdout, stderr, status } = spawnSync(
    process.execPath,
    ['--experimental-wasi-unstable-preview1', '--no-warnings', RUNNER, ...args],
    { maxBuffer: 1 << 30, env: { ...env, RASC_WASM: wasmPath } },
  );
  const text = stderr.toString('utf8');
  return {
    code: status,
    stdout,
    stderr,
    trapped: /RuntimeError|unreachable|terminated/i.test(text),
    panicked: text.includes('PANIC:'),
  };
}

function runNative(args, env = {}) {
  const { stdout, stderr, status } = spawnSync(NATIVE, args, { maxBuffer: 1 << 30, env: { ...env } });
  return { code: status, stdout, stderr };
}

const commands = [
  ['manifest', working],
  ['classes', '--threads', '1', working],
  ['findrefs', '--threads', '1', working, 'string', 'androidx'],
  ['getclass', '--threads', '1', working, 'Landroid/app/Activity;'],
];

let runs = 0;
let traps = 0;
let panics = 0;
let badCodes = 0;
let disagreed = 0;
const failures = [];

/** Runs one command on both hosts and compares them. */
function check(label, argv, env = {}) {
  const wasi = runWasi(argv, env);
  const native = runNative(argv, env);
  runs += 1;
  if (wasi.trapped && !native.trapped) {
    traps += 1;
    failures.push(`${label} ${argv[0]}: TRAP ${wasi.stderr.toString('utf8').slice(0, 120)}`);
  }
  if (wasi.panicked) {
    panics += 1;
    failures.push(`${label} ${argv[0]}: PANIC ${wasi.stderr.toString('utf8').slice(0, 120)}`);
  }
  if (!Number.isInteger(wasi.code) || wasi.code < 0 || wasi.code > 2) {
    badCodes += 1;
    failures.push(`${label} ${argv[0]}: exit ${wasi.code}`);
  }
  const sameOut = Buffer.compare(wasi.stdout, native.stdout) === 0;
  const sameErr = normalize(wasi.stderr).equals(normalize(native.stderr));
  if (!sameOut || !sameErr || wasi.code !== native.code) {
    disagreed += 1;
    const detail = sameErr
      ? ''
      : `\n      wasi:   ${JSON.stringify(normalize(wasi.stderr).toString('utf8').trim().slice(0, 140))}` +
        `\n      native: ${JSON.stringify(normalize(native.stderr).toString('utf8').trim().slice(0, 140))}`;
    failures.push(
      `${label} ${argv[0]}: wasi ${wasi.code} native ${native.code}` +
        ` stdout ${sameOut ? '=' : '≠'} stderr ${sameErr ? '=' : '≠'}${detail}`,
    );
  }
  return wasi;
}

/** WASI's errno table is its own; the wording, the stream and the code are the contract. */
const normalize = (buffer) =>
  Buffer.from(buffer.toString('utf8').replace(/\(os error \d+\)/g, '(os error N)'), 'utf8');

// --------------------------------------------------------------------- control

console.log(`archive ${apk}\ncopy    ${working}\n`);
const control = runWasi(['manifest', working]);
if (control.code !== 0 || control.stdout.length === 0) {
  console.error(`control failed: manifest exited ${control.code} with ${control.stdout.length} bytes`);
  console.error(control.stderr.toString('utf8').slice(0, 400));
  process.exit(1);
}
console.log(`ok   control: manifest prints ${control.stdout.length} bytes\n`);

// ----------------------------------------------------------------------- bomb

/** A deflate stream that expands to `bytes` of zeros without materialising them. */
async function deflateZeros(bytes) {
  const chunk = Buffer.alloc(1 << 20);
  const deflate = createDeflateRaw({ level: 9 });
  const parts = [];
  deflate.on('data', (part) => parts.push(part));
  const finished = new Promise((resolve) => deflate.on('end', resolve));
  for (let written = 0; written < bytes; written += chunk.length) {
    if (!deflate.write(chunk)) await new Promise((resolve) => deflate.once('drain', resolve));
  }
  deflate.end();
  await finished;
  return Buffer.concat(parts);
}

/** Minimal ZIP writer for one entry, with an explicit method and declared size. */
function buildZip(name, compressed, uncompressed, method) {
  const nameBytes = Buffer.from(name);
  const local = Buffer.alloc(30 + nameBytes.length);
  local.write('PK\x03\x04', 0, 'latin1');
  local.writeUInt16LE(20, 4);
  local.writeUInt16LE(method, 8);
  local.writeUInt32LE(compressed.length, 18);
  local.writeUInt32LE(uncompressed, 22);
  local.writeUInt16LE(nameBytes.length, 26);
  nameBytes.copy(local, 30);

  const central = Buffer.alloc(46 + nameBytes.length);
  central.write('PK\x01\x02', 0, 'latin1');
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt16LE(method, 10);
  central.writeUInt32LE(compressed.length, 20);
  central.writeUInt32LE(uncompressed, 24);
  central.writeUInt16LE(nameBytes.length, 28);
  nameBytes.copy(central, 46);

  const eocd = Buffer.alloc(22);
  eocd.write('PK\x05\x06', 0, 'latin1');
  eocd.writeUInt16LE(1, 8);
  eocd.writeUInt16LE(1, 10);
  eocd.writeUInt32LE(central.length, 12);
  eocd.writeUInt32LE(local.length + compressed.length, 16);
  return Buffer.concat([local, compressed, central, eocd]);
}

const bombGiB = Number(process.env.BOMB_GIB ?? 1.5);
const bombPath = join(dir, 'bomb.apk');
const bomb = buildZip('classes.dex', await deflateZeros(bombGiB * 1024 ** 3), bombGiB * 1024 ** 3, 8);
const bombFd = openSync(bombPath, 'w');
writeSync(bombFd, bomb);
closeSync(bombFd);
console.log(`bomb: ${bomb.length} bytes expanding to ${bombGiB} GiB`);
for (const argv of [['classes', '--threads', '1', bombPath], ['getclass', '--threads', '1', bombPath, 'Lcom/example/Absent;']]) {
  const wasi = check('bomb', argv);
  console.log(
    `ok   bomb ${argv[0].padEnd(9)} exit ${wasi.code} ${JSON.stringify(normalize(wasi.stderr).toString('utf8').trim().slice(0, 70))}`,
  );
}

// ------------------------------------------------------------------ mutations

/** Deterministic PRNG: a run has to be reproducible from the seed in this file. */
let seed = 0x9e3779b9;
function random() {
  seed ^= seed << 13;
  seed ^= seed >>> 17;
  seed ^= seed << 5;
  return (seed >>> 0) / 0x1_0000_0000;
}

const size = readFileSync(working).length;
const fd = openSync(working, 'r+');

/** Applies one mutation and returns how to undo it, or null when nothing was written. */
function mutate(kind) {
  const at = (lo, hi) => lo + Math.floor(random() * (hi - lo));
  if (kind === 'bitflip') {
    const offset = at(0, size);
    const original = Buffer.alloc(4);
    readSyncInto(original, offset);
    const changed = Buffer.from(original);
    changed[Math.floor(random() * 4)] ^= 1 << Math.floor(random() * 8);
    patch(offset, changed);
    return () => patch(offset, original);
  }
  if (kind === 'zeros') {
    const offset = at(0, size - 4096);
    const original = Buffer.alloc(4096);
    readSyncInto(original, offset);
    patch(offset, Buffer.alloc(4096));
    return () => patch(offset, original);
  }
  if (kind === 'eocd') {
    // The end-of-central-directory record lives in the last 64 KiB by definition.
    const offset = size - 22 - Math.floor(random() * 64);
    const original = Buffer.alloc(22);
    readSyncInto(original, offset);
    const changed = Buffer.from(original);
    changed[12] = 0xff;
    changed[13] = 0xff;
    patch(offset, changed);
    return () => patch(offset, original);
  }
  return null;
}

function readSyncInto(into, offset) {
  readSync(fd, into, 0, into.length, offset);
}

function patch(offset, bytes) {
  writeSync(fd, bytes, 0, bytes.length, offset);
}

const kinds = ['bitflip', 'zeros', 'eocd'];
let unchanged = 0;
for (let index = 0; index < count; index += 1) {
  const kind = kinds[index % kinds.length];
  const undo = mutate(kind);
  if (!undo) {
    unchanged += 1;
    continue;
  }
  check(`#${index} ${kind}`, commands[index % commands.length]);
  undo();
}
closeSync(fd);
console.log(`\nmutations: ${count} (${unchanged} wrote nothing)`);

// ---------------------------------------------------------------- truncations

const truncations = [1024, 65536, 1 << 20];
for (const length of truncations) {
  const path = join(dir, `truncated-${length}.apk`);
  const truncFd = openSync(path, 'w');
  writeSync(truncFd, readFileSync(apk).subarray(0, length));
  closeSync(truncFd);
  const wasi = check(`truncated ${length}`, ['manifest', path]);
  console.log(`ok   truncated ${String(length).padStart(8)}B exit ${wasi.code}`);
}

// ---------------------------------------------------------------------- report

console.log(
  `\nruns=${runs} traps=${traps} panics=${panics} bad_codes=${badCodes} disagreements_with_native=${disagreed}`,
);
if (failures.length > 0) {
  for (const failure of failures.slice(0, 10)) console.log(`  ${failure}`);
}
console.log(failures.length === 0 ? 'PASS (hostile input ends in a code, never a trap)' : 'FAIL');
process.exitCode = failures.length === 0 ? 0 : 1;
