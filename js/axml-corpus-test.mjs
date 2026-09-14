#!/usr/bin/env node
/**
 * Hostile-input check for the binary AndroidManifest.xml decoder.
 *
 *   APK=/path/to/app.apk node js/axml-corpus-test.mjs
 *
 * The AXML decoder is vendored third-party code with a documented panic history (see
 * vendor/axmldecoder/PATCHES.md: upstream panics on string pools that use the extended
 * length forms, which real manifests do). Neither the archive-level nor the DEX-level corpus
 * can reach it: random byte flips almost never produce a structurally interesting manifest.
 *
 * So this corpus pulls AndroidManifest.xml out of the sample, repacks it as a stored entry in
 * a tiny archive, and damages the parts the decoder reads: chunk types and sizes, the string
 * pool's counts/flags/offsets, attribute counts and sizes, plus truncations and byte flips in
 * the string data.
 *
 * Each case runs through the wasm module and the native binary on the *same* archive, so the
 * two must agree on the exit code and the bytes - a mismatch is as much a finding as a trap.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { inflateRawSync } from 'node:zlib';
import { Rasc, sourceFromBytes } from './node.mjs';

const apk = process.env.APK;
if (!apk || !existsSync(apk)) {
  console.log('skip: set APK=/path/to/app.apk');
  process.exit(0);
}
const repo = new URL('..', import.meta.url).pathname;
const native = process.env.RASC_BIN ?? join(repo, 'target/release/rasc');
const wasmPath =
  process.env.RASC_WASM ?? join(repo, 'target/wasm32-unknown-unknown/release/rasc.wasm');
for (const [label, path] of [
  ['wasm', wasmPath],
  ['native', native],
]) {
  if (!existsSync(path)) {
    console.error(`missing ${label}: ${path}`);
    process.exit(1);
  }
}

const archive = readFileSync(apk);

/** Central-directory lookup + inflate, enough to pull one entry out of the sample. */
function findEntry(name) {
  const eocd = archive.lastIndexOf(Buffer.from('PK\x05\x06'));
  const count = archive.readUInt16LE(eocd + 10);
  let at = archive.readUInt32LE(eocd + 16);
  for (let index = 0; index < count; index += 1) {
    const nameLength = archive.readUInt16LE(at + 28);
    const extraLength = archive.readUInt16LE(at + 30);
    const commentLength = archive.readUInt16LE(at + 32);
    if (archive.toString('utf8', at + 46, at + 46 + nameLength) === name) {
      const method = archive.readUInt16LE(at + 10);
      const compressedSize = archive.readUInt32LE(at + 20);
      const local = archive.readUInt32LE(at + 42);
      const start =
        local + 30 + archive.readUInt16LE(local + 26) + archive.readUInt16LE(local + 28);
      const stored = archive.subarray(start, start + compressedSize);
      return method === 8 ? inflateRawSync(stored) : Buffer.from(stored);
    }
    at += 46 + nameLength + extraLength + commentLength;
  }
  return null;
}

/** Minimal ZIP writer for one stored entry (CRCs stay zero, as in the Rust fixtures). */
function buildZip(name, payload) {
  const nameBytes = Buffer.from(name);
  const local = Buffer.alloc(30 + nameBytes.length);
  local.write('PK\x03\x04', 0, 'latin1');
  local.writeUInt16LE(20, 4);
  local.writeUInt32LE(payload.length, 18);
  local.writeUInt32LE(payload.length, 22);
  local.writeUInt16LE(nameBytes.length, 26);
  nameBytes.copy(local, 30);

  const central = Buffer.alloc(46 + nameBytes.length);
  central.write('PK\x01\x02', 0, 'latin1');
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt32LE(payload.length, 20);
  central.writeUInt32LE(payload.length, 24);
  central.writeUInt16LE(nameBytes.length, 28);
  nameBytes.copy(central, 46);

  const eocd = Buffer.alloc(22);
  eocd.write('PK\x05\x06', 0, 'latin1');
  eocd.writeUInt16LE(1, 8);
  eocd.writeUInt16LE(1, 10);
  eocd.writeUInt32LE(central.length, 12);
  eocd.writeUInt32LE(local.length + payload.length, 16);
  return Buffer.concat([local, payload, central, eocd]);
}

const manifest = findEntry('AndroidManifest.xml');
if (!manifest) {
  console.error('no AndroidManifest.xml in the sample');
  process.exit(1);
}

/** First chunk header of `type`, scanning even offsets: type u16, header size u16, size u32. */
function findChunk(bytes, type) {
  for (let at = 0; at + 8 <= bytes.length; at += 2) {
    if (bytes.readUInt16LE(at) !== type) continue;
    const header = bytes.readUInt16LE(at + 2);
    if (header < 8 || header > 0x40) continue;
    return at;
  }
  return -1;
}

