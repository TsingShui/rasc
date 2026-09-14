#!/usr/bin/env node
/**
 * Hostile-input check for the wasm module.
 *
 *   APK=/path/to/app.apk node js/mutation-test.mjs [count]
 *
 * The native binary has its own mutation harness. The wasm build needs its own,
 * because there a panic is not a message on stderr: `panic = "abort"` turns it into an
 * unreachable trap, which in a browser kills the whole instance. So the property to check
 * is "no trap, no PANIC, exit code 0/1/2" for every command on every mutated archive.
 *
 * Mutations are deterministic (seed below) and applied in memory - the archive is mutated
 * in place and restored afterwards, and truncation is served as a subarray view - so a
 * 343 MiB sample costs no disk writes. The same kinds as the native checker are used:
 * truncate, bit flips, zero runs, a damaged central-directory entry and a damaged
 * AndroidManifest.xml payload.
 *
 * Note on coverage: the native checker's `dex-header` kind searches the archive for a DEX
 * magic, but a real APK stores its DEX entries deflated, so that search usually finds
 * nothing and the mutation is a no-op. This script reports how many mutations changed
 * nothing instead of hiding it.
 */
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { Rasc, sourceFromBytes, withReadAhead } from './node.mjs';

const apk = process.env.APK;
if (!apk || !existsSync(apk)) {
  console.log('skip: set APK=/path/to/app.apk');
  process.exit(0);
}
const count = Number(process.argv[2] ?? process.env.MUTATIONS ?? 40);
const repo = new URL('..', import.meta.url).pathname;
const wasmPath =
  process.env.RASC_WASM ?? join(repo, 'target/wasm32-unknown-unknown/release/rasc.wasm');
if (!existsSync(wasmPath)) {
  console.error(`missing wasm: ${wasmPath}`);
  process.exit(1);
}

const wasm = readFileSync(wasmPath);
const base = readFileSync(apk);
const commands = [
  ['manifest', '{apk}'],
  ['classes', '--threads', '1', '{apk}'],
  ['findrefs', '--threads', '1', '{apk}', 'string', 'androidx'],
  ['findrefs', '--threads', '1', '{apk}', 'type', 'Ljava/lang/String;'],
  ['getclass', '--threads', '1', '{apk}', 'Landroid/app/Activity;'],
];

/** Deterministic generator, so a failure is reproducible from the seed alone. */
let state = 20260912;
const random = () => {
  state = (state + 0x6d2b79f5) | 0;
  let t = Math.imul(state ^ (state >>> 15), 1 | state);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const between = (low, high) => low + Math.floor(random() * (high - low));
const KINDS = ['truncate', 'flip', 'zero-run', 'central-dir', 'manifest'];

const localSignature = Buffer.from('PK\x03\x04');
const centralSignature = Buffer.from('PK\x01\x02');
const manifestName = Buffer.from('AndroidManifest.xml');

/**
 * Applies one mutation to `base` and returns how to undo it.
 *
 * `bytes` is what the module is given (a view for truncation), `changed` says whether any
 * byte actually differs, which is how no-op kinds are counted rather than assumed.
 */
function applyMutation() {
  const kind = KINDS[between(0, KINDS.length)];
  if (kind === 'truncate') {
    const end = between(1, base.length);
    return { label: `truncate@${end}`, bytes: base.subarray(0, end), changed: true, undo() {} };
  }
  if (kind === 'flip') {
    const touched = [];
    for (let i = 0, n = between(1, 8); i < n; i += 1) {
      const offset = between(0, base.length);
      touched.push([offset, base[offset]]);
      base[offset] ^= 1 << between(0, 8);
    }
    return {
      label: `flip×${touched.length}`,
      bytes: base,
      changed: true,
      undo: () => touched.forEach(([offset, value]) => (base[offset] = value)),
    };
  }
  if (kind === 'zero-run') {
    const start = between(0, base.length);
    const length = Math.min(between(1, 4096), base.length - start);
    const touched = base.subarray(start, start + length);
    const saved = Buffer.from(touched);
    touched.fill(0);
    return {
      label: `zero-run@${start}+${length}`,
      bytes: base,
      changed: true,
      undo: () => saved.copy(base, start),
    };
  }
  if (kind === 'central-dir') {
    const signature = base.lastIndexOf(centralSignature);
    if (signature < 0) return { label: 'central-dir(none)', bytes: base, changed: false, undo() {} };
    const touched = [];
    for (let i = 0, n = between(1, 5); i < n; i += 1) {
      const offset = signature + between(0, 46);
      touched.push([offset, base[offset]]);
      base[offset] = between(0, 256);
    }
    return {
      label: `central-dir@${signature}`,
      bytes: base,
      changed: true,
      undo: () => touched.forEach(([offset, value]) => (base[offset] = value)),
    };
  }
  // The manifest's payload, reached through its local header, so the inflate and AXML paths
  // see damaged bytes; a rewrite would need a ZIP writer and copy the whole archive.
  const name = base.indexOf(manifestName);
  const header = name < 0 ? -1 : base.lastIndexOf(localSignature, name);
  if (header < 0) return { label: 'manifest(none)', bytes: base, changed: false, undo() {} };
  const touched = [];
  for (let i = 0, n = between(1, 6); i < n; i += 1) {
    const offset = name + between(0, 4096);
    if (offset >= base.length) continue;
    touched.push([offset, base[offset]]);
    base[offset] = random() < 0.5 ? 0 : between(0, 256);
  }
  return {
    label: `manifest@${name}`,
    bytes: base,
    changed: touched.length > 0,
    undo: () => touched.forEach(([offset, value]) => (base[offset] = value)),
  };
}

let runs = 0;
let traps = 0;
let panics = 0;
let badCodes = 0;
let unchanged = 0;
const failures = [];

for (let index = 0; index < count; index += 1) {
  const mutation = applyMutation();
  if (!mutation.changed) unchanged += 1;
  try {
    const rasc = await Rasc.load({
      wasm,
      source: withReadAhead(sourceFromBytes(mutation.bytes), 1 << 16),
    });
    for (const template of commands) {
      const argv = template.map((part) => (part === '{apk}' ? 'mutated.apk' : part));
      runs += 1;
      let result;
      try {
        result = rasc.run(argv);
      } catch (error) {
        traps += 1;
        failures.push(`#${index} ${mutation.label} ${argv[0]}: TRAP ${error}`);
        continue;
      }
      if (result.code !== 0) {
        // An error path should produce a message, and a panic prints PANIC: through the
        // host hook rather than escaping as a trap.
        const text = Buffer.from(result.output).toString('latin1');
        if (text.includes('PANIC:')) {
          panics += 1;
          failures.push(`#${index} ${mutation.label} ${argv[0]}: ${text.trim().slice(0, 120)}`);
        }
      }
      if (result.code < 0 || result.code > 2) {
        badCodes += 1;
        failures.push(`#${index} ${mutation.label} ${argv[0]}: exit ${result.code}`);
      }
    }
    rasc.close();
  } finally {
    mutation.undo();
  }
}

for (const failure of failures.slice(0, 20)) console.log(`  ${failure}`);
console.log(
  `mutations=${count} runs=${runs} traps=${traps} panics=${panics} bad_codes=${badCodes} ` +
    `unchanged_mutations=${unchanged}`,
);
console.log(traps + panics + badCodes === 0 ? `PASS (${runs} runs clean)` : 'FAIL');
process.exit(traps + panics + badCodes === 0 ? 0 : 1);
