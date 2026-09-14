#!/usr/bin/env node
/**
 * The wasm CLI must behave like the native one.
 *
 *   APK=/path/to/app.apk node js/cli-test.mjs
 *
 * Runs each command through `js/bin/rasc-wasm.mjs` and through the native binary with the same
 * arguments and compares exit code, stdout bytes and - for failures - the stderr text. The
 * stdout contract is what makes it usable as a drop-in: a payload on stdout and nothing else,
 * errors on stderr with stdout empty.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

const apk = process.env.APK;
if (!apk || !existsSync(apk)) {
  console.log('skip: set APK=/path/to/app.apk');
  process.exit(0);
}
const repo = new URL('..', import.meta.url).pathname;
const wrapper = join(repo, 'js/bin/rasc-wasm.mjs');
const native = process.env.RASC_BIN ?? join(repo, 'target/release/rasc');
const wasmPath =
  process.env.RASC_WASM ?? join(repo, 'target/wasm32-unknown-unknown/release/rasc.wasm');
for (const [label, path] of [
  ['wrapper', wrapper],
  ['native', native],
  ['wasm', wasmPath],
]) {
  if (!existsSync(path)) {
    console.error(`missing ${label}: ${path}`);
    process.exit(1);
  }
}

const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** Runs a command and returns its status plus both streams, without throwing on failure. */
function run(file, argv) {
  try {
    const out = execFileSync(file, argv, { maxBuffer: 1 << 30 });
    return { code: 0, stdout: out, stderr: '' };
  } catch (error) {
    return {
      code: error.status ?? -1,
      stdout: error.stdout ?? Buffer.alloc(0),
      stderr: (error.stderr ?? Buffer.alloc(0)).toString('utf8'),
    };
  }
}

let checks = 0;
let failures = 0;
const check = (label, ok, detail = '') => {
  checks += 1;
  if (!ok) failures += 1;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? `  ${detail}` : ''}`);
};

/**
 * A class from the archive's latest dex, by name rather than by a hardcoded `classes56.dex`,
 * so the probe works with any APK. `classes.dex` ranks as 1, `classes2.dex` as 2.
 */
function latestDexClass(indexOutput) {
  const rows = indexOutput.split('\n').filter(Boolean);
  const rank = (name) => Number(/(\d+)\.dex$/.exec(name)?.[1] ?? 1);
  const names = [...new Set(rows.map((row) => row.split(' | ')[0]))].sort((a, b) => rank(a) - rank(b));
  const last = names[names.length - 1];
  return rows.find((row) => row.startsWith(`${last} | `))?.split(' | ')[1];
}

const late = latestDexClass(
  execFileSync(native, ['classes', '--threads', '8', apk], { maxBuffer: 1 << 30 }).toString('utf8'),
);
if (!late) {
  console.error('the class index has no usable rows');
  process.exit(1);
}

const cases = [
  ['manifest', ['manifest', apk], true],
  ['classes', ['classes', '--threads', '1', apk], true],
  ['findrefs field', ['findrefs', '--threads', '1', apk, 'field', 'INSTANCE'], true],
  ...(late ? [[`getclass ${late}`, ['getclass', '--threads', '1', apk, late], true]] : []),
  ['missing class', ['getclass', '--threads', '1', apk, 'Lcom/example/Absent;'], true],
  // Archive-free: the bundled skill is the payload, and both builds must embed it byte for byte.
  ['skill --print', ['skill', '--print'], true],
  // A usage error the wrapper reports itself, so the exit code is the wrapper's own (2)
  // rather than the module's 1: the arguments are checked before a module is instantiated.
  ['no such archive', ['manifest', join(repo, 'does-not-exist.apk')], false],
];

for (const [label, argv, compareExit] of cases) {
  const viaWrapper = run('node', [wrapper, ...argv]);
  const viaNative = run(native, argv);
  if (compareExit) {
    check(
      `${label}: exit code`,
      viaWrapper.code === viaNative.code,
      `wrapper=${viaWrapper.code} native=${viaNative.code}`,
    );
  } else {
    check(`${label}: exit code`, viaWrapper.code === 2, `wrapper=${viaWrapper.code}`);
  }
  check(
    `${label}: stdout`,
    digest(viaWrapper.stdout) === digest(viaNative.stdout),
    `bytes=${viaWrapper.stdout.length} vs ${viaNative.stdout.length}`,
  );
  if (viaNative.code !== 0) {
    // Errors belong on stderr with stdout empty, exactly as the native CLI does it.
    check(
      `${label}: error text on stderr`,
      viaWrapper.stderr.trim().length > 0 && viaWrapper.stdout.length === 0,
      JSON.stringify(viaWrapper.stderr.trim().slice(0, 70)),
    );
  }
}

console.log(failures === 0 ? `PASS (${checks} checks)` : `FAIL (${failures} of ${checks})`);
process.exit(failures === 0 ? 0 : 1);