const U16 = (offset, value) => (bytes) => bytes.writeUInt16LE(value, offset);
const U32 = (offset, value) => (bytes) => bytes.writeUInt32LE(value, offset);
const stringPool = findChunk(manifest, 0x0001);
const element = findChunk(manifest, 0x0102);
const mutations = [
  ['file type=0x0009', U16(0, 0x0009)],
  ['header size=0', U16(2, 0)],
  ['file size=0', U32(4, 0)],
  ['file size=huge', U32(4, 0xfffffff0)],
];
if (stringPool >= 0) {
  mutations.push(
    ['string pool type=0x0009', U16(stringPool, 0x0009)],
    ['string pool header=0', U16(stringPool + 2, 0)],
    ['string pool size=0', U32(stringPool + 4, 0)],
    ['string pool size=huge', U32(stringPool + 4, 0xfffffff0)],
    ['string count=huge', U32(stringPool + 8, 0x0fffffff)],
    ['style count=huge', U32(stringPool + 12, 0x0fffffff)],
    ['flags: UTF-16 over UTF-8 data', U32(stringPool + 16, 0)],
    ['strings start=end', U32(stringPool + 20, manifest.length - 1)],
    ['strings start=huge', U32(stringPool + 20, 0xfffffff0)],
    ['styles start=huge', U32(stringPool + 24, 0xfffffff0)],
  );
}
if (element >= 0) {
  mutations.push(
    ['element header=0', U16(element + 2, 0)],
    ['attribute count=huge', U16(element + 0x1c, 0xffff)],
    ['attribute size=0', U16(element + 0x1a, 0)],
    ['attribute size=huge', U16(element + 0x1a, 0xffff)],
  );
}
mutations.push([
  'string data flips',
  (bytes) => {
    const from = stringPool >= 0 ? stringPool + 0x1c : 0x40;
    for (let at = from; at < Math.min(from + 256, bytes.length); at += 3) bytes[at] ^= 0x7f;
  },
]);
for (const cut of [8, 0x1c, 0x40, manifest.length >> 1, manifest.length - 1]) {
  mutations.push([`truncated@${cut}`, null, cut]);
}

const argv = ['manifest', 'mutated.apk'];
const dir = mkdtempSync(join(tmpdir(), 'rasc-axml-'));
const path = join(dir, 'mutated.apk');

/** Runs one archive through both hosts and returns what each did. */
async function runBoth(apkBytes) {
  writeFileSync(path, apkBytes);
  let wasm;
  try {
    const rasc = await Rasc.load({ wasm: readFileSync(wasmPath), source: sourceFromBytes(apkBytes) });
    const result = rasc.run(argv);
    rasc.close();
    wasm = {
      code: result.code,
      sha: createHash('sha256').update(result.output).digest('hex'),
      text: Buffer.from(result.output).toString('latin1').trim().slice(0, 120),
    };
  } catch (error) {
    wasm = { trap: String(error).slice(0, 120) };
  }
  let nativeResult;
  try {
    const out = execFileSync(native, [argv[0], path], { maxBuffer: 1 << 28 });
    nativeResult = { code: 0, sha: createHash('sha256').update(out).digest('hex'), text: '' };
  } catch (error) {
    const out = error.stdout ?? Buffer.alloc(0);
    nativeResult = {
      code: error.status ?? -1,
      sha: createHash('sha256').update(out).digest('hex'),
      // The native CLI reports errors on stderr; the wasm build has no stderr and sends
      // them through the host output instead, so the two are compared as text.
      text: (error.stderr ?? Buffer.alloc(0)).toString('latin1').trim(),
    };
  }
  return { wasm, native: nativeResult };
}

// Control: the unmutated manifest must decode, and both hosts must agree on the bytes.
const control = await runBoth(buildZip('AndroidManifest.xml', manifest));
console.log(
  `control: wasm exit=${control.wasm.code} sha=${control.wasm.sha.slice(0, 8)}; ` +
    `native exit=${control.native.code} sha=${control.native.sha.slice(0, 8)}`,
);
if (
  control.wasm.code !== 0 ||
  control.wasm.sha !== control.native.sha ||
  control.wasm.sha === createHash('sha256').update(Buffer.alloc(0)).digest('hex')
) {
  console.log('FAIL: the unmutated manifest control does not decode, so the corpus proves nothing');
  process.exit(1);
}

let traps = 0;
let panics = 0;
let disagreements = 0;
let clean = 0;
const failures = [];

for (const [label, mutate, cut] of mutations) {
  const bytes = Buffer.from(cut === undefined ? manifest : manifest.subarray(0, cut));
  if (mutate) mutate(bytes);
  const outcome = await runBoth(buildZip('AndroidManifest.xml', bytes));
  if (outcome.wasm.trap) {
    traps += 1;
    failures.push(`${label}: TRAP ${outcome.wasm.trap}`);
    continue;
  }
  if (outcome.wasm.text.includes('PANIC:')) {
    panics += 1;
    failures.push(`${label}: ${outcome.wasm.text}`);
    continue;
  }
  if (outcome.wasm.code !== outcome.native.code) {
    disagreements += 1;
    failures.push(`${label}: exit wasm=${outcome.wasm.code} native=${outcome.native.code}`);
    continue;
  }
  // On success the payload has to match byte for byte; on an error the message has to, which
  // is the same "Error: ..." string on both sides (only the stream differs).
  const agrees =
    outcome.wasm.code === 0
      ? outcome.wasm.sha === outcome.native.sha
      : outcome.wasm.text === outcome.native.text;
  if (!agrees) {
    disagreements += 1;
    failures.push(
      `${label}: wasm ${outcome.wasm.sha.slice(0, 8)} ${JSON.stringify(outcome.wasm.text.slice(0, 80))}` +
        ` vs native ${outcome.native.sha.slice(0, 8)} ${JSON.stringify(outcome.native.text.slice(0, 80))}`,
    );
    continue;
  }
  clean += 1;
}

for (const failure of failures.slice(0, 25)) console.log(`  ${failure}`);
console.log(
  `axml_mutations=${mutations.length} clean=${clean} traps=${traps} panics=${panics} ` +
    `disagreements=${disagreements}`,
);
console.log(traps + panics + disagreements === 0 ? `PASS (${clean} cases agree)` : 'FAIL');
process.exit(traps + panics + disagreements === 0 ? 0 : 1);
