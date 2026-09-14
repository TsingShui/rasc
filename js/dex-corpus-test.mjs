#!/usr/bin/env node
/**
 * Hostile-input check for the DEX parser, which the archive-level mutations cannot reach.
 *
 *   APK=/path/to/app.apk node js/dex-corpus-test.mjs
 *
 * `js/mutation-test.mjs` corrupts the archive itself, but a real APK stores its DEX entries
 * deflated, so no byte flip ever lands on a DEX header or table (measured: `dex\n03` does
 * not appear anywhere in the 343 MiB sample). This script closes that gap: it pulls one DEX
 * out of the sample, repacks it into a small archive as a *stored* entry, and then damages
 * the fields the parser actually reads - header size, endian tag, map offset and every
 * table's size/offset - plus truncated variants and byte flips inside the class table.
 *
 * Every command must finish with exit code 0/1/2, no trap and no `PANIC:` line: in wasm a
 * panic is an unreachable trap that kills the instance rather than a message.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
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

/** Central-directory lookup, enough to pull one entry out of the sample. */
function findEntry(name) {
  const eocd = archive.lastIndexOf(Buffer.from('PK\x05\x06'));
  const count = archive.readUInt16LE(eocd + 10);
  let at = archive.readUInt32LE(eocd + 16);
  for (let index = 0; index < count; index += 1) {
    const nameLength = archive.readUInt16LE(at + 28);
    const extraLength = archive.readUInt16LE(at + 30);
    const commentLength = archive.readUInt16LE(at + 32);
    const found = archive.toString('utf8', at + 46, at + 46 + nameLength);
    if (found === name) {
      const method = archive.readUInt16LE(at + 10);
      const compressedSize = archive.readUInt32LE(at + 20);
      const local = archive.readUInt32LE(at + 42);
      const start =
        local + 30 + archive.readUInt16LE(local + 26) + archive.readUInt16LE(local + 28);
      const stored = archive.subarray(start, start + compressedSize);
      // 8 = deflate, 0 = stored
      return method === 8 ? inflateRawSync(stored) : Buffer.from(stored);
    }
    at += 46 + nameLength + extraLength + commentLength;
  }
  return null;
}

