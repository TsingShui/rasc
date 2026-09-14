#!/usr/bin/env node
/**
 * Crafted-archive check for the ZIP layer: inconsistencies between the central directory and
 * the local headers.
 *
 *   APK=/path/to/app.apk node js/zip-craft-corpus-test.mjs
 *
 * The other corpora corrupt bytes at random, which almost never produces a *coherent*
 * mismatch. A ZIP reader takes its entry list from the central directory but its file data
 * offset from the local header, so an archive where the two disagree is exactly where a
 * crafted file can make a naive reader read the wrong bytes, or past the end.
 *
 * Every case is a hand-built archive over the sample's real `classes.dex` and
 * `AndroidManifest.xml`, so the control proves the pipeline works before anything is broken,
 * and every case runs through the wasm module and the native binary on the same file and has
 * to agree on the exit code, on the payload when it succeeds and on the error text when it
 * fails.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
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

const sample = readFileSync(apk);

/** Pulls one entry's bytes out of the sample (central directory + inflate). */
function extract(name) {
  const eocd = sample.lastIndexOf(Buffer.from('PK\x05\x06'));
  const count = sample.readUInt16LE(eocd + 10);
  let at = sample.readUInt32LE(eocd + 16);
  for (let index = 0; index < count; index += 1) {
    const nameLength = sample.readUInt16LE(at + 28);
    const extraLength = sample.readUInt16LE(at + 30);
    const commentLength = sample.readUInt16LE(at + 32);
    if (sample.toString('utf8', at + 46, at + 46 + nameLength) === name) {
      const method = sample.readUInt16LE(at + 10);
      const compressedSize = sample.readUInt32LE(at + 20);
      const local = sample.readUInt32LE(at + 42);
      const start =
        local + 30 + sample.readUInt16LE(local + 26) + sample.readUInt16LE(local + 28);
      const stored = sample.subarray(start, start + compressedSize);
      return method === 8 ? inflateRawSync(stored) : Buffer.from(stored);
    }
    at += 46 + nameLength + extraLength + commentLength;
  }
  return null;
}

const dex = extract('classes.dex');
const manifest = extract('AndroidManifest.xml');
if (!dex || !manifest) {
  console.error('the sample needs both classes.dex and AndroidManifest.xml');
  process.exit(1);
}

/**
 * Builds an archive from explicit per-entry fields, so the local header and the central
 * directory can be made to disagree on purpose.
 *
 * Defaults describe a well-formed stored entry; each case overrides only what it is about.
 */
function build(specs, eocdOverrides = {}) {
  const body = [];
  const central = [];
  let offset = 0;
  for (const spec of specs) {
    const data = spec.data;
    const localName = Buffer.from(spec.localName ?? spec.name);
    const localExtra = Buffer.alloc(spec.localExtraLength ?? 0);
    const local = Buffer.alloc(30 + localName.length + localExtra.length);
    local.write('PK\x03\x04', 0, 'latin1');
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(spec.flags ?? 0, 6);
    local.writeUInt16LE(spec.method ?? 0, 8);
    local.writeUInt32LE(spec.localCompressed ?? data.length, 18);
    local.writeUInt32LE(spec.localUncompressed ?? data.length, 22);
    local.writeUInt16LE(localName.length, 26);
    local.writeUInt16LE(localExtra.length, 28);
    localName.copy(local, 30);

    const cdName = Buffer.from(spec.cdName ?? spec.name);
    const cdExtra = Buffer.alloc(spec.cdExtraLength ?? 0);
    const cd = Buffer.alloc(46 + cdName.length + cdExtra.length);
    cd.write('PK\x01\x02', 0, 'latin1');
    cd.writeUInt16LE(20, 4);
    cd.writeUInt16LE(20, 6);
    cd.writeUInt16LE(spec.flags ?? 0, 8);
    cd.writeUInt16LE(spec.method ?? 0, 10);
    cd.writeUInt32LE(spec.cdCompressed ?? data.length, 20);
    cd.writeUInt32LE(spec.cdUncompressed ?? data.length, 24);
    cd.writeUInt16LE(cdName.length, 28);
    cd.writeUInt16LE(cdExtra.length, 30);
    cd.writeUInt32LE(spec.localOffset ?? offset, 42);
    cdName.copy(cd, 46);

    body.push(local, data);
    central.push(cd);
    offset += local.length + data.length;
  }
  const cdStart = offset;
  const cdBytes = Buffer.concat(central);
  const eocd = Buffer.alloc(22);
  eocd.write('PK\x05\x06', 0, 'latin1');
  eocd.writeUInt16LE(specs.length, 8);
  eocd.writeUInt16LE(eocdOverrides.count ?? specs.length, 10);
  eocd.writeUInt32LE(eocdOverrides.cdSize ?? cdBytes.length, 12);
  eocd.writeUInt32LE(eocdOverrides.cdOffset ?? cdStart, 16);
  return Buffer.concat([...body, cdBytes, eocd]);
}

