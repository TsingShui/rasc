/**
 * The WASI build is the same program as the native one.
 *
 * That is now the only property worth checking, and it is checkable from outside the
 * program: run the same command line through the native binary and through the WASI module
 * under a WASI host, and compare SHA-256 of stdout, SHA-256 of stderr and the exit code.
 *
 *   APK=/path/to.apk node js/wasi-test.mjs
 *   NATIVE=/path/to/rasc node js/wasi-test.mjs
 *
 * Every case that is expected to print something carries a **control**: an empty payload
 * compared against an empty payload is a pass that proved nothing, which this repository has
 * been fooled by before.
 *
 * This replaces the host-module suites. They drove `rasc_host_*` imports and the record
 * modes; both are gone, and what is left is a program whose whole contract is its command
 * line, its two streams and its exit code.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DEFAULT_WASM } from './wasi-run.mjs';

// Absolute, because WASI has no working directory: a relative path has nothing to resolve
// against in the guest, so a host hands over a path its own preopens cover.
const APK = resolve(process.argv[2] ?? process.env.APK ?? '');
if (APK === '/' || !(process.argv[2] ?? process.env.APK)) {
  console.error('usage: APK=/path/to.apk node js/wasi-test.mjs  (or pass the path as argv[2])');
  process.exit(2);
}
const NATIVE =
  process.env.NATIVE ?? fileURLToPath(new URL('../target/release/rasc', import.meta.url));
const RUNNER = fileURLToPath(new URL('./wasi-run.mjs', import.meta.url));

const sha = (data) => createHash('sha256').update(data).digest('hex').slice(0, 16);

/**
 * The errno a failed `open` reports is the platform's, and WASI's numbers are its own:
 * `os error 2` for a missing file natively is `os error 44` there. The wording, the stream
 * and the exit code are the contract; the number belongs to the host's errno table. Folding
 * it away is the only allowance - every other difference still fails.
 */
const normalize = (buffer) =>
  Buffer.from(buffer.toString('utf8').replace(/\(os error \d+\)/g, '(os error N)'), 'utf8');

/** The native binary, with the same argument list. One process per case, as in the browser. */
function nativeRun(args, env = {}) {
  const started = performance.now();
  const { stdout, stderr, status } = spawnSync(NATIVE, args, { maxBuffer: 1 << 30, env: { ...env } });
  return { stdout, stderr, status, ms: performance.now() - started };
}

/** The WASI module under Node's WASI, one process per case: a fresh instance every time. */
function wasiRun(args, env = {}) {
  const started = performance.now();
  const { stdout, stderr, status } = spawnSync(
    process.execPath,
    ['--experimental-wasi-unstable-preview1', '--no-warnings', RUNNER, ...args],
    {
      maxBuffer: 1 << 30,
      env: { ...env, RASC_WASM: process.env.RASC_WASM ?? DEFAULT_WASM },
    },
  );
  return { stdout, stderr, status, ms: performance.now() - started };
}

/** `[name, args, control, compareStderr]`; a sizeable command must print something. */
const cases = [
  ['manifest', ['manifest', APK], true, true],
  ['entries', ['entries', APK], true, true],
  ['classes', ['classes', APK], true, true],
  ['classes --filter', ['classes', '--filter', 'Activity', APK], true, true],
  ['strings --limit 5', ['strings', '--limit', '5', APK], true, true],
  ['findrefs type', ['findrefs', '--threads', '1', APK, 'type', 'View'], true, true],
  ['skill --print', ['skill', '--print'], true, true],
  ['--help', ['--help'], true, true],
  ['--version', ['--version'], true, true],
  ['usage error', ['manifest'], false, true],
  ['missing archive', ['manifest', '/nonexistent.apk'], false, true],
];

