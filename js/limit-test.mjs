#!/usr/bin/env node
/**
 * Host-settable per-entry inflation ceiling.
 *
 *   APK=/path/to/app.apk node js/limit-test.mjs
 *
 * The ceiling used to be a fixed 256 MiB constant. A host with a tighter budget - a browser
 * tab holding a wasm instance - should be able to lower it, and this checks that the knob
 * works and that it is a *per entry* policy rather than a global size check:
 *
 *   default        -> the real APK still produces the native bytes
 *   4 MiB          -> `findrefs` is refused with a structured error (no trap),
 *                     while `manifest`, whose entry is far smaller, still works
 *   back to default-> `classes` works again
 *
 * The refused command has to be one whose per-entry inflation is above the ceiling on
 * any real archive: `findrefs` inflates every DEX entry whole, which is 8-13 MiB for
 * the corpora here. `classes` used to serve as the example, but a string-only command
 * only reads the part of an entry before the code section (26-65%), so whether it fits
 * under 4 MiB is a property of the archive's layout rather than of the ceiling - and a
 * command that *does* fit under the ceiling is supposed to run, not to be refused.
 */
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { DEFAULT_WASM, Rasc, sourceFromFile } from './node.mjs';

const apk = process.env.APK;
if (!apk || !existsSync(apk)) {
  console.log('skip: set APK=/path/to/app.apk');
  process.exit(0);
}
const repo = new URL('..', import.meta.url).pathname;
const native = process.env.RASC_BIN ?? join(repo, 'target/release/rasc');
const wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM;
for (const [label, path] of [
  ['wasm', wasmPath],
  ['native', native],
]) {
  if (!existsSync(path)) {
    console.error(`missing ${label}: ${path}`);
    process.exit(1);
  }
}

const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
const nativeDigest = (argv) => digest(execFileSync(native, argv, { maxBuffer: 1 << 30 }));
const sha = (bytes) => Buffer.from(bytes).toString('latin1');

let checks = 0;
let failures = 0;
const check = (label, ok, detail = '') => {
  checks += 1;
  if (!ok) failures += 1;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `  ${detail}` : ''}`);
};

const wasm = readFileSync(wasmPath);
const classes = ['classes', '--threads', '1', apk];
const manifest = ['manifest', apk];

// Default ceiling: unchanged behaviour.
const rasc = await Rasc.load({ wasm, source: sourceFromFile(apk) });
const defaultClasses = rasc.run(classes);
check('default: classes matches native', digest(defaultClasses.output) === nativeDigest(classes));
check('default: manifest matches native', digest(rasc.run(manifest).output) === nativeDigest(manifest));

// A 4 MiB ceiling: small entries still work, entries that need whole inflation do not.
const previous = rasc.setMaxInflatedEntry(4 << 20);
check('setter reports the previous ceiling', previous === 0 || previous >= 4 << 20, `previous=${previous}`);
check('4 MiB: manifest still matches native', digest(rasc.run(manifest).output) === nativeDigest(manifest));
const overLimit = rasc.run(['findrefs', apk, 'string', 'Authorization']);
check(
  '4 MiB: findrefs refused with a structured error',
  overLimit.code !== 0 && sha(overLimit.output).includes('entry limit'),
  `exit=${overLimit.code} text=${JSON.stringify(sha(overLimit.output).trim().slice(0, 80))}`,
);

// 0 restores the built-in default.
const restored = rasc.setMaxInflatedEntry(0);
check('restore reports the lowered ceiling', restored === (4 << 20), `previous=${restored}`);
check('restored: classes matches native again', digest(rasc.run(classes).output) === nativeDigest(classes));
rasc.close();

console.log(failures === 0 ? `PASS (${checks} checks)` : `FAIL (${failures} of ${checks})`);
process.exit(failures === 0 ? 0 : 1);
