#!/usr/bin/env node
/**
 * Decompression-bomb check.
 *
 *   node js/bomb-test.mjs [targetGiB]
 *
 * The core bounds the *first* allocation by the compressed size instead of trusting the
 * declared one, but the buffer still grows with whatever the deflate stream actually
 * produces. So an entry that really expands to gigabytes - which compresses to a few
 * megabytes, so the archive itself is tiny - is a resource-exhaustion attack against the
 * host: in wasm the linear memory cannot grow that far, allocation fails and the instance
 * traps instead of reporting an error.
 *
 * This script builds such an archive (a stream of zeros compressed to the target size) and
 * runs the wasm module and the native binary on it, reporting what each of them does.
 */
import { execFileSync } from 'node:child_process';
import { createDeflateRaw } from 'node:zlib';
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Rasc, sourceFromBytes } from './node.mjs';

const target = Number(process.argv[2] ?? 1.5) * 1024 * 1024 * 1024;
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

/** Deflate stream that expands to `bytes` of zeros, without materialising them. */
async function buildBomb(bytes) {
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

console.log(`building a bomb that expands to ${(target / 1024 ** 3).toFixed(2)} GiB ...`);
const compressed = await buildBomb(target);
const archive = buildZip('classes.dex', compressed, target, 8);
console.log(
  `bomb archive: ${archive.length} bytes (compressed payload ${compressed.length} bytes)`,
);

const commands = [
  ['classes', '--threads', '1', 'bomb.apk'],
  ['getclass', '--threads', '1', 'bomb.apk', 'Lcom/example/Absent;'],
];

let trapped = 0;
let clean = 0;
for (const argv of commands) {
  const started = Date.now();
  try {
    const rasc = await Rasc.load({
      wasm: readFileSync(wasmPath),
      source: sourceFromBytes(archive),
    });
    const result = rasc.run(argv);
    rasc.close();
    const text = result.code === 0 ? '' : Buffer.from(result.output).toString('latin1').trim();
    console.log(`  wasm ${argv[0]}: exit=${result.code} in ${Date.now() - started} ms  ${text.slice(0, 120)}`);
    clean += 1;
  } catch (error) {
    console.log(`  wasm ${argv[0]}: TRAP in ${Date.now() - started} ms  ${String(error).slice(0, 120)}`);
    trapped += 1;
  }
}

const dir = mkdtempSync(join(tmpdir(), 'rasc-bomb-'));
const path = join(dir, 'bomb.apk');
writeFileSync(path, archive);
for (const argv of commands) {
  const started = Date.now();
    try {
    const out = execFileSync(native, argv.map((part) => (part === 'bomb.apk' ? path : part)), {
      timeout: 120000,
      maxBuffer: 1 << 28,
    });
    console.log(`  native ${argv[0]}: exit=0 in ${Date.now() - started} ms bytes=${out.length}`);
  } catch (error) {
    const message = (error.stderr ?? Buffer.alloc(0)).toString('latin1').trim().slice(0, 120);
    console.log(
      `  native ${argv[0]}: exit=${error.status ?? 'signal ' + error.signal} in ${Date.now() - started} ms  ${message}`,
    );
  }
}

console.log(`wasm: clean=${clean} trapped=${trapped}`);
console.log(clean === commands.length ? 'PASS (bomb rejected without a trap)' : 'FAIL (bomb traps the instance)');
process.exit(clean === commands.length ? 0 : 1);