const wellFormed = [
  { name: 'classes.dex', data: dex },
  { name: 'AndroidManifest.xml', data: manifest },
];
const cases = [
  ['control', () => build(wellFormed)],
  ['local name shorter than the directory', () => build([{ name: 'classes.dex', data: dex, localName: 'clas' }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['local extra length past the end', () => build([{ name: 'classes.dex', data: dex, localExtraLength: 5000 }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['unknown compression method', () => build([{ name: 'classes.dex', data: dex, method: 12 }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['stored entry whose sizes disagree', () => build([{ name: 'classes.dex', data: dex, method: 0, cdCompressed: dex.length - 4, cdUncompressed: dex.length }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['deflate entry with an empty stream', () => build([{ name: 'classes.dex', data: Buffer.alloc(0), method: 8, cdCompressed: 0, cdUncompressed: dex.length }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['zip64 placeholder uncompressed size', () => build([{ name: 'classes.dex', data: dex, cdUncompressed: 0xfffffff0, localUncompressed: 0xfffffff0 }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['local offset into another entry', () => build([{ name: 'classes.dex', data: dex, localOffset: 60 }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['two entries sharing one local header', () => build([{ name: 'classes.dex', data: dex }, { name: 'AndroidManifest.xml', data: manifest, localOffset: 0 }])],
  ['directory lists more entries than exist', () => build(wellFormed, { count: 5 })],
  ['directory offset points at a local header', () => build(wellFormed, { cdOffset: 0 })],
  ['dex only in a subdirectory', () => build([{ name: 'assets/classes.dex', data: dex }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['name length past the end', () => build([{ name: 'classes.dex', data: dex, cdName: 'x'.repeat(4096) }, { name: 'AndroidManifest.xml', data: manifest }])],
  ['data descriptor flag with zero local sizes', () => build([{ name: 'classes.dex', data: dex, flags: 8, localCompressed: 0, localUncompressed: 0 }, { name: 'AndroidManifest.xml', data: manifest, flags: 8, localCompressed: 0, localUncompressed: 0 }])],
  ['duplicate entry names', () => build([{ name: 'classes.dex', data: dex }, { name: 'classes.dex', data: dex }, { name: 'AndroidManifest.xml', data: manifest }])],
];

const commands = [
  ['manifest', 'crafted.apk'],
  ['classes', '--threads', '1', 'crafted.apk'],
  ['findrefs', '--threads', '1', 'crafted.apk', 'string', 'androidx'],
  ['getclass', '--threads', '1', 'crafted.apk', 'Landroid/app/Activity;'],
];

const dir = mkdtempSync(join(tmpdir(), 'rasc-zip-'));
const path = join(dir, 'crafted.apk');
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
let errorTextsCompared = 0;

async function runOne(archiveBytes, argv) {
  writeFileSync(path, archiveBytes);
  let wasm;
  try {
    const rasc = await Rasc.load({ wasm: readFileSync(wasmPath), source: sourceFromBytes(archiveBytes) });
    const result = rasc.run(argv);
    rasc.close();
    wasm = {
      code: result.code,
      sha: digest(result.output),
      text: Buffer.from(result.output).toString('latin1').trim(),
    };
  } catch (error) {
    return { ok: false, why: `TRAP ${String(error).slice(0, 120)}` };
  }
  let reference;
  try {
    const out = execFileSync(
      native,
      argv.map((part) => (part === 'crafted.apk' ? path : part)),
      { maxBuffer: 1 << 28 },
    );
    reference = { code: 0, sha: digest(out), text: '' };
  } catch (error) {
    reference = {
      code: error.status ?? -1,
      sha: digest(error.stdout ?? Buffer.alloc(0)),
      text: (error.stderr ?? Buffer.alloc(0)).toString('latin1').trim(),
    };
  }
  if (wasm.code !== reference.code) return { ok: false, why: `exit ${wasm.code} vs ${reference.code}` };
  if (wasm.code === 0) {
    return wasm.sha === reference.sha
      ? { ok: true }
      : { ok: false, why: `payload ${wasm.sha.slice(0, 8)} vs ${reference.sha.slice(0, 8)}` };
  }
  if (wasm.text.length > 0 && reference.text.length > 0) errorTextsCompared += 1;
  return wasm.text === reference.text
    ? { ok: true }
    : { ok: false, why: `${JSON.stringify(wasm.text.slice(0, 70))} vs ${JSON.stringify(reference.text.slice(0, 70))}` };
}

// Control: the well-formed archive must work through both hosts, and its manifest must be
// the same bytes the real APK yields - that is what makes the corpus non-vacuous.
const controlArchive = build(wellFormed);
const controlManifest = await runOne(controlArchive, commands[0]);
const realManifest = execFileSync(native, ['manifest', apk], { maxBuffer: 1 << 28 });
const controlOk =
  controlManifest.ok &&
  digest(realManifest) ===
    digest(
      execFileSync(native, ['manifest', path], { maxBuffer: 1 << 28 }),
    );
console.log(`control: manifest agrees with native=${controlManifest.ok}, well-formed archive decodes=${controlOk}`);
if (!controlOk) {
  console.log('FAIL: the well-formed control does not decode, so the corpus proves nothing');
  process.exit(1);
}

let clean = 0;
const failures = [];
for (const [label, make] of cases) {
  const archiveBytes = make();
  for (const argv of commands) {
    const verdict = await runOne(archiveBytes, argv);
    if (verdict.ok) clean += 1;
    else failures.push(`${label} ${argv[0]}: ${verdict.why}`);
  }
}

for (const failure of failures.slice(0, 25)) console.log(`  ${failure}`);
console.log(
  `cases=${cases.length} runs=${cases.length * commands.length} clean=${clean} ` +
    `failures=${failures.length} error_texts_compared=${errorTextsCompared}`,
);
console.log(failures.length === 0 ? `PASS (${clean} runs agree)` : 'FAIL');
process.exit(failures.length === 0 ? 0 : 1);