let failures = 0;
let nativeMs = 0;
let wasiMs = 0;
console.log(`archive ${APK}\n`);
for (const [name, args, expectOutput, compareStderr] of cases) {
  const native = nativeRun(args);
  const wasi = wasiRun(args);
  nativeMs += native.ms;
  wasiMs += wasi.ms;
  const sameOut = sha(wasi.stdout) === sha(native.stdout);
  const sameErr = sha(normalize(wasi.stderr)) === sha(normalize(native.stderr));
  const sameCode = wasi.status === native.status;
  const control = !expectOutput || wasi.stdout.length > 0;
  const ok = sameOut && sameCode && control && (compareStderr ? sameErr : true);
  if (!ok) failures += 1;
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${name.padEnd(24)} exit ${String(wasi.status)}/${String(native.status)}` +
      ` stdout ${sameOut ? '=' : '≠'} ${sha(wasi.stdout)} ${String(wasi.stdout.length).padStart(9)}B` +
      ` stderr ${sameErr ? '=' : '≠'} ${String(wasi.stderr.length).padStart(6)}B` +
      ` ${String(Math.round(wasi.ms)).padStart(5)}ms wasi / ${String(Math.round(native.ms)).padStart(5)}ms native` +
      `${control ? '' : ' CONTROL'}`,
  );
}

// A class from the archive itself, decompiled by the WASI build: the one command whose
// output is written by the Java emitter, and the reason the browser needs this engine at all.
// A class that is not in the archive first, so the failure path is compared too.
{
  const args = ['getclass', '--threads', '1', APK, 'no.Such.Class'];
  const native = nativeRun(args);
  const wasi = wasiRun(args);
  const ok =
    sha(wasi.stdout) === sha(native.stdout) &&
    sha(normalize(wasi.stderr)) === sha(normalize(native.stderr)) &&
    wasi.status === native.status;
  if (!ok) failures += 1;
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${'getclass missing class'.padEnd(24)} exit ${String(wasi.status)}/${String(native.status)}` +
      ` stdout ${sha(wasi.stdout) === sha(native.stdout) ? '=' : '≠'} stderr ${sha(normalize(wasi.stderr)) === sha(normalize(native.stderr)) ? '=' : '≠'}`,
  );
}
const classes = nativeRun(['classes', '--filter', 'Activity', APK]).stdout.toString('utf8');
const klass = classes
  .split('\n')
  .map((line) => line.split('|')[1]?.trim())
  .find((descriptor) => descriptor && !descriptor.includes('$'));
if (klass) {
  const args = ['getclass', '--threads', '1', APK, klass];
  const native = nativeRun(args);
  const wasi = wasiRun(args);
  const sameOut = sha(wasi.stdout) === sha(native.stdout);
  const ok = sameOut && wasi.status === native.status && wasi.stdout.length > 0;
  if (!ok) failures += 1;
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${'getclass '.padEnd(24)} exit ${String(wasi.status)}/${String(native.status)}` +
      ` stdout ${sameOut ? '=' : '≠'} ${sha(wasi.stdout)} ${String(wasi.stdout.length).padStart(9)}B` +
      `  ${klass}`,
  );
}

// The inflation ceiling is host policy that arrives in the environment, so both targets have
// to honour it the same way: refuse this archive's DEX entries and say so the same way.
const limit = { RASC_MAX_INFLATED_ENTRY: String(1024) };
for (const [name, args] of [
  ['limit: classes refused', ['classes', APK]],
  ['limit: manifest still works', ['manifest', APK]],
]) {
  const native = nativeRun(args, limit);
  const wasi = wasiRun(args, limit);
  const sameOut = sha(wasi.stdout) === sha(native.stdout);
  const sameErr = sha(normalize(wasi.stderr)) === sha(normalize(native.stderr));
  const ok = sameOut && sameErr && wasi.status === native.status;
  if (!ok) failures += 1;
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${name.padEnd(24)} exit ${String(wasi.status)}/${String(native.status)}` +
      ` stdout ${sameOut ? '=' : '≠'} stderr ${sameErr ? '=' : '≠'}` +
      ` ${JSON.stringify(wasi.stderr.toString('utf8').trim().slice(0, 60))}`,
  );
}

console.log(
  `\ntotal ${Math.round(wasiMs)}ms wasi / ${Math.round(nativeMs)}ms native` +
    ` (${(wasiMs / nativeMs).toFixed(2)}x, one process per case)`,
);
console.log(failures === 0 ? 'the WASI build is the native program' : `${failures} case(s) failed`);
process.exitCode = failures === 0 ? 0 : 1;
