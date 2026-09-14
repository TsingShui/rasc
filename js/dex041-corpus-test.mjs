#!/usr/bin/env node
/**
 * Hostile-input check for the DEX 041 container layer (src/dex/container.rs).
 *
 *   node js/dex041-corpus-test.mjs
 *
 * No other corpus reaches this code: the sample APK has no 041 entries, so the member walk,
 * the header overlay and the container validation are only covered by Rust unit tests. A 041
 * entry is a tiling of 0x78-byte member headers whose section offsets are container-relative,
 * so this script builds containers directly - the members carry empty tables, which the
 * scanner accepts, so a valid container is a control that proves the path is reached.
 *
 * Every case runs through the wasm module and the native binary on the same archive and must
 * agree on the exit code, on the payload when it succeeds and on the error text when it
 * fails.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Rasc, sourceFromBytes } from './node.mjs';

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

const HEADER = 0x78;
const magic = 'dex\n041\0';

/** One member header: the fixed fields the container walk reads, tables left empty. */
function member(offset, total, fileSize = HEADER) {
  const bytes = Buffer.alloc(HEADER);
  bytes.write(magic, 0, 'latin1');
  bytes.writeUInt32LE(fileSize, 0x20);
  bytes.writeUInt32LE(HEADER, 0x24);
  bytes.writeUInt32LE(total, 0x70);
  bytes.writeUInt32LE(offset, 0x74);
  return bytes;
}

/** A valid container of `count` members tiling the whole address space. */
function container(count) {
  const total = HEADER * count;
  return Buffer.concat(Array.from({ length: count }, (_, index) => member(index * HEADER, total)));
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

const commands = [
  ['classes', '--threads', '1', 'mutated.apk'],
  ['findrefs', '--threads', '1', 'mutated.apk', 'type', 'Ljava/lang/String;'],
];

const dir = mkdtempSync(join(tmpdir(), 'rasc-041-'));
const path = join(dir, 'mutated.apk');
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
let errorTextsCompared = 0;

async function runBoth(containerBytes) {
  const archiveBytes = buildZip('classes.dex', containerBytes);
  writeFileSync(path, archiveBytes);
  const results = [];
  for (const argv of commands) {
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
      wasm = { trap: String(error).slice(0, 140) };
    }
    let nativeResult;
    try {
      const out = execFileSync(native, [argv[0], '--threads', '1', path, ...argv.slice(4)], {
        maxBuffer: 1 << 28,
      });
      nativeResult = { code: 0, sha: digest(out), text: '' };
    } catch (error) {
      nativeResult = {
        code: error.status ?? -1,
        sha: digest(error.stdout ?? Buffer.alloc(0)),
        text: (error.stderr ?? Buffer.alloc(0)).toString('latin1').trim(),
      };
    }
    results.push({ argv: argv[0], wasm, native: nativeResult });
  }
  return results;
}

function agrees({ wasm, native: reference }) {
  if (wasm.trap) return { ok: false, why: `TRAP ${wasm.trap}` };
  if (wasm.code !== reference.code) return { ok: false, why: `exit ${wasm.code} vs ${reference.code}` };
  if (wasm.code === 0) {
    return wasm.sha === reference.sha
      ? { ok: true }
      : { ok: false, why: `payload ${wasm.sha.slice(0, 8)} vs ${reference.sha.slice(0, 8)}` };
  }
  // Count only the cases where both sides really produced text, so a run where the native
  // side reported nothing cannot be mistaken for a passing comparison.
  if (wasm.text.length > 0 && reference.text.length > 0) errorTextsCompared += 1;
  return wasm.text === reference.text
    ? { ok: true }
    : { ok: false, why: `${JSON.stringify(wasm.text.slice(0, 70))} vs ${JSON.stringify(reference.text.slice(0, 70))}` };
}

// Controls: a valid container has to be accepted (exit 0), which is what proves the corpus
// reaches the container walk instead of being rejected as a plain malformed DEX.
for (const count of [1, 2]) {
  const outcome = await runBoth(container(count));
  const failed = outcome.filter((result) => !agrees(result).ok || result.wasm.code !== 0);
  console.log(
    `control ${count} member(s): ` +
      outcome.map((r) => `${r.argv} wasm=${r.wasm.code}/${r.wasm.sha.slice(0, 8)} native=${r.native.code}`).join(', '),
  );
  if (failed.length > 0) {
    console.log('FAIL: a valid container is not accepted, so the corpus proves nothing');
    console.log(failed.map((r) => `  ${r.argv}: ${JSON.stringify(r.wasm)}`).join('\n'));
    process.exit(1);
  }
}

const mutations = [];
const two = container(2);
const three = container(3);
const U32 = (offset, value) => (bytes) => bytes.writeUInt32LE(value, offset);
mutations.push(
  ['container_size=0 (0x70)', U32(0x70, 0)],
  ['container_size=huge', U32(0x70, 0xfffffff0)],
  ['container_size=one member less', U32(0x70, HEADER)],
  ['member2 header_offset=0', U32(HEADER + 0x74, 0)],
  ['member2 header_offset=huge', U32(HEADER + 0x74, 0xfffffff0)],
  ['member1 file_size=0', U32(0x20, 0)],
  ['member1 file_size=huge', U32(0x20, 0xfffffff0)],
  ['member1 file_size=off by one', U32(0x20, HEADER - 1)],
  ['member1 header_size=0x70', U32(0x24, 0x70)],
  ['member2 magic broken', (bytes) => bytes.write('dex\n040\0', HEADER, 'latin1')],
  ['member2 magic zeroed', (bytes) => bytes.fill(0, HEADER, HEADER + 8)],
  ['file type 0x0009', (bytes) => bytes.writeUInt16LE(0x0009, 0)],
  ['member2 table offsets huge', (bytes) => {
    bytes.writeUInt32LE(0xfffffff0, HEADER + 0x3c);
    bytes.writeUInt32LE(0xfffffff0, HEADER + 0x64);
  }],
);
for (const cut of [8, HEADER, HEADER * 2 - 1, (HEADER * 3) / 2]) {
  mutations.push([`truncated@${cut}`, null, cut]);
}
let clean = 0;
const failures = [];
for (const [label, mutate, cut] of mutations) {
  const bytes = cut === undefined ? Buffer.from(two) : three.subarray(0, cut);
  if (mutate) mutate(bytes);
  for (const outcome of await runBoth(bytes)) {
    const verdict = agrees(outcome);
    if (verdict.ok) {
      clean += 1;
    } else {
      failures.push(`${label} ${outcome.argv}: ${verdict.why}`);
    }
  }
}

// Scale: a tiling container with many members exercises the lazy member path in wasm (each
// member copies the whole container, so an eager implementation would allocate N x size).
const started = Date.now();
const many = await runBoth(container(1000));
const manyOk = many.every((outcome) => agrees(outcome).ok && outcome.wasm.code === 0);
console.log(`scale 1000 members: ok=${manyOk} in ${Date.now() - started} ms`);
if (manyOk) clean += many.length;

for (const failure of failures.slice(0, 25)) console.log(`  ${failure}`);
console.log(
  `mutations=${mutations.length} clean=${clean} failures=${failures.length} ` +
    `error_texts_compared=${errorTextsCompared}`,
);
console.log(failures.length === 0 && manyOk ? `PASS (${clean} runs agree)` : 'FAIL');
process.exit(failures.length === 0 && manyOk ? 0 : 1);