/** Minimal ZIP writer: one stored entry. CRCs stay zero, as in the Rust test fixtures. */
function buildZip(name, payload) {
  const nameBytes = Buffer.from(name);
  const local = Buffer.alloc(30 + nameBytes.length);
  local.write('PK\x03\x04', 0, 'latin1');
  local.writeUInt16LE(20, 4);
  local.writeUInt16LE(0, 8); // stored
  local.writeUInt32LE(payload.length, 18);
  local.writeUInt32LE(payload.length, 22);
  local.writeUInt16LE(nameBytes.length, 26);
  nameBytes.copy(local, 30);

  const central = Buffer.alloc(46 + nameBytes.length);
  central.write('PK\x01\x02', 0, 'latin1');
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt16LE(0, 10); // stored
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

const dex = findEntry('classes.dex');
if (!dex) {
  console.error('no classes.dex in the sample');
  process.exit(1);
}

// A class that lives in this DEX, so the decompiler is reached when a mutation leaves the
// class table intact enough to still resolve it.
const early = execFileSync(native, ['classes', '--threads', '8', apk], { maxBuffer: 1 << 30 })
  .toString('utf8')
  .split('\n')
  .find((line) => line.startsWith('classes.dex | '))
  ?.split(' | ')[1];

const U32 = (offset, value) => (bytes) => bytes.writeUInt32LE(value, offset);
const MUTATIONS = [
  ['magic', (bytes) => bytes.write('dex\n999', 0, 'latin1')],
  ['checksum+signature zeroed', (bytes) => bytes.fill(0, 8, 0x20)],
  ['file_size=0', U32(0x20, 0)],
  ['file_size=huge', U32(0x20, 0x7fffffff)],
  ['header_size=0', U32(0x24, 0)],
  ['endian_tag reversed', U32(0x28, 0x12345678)],
  ['link_size=huge', U32(0x2c, 0x40000000)],
  ['map_off=end', (bytes) => bytes.writeUInt32LE(bytes.length - 1, 0x34)],
  ['map_off=huge', U32(0x34, 0xfffffff0)],
  ['string_ids_size=huge', U32(0x38, 0x40000000)],
  ['string_ids_off=end', (bytes) => bytes.writeUInt32LE(bytes.length - 4, 0x3c)],
  ['type_ids_size=huge', U32(0x40, 0x40000000)],
  ['type_ids_off=0', U32(0x44, 0)],
  ['proto_ids_size=huge', U32(0x48, 0x40000000)],
  ['field_ids_size=huge', U32(0x50, 0x40000000)],
  ['method_ids_size=huge', U32(0x58, 0x40000000)],
  ['method_ids_off=huge', U32(0x5c, 0xfffffff0)],
  ['class_defs_size=huge', U32(0x60, 0x40000000)],
  ['class_defs_off=end', (bytes) => bytes.writeUInt32LE(bytes.length - 8, 0x64)],
  ['data_off=huge', U32(0x6c, 0xfffffff0)],
  [
    'class_def flips',
    (bytes) => {
      const off = bytes.readUInt32LE(0x64);
      const end = Math.min(off + 32 * 64, bytes.length - 1);
      for (let at = off; at < end; at += 4) bytes[at] ^= 0x5a;
    },
  ],
  [
    'string_data flips',
    (bytes) => {
      const off = bytes.readUInt32LE(0x3c);
      const end = Math.min(off + 4096, bytes.length - 1);
      for (let at = off; at < end; at += 3) bytes[at] ^= 0xff;
    },
  ],
];
for (const cut of [0x70, 0x200, 0x1000, dex.length >> 1, dex.length - 1]) {
  MUTATIONS.push([`truncated@${cut}`, null, cut]);
}

const commands = [
  ['manifest', 'mutated.apk'],
  ['classes', '--threads', '1', 'mutated.apk'],
  ['findrefs', '--threads', '1', 'mutated.apk', 'string', 'androidx'],
  ['findrefs', '--threads', '1', 'mutated.apk', 'type', 'Ljava/lang/String;'],
  ...(early ? [['getclass', '--threads', '1', 'mutated.apk', early]] : []),
];

let runs = 0;
let traps = 0;
let panics = 0;
let badCodes = 0;
let unchanged = 0;
let succeeded = 0;
const failures = [];

// Control first: an unmutated stored-DEX archive has to work. Without this, a broken ZIP
// builder would make every command fail instantly and the whole check pass vacuously.
{
  const control = buildZip('classes.dex', dex);
  const rasc = await Rasc.load({ wasm: readFileSync(wasmPath), source: sourceFromBytes(control) });
  const listed = rasc.run(['classes', '--threads', '1', 'control.apk']);
  const rows = listed.output.length;
  const hit = early
    ? rasc.run(['getclass', '--threads', '1', 'control.apk', early])
    : { code: 0, output: Buffer.alloc(0) };
  rasc.close();
  console.log(
    `control: classes exit=${listed.code} bytes=${rows}; getclass ${early} exit=${hit.code} bytes=${hit.output.length}`,
  );
  if (listed.code !== 0 || rows < 1000 || hit.code !== 0 || hit.output.length < 100) {
    console.log('FAIL: the unmutated stored-DEX control does not work, so the corpus is not reaching the parser');
    process.exit(1);
  }
}

for (const [label, mutate, cut] of MUTATIONS) {
  const bytes = Buffer.from(cut === undefined ? dex : dex.subarray(0, cut));
  if (mutate) {
    mutate(bytes);
    if (bytes.equals(dex)) unchanged += 1;
  }
  const apkBytes = buildZip('classes.dex', bytes);
  const rasc = await Rasc.load({ wasm: readFileSync(wasmPath), source: sourceFromBytes(apkBytes) });
  for (const argv of commands) {
    runs += 1;
    let result;
    try {
      result = rasc.run(argv);
    } catch (error) {
      traps += 1;
      failures.push(`${label} ${argv[0]}: TRAP ${error}`);
      continue;
    }
    const text = result.code === 0 ? '' : Buffer.from(result.output).toString('latin1');
    if (result.code === 0) succeeded += 1;
    if (text.includes('PANIC:')) {
      panics += 1;
      failures.push(`${label} ${argv[0]}: ${text.trim().slice(0, 140)}`);
    }
    if (result.code < 0 || result.code > 2) {
      badCodes += 1;
      failures.push(`${label} ${argv[0]}: exit ${result.code}`);
    }
  }
  rasc.close();
}

for (const failure of failures.slice(0, 25)) console.log(`  ${failure}`);
console.log(
  `dex_mutations=${MUTATIONS.length} runs=${runs} traps=${traps} panics=${panics} ` +
    `bad_codes=${badCodes} unchanged_mutations=${unchanged}`,
);
console.log(`exit codes: ${succeeded} succeeded, ${runs - succeeded - traps} rejected with an error`);
console.log(traps + panics + badCodes === 0 ? `PASS (${runs} runs clean)` : 'FAIL');
process.exit(traps + panics + badCodes === 0 ? 0 : 1);
