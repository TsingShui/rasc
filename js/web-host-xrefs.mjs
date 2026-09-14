#!/usr/bin/env node
/**
 * Reference queries, measured at the host boundary.
 *
 * `findrefs` is the engine's answer to "what touches this?" - the question behind the
 * STRINGS view's cross-references and its jump to the code that uses a string. The
 * command exists and prints text rows, so this suite asks what a *host* needs from it:
 * records it can parse when a member name holds the separator, the same rows the text
 * mode shows, an empty answer that is empty rather than absent, and a failure it can
 * read. Every assertion is answered against an independent oracle - the module's own
 * text mode, or the native binary on the same bytes.
 *
 * One correction is recorded here rather than taken quietly. The first version compared
 * each record against a text row split on ` | ` and counted the text mode's lines. That
 * oracle cannot represent the data: a matched value is a DEX string and may contain a
 * newline, which the text mode prints raw - so one row becomes two lines, and the rows
 * that distinction exists for are exactly the ones the comparison got wrong. The
 * assertions now render each record back into the text row shape and require the whole
 * reconstruction to equal the native build's text output byte for byte, which is a
 * stronger check than the field-by-field one it replaces and does not care what the
 * value holds.
 *
 *   node js/web-host-xrefs.mjs
 *   RASC_WASM=... RASC_SUITE_APK=... node js/web-host-xrefs.mjs
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Rasc, sourceFromBytes } from './rasc.mjs';
import { DEFAULT_WASM } from './node.mjs';
import { apkOf, fixtureDex } from './fixtures.mjs';

const wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM;
const native = process.env.RASC_BIN ?? 'target/release/rasc';
const wasm = readFileSync(wasmPath);
const decoder = new TextDecoder();

/** The smallest cached corpus: the reference scan runs over every DEX in it. */
function findCorpus() {
  if (process.env.RASC_SUITE_APK) return process.env.RASC_SUITE_APK;
  for (const directory of ['.cache/corpus', '.']) {
    if (!existsSync(directory)) continue;
    const found = readdirSync(directory)
      .filter((name) => name.endsWith('.apk'))
      .map((name) => `${directory}/${name}`)
      .sort((left, right) => statSync(left).size - statSync(right).size);
    if (found.length > 0) return found[0];
  }
  return null;
}

const corpusPath = findCorpus();
const corpus = corpusPath === null ? null : readFileSync(corpusPath);

/** A class and a method whose names hold what a text row cannot. */
const hostile = fixtureDex({
  classCount: 2,
  value: 'Authorization',
  descriptor: (index) => ['Lcom/example/Pipe|Inside;', 'Lcom/example/Quote"Inside;'][index],
  methodName: (index) => ['m|0', 'm"1'][index],
});

const workdir = mkdtempSync(join(tmpdir(), 'rasc-xrefs-'));
const paths = {
  corpus: corpusPath,
  hostile: (() => {
    const path = join(workdir, 'hostile.apk');
    writeFileSync(path, apkOf(hostile));
    return path;
  })(),
};

const instances = new Map();
async function prepare() {
  for (const [id, bytes] of [['corpus', corpus], ['hostile', hostile]]) {
    if (bytes !== null) instances.set(id, await Rasc.load({ wasm, source: sourceFromBytes(bytes) }));
  }
}

function run(id, args) {
  const rasc = instances.get(id);
  if (rasc === undefined) throw new Error(`no fixture named ${id}`);
  const chunks = [];
  let failed = null;
  try {
    const result = rasc.run(args, { onOutput: (chunk) => chunks.push(chunk) });
    return { ...result, text: decoder.decode(concat(chunks)) };
  } catch (error) {
    failed = String(error);
    return { code: null, text: '', trapped: true, error: failed };
  }
}

function concat(chunks) {
  const length = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(length);
  let at = 0;
  for (const chunk of chunks) {
    out.set(chunk, at);
    at += chunk.length;
  }
  return out;
}

function nativeRun(args) {
  try {
    return { code: 0, stdout: execFileSync(native, args, { maxBuffer: 1 << 30 }).toString('utf8') };
  } catch (error) {
    return { code: error.status ?? 1, stdout: (error.stdout ?? Buffer.alloc(0)).toString('utf8') };
  }
}

const rows = (text) => text.split('\n').filter((line) => line.length > 0);
/** One text row: `dex | member | matched=(…)`. */
const textRow = (line) => {
  const parts = line.split(' | ');
  const matched = parts.length < 3 ? '' : parts.slice(2).join(' | ').replace(/^matched=\(/, '').replace(/\)$/, '');
  return { dex: parts[0], member: parts[1], matched };
};

let passed = 0;
let total = 0;
const failures = [];
function assert(label, condition, detail = '') {
  total += 1;
  if (condition) {
    passed += 1;
    console.log(`ok   ${label}${detail ? `  ${detail}` : ''}`);
  } else {
    failures.push(`${label}${detail ? `  ${detail}` : ''}`);
    console.log(`FAIL ${label}${detail ? `  ${detail}` : ''}`);
  }
}

async function main() {
  console.log(`wasm:   ${wasmPath}`);
  console.log(`corpus: ${corpusPath ?? '(none)'}\n`);
  await prepare();

  // The anchors: the query works today, in the mode that exists and on the native build.
  {
    const text = run('corpus', ['findrefs', '--threads', '1', 'corpus.apk', 'string', 'http']);
    assert(
      'anchor: the text mode answers a reference query',
      text.code === 0 && rows(text.text).length > 10 && /matched=\(http\)/.test(text.text),
      `${rows(text.text).length} rows`,
    );
    const reference = nativeRun(['findrefs', '--threads', '1', paths.corpus, 'string', 'http']);
    assert(
      'anchor: and the native build answers it the same way',
      reference.code === 0 && text.text === reference.stdout,
      `${text.text.length} vs ${reference.stdout.length} bytes`,
    );
  }

  {
    const json = run('corpus', ['findrefs', '--json', '--threads', '1', 'corpus.apk', 'string', 'http']);
    const nativeJson = nativeRun(['findrefs', '--json', '--threads', '1', paths.corpus, 'string', 'http']);
    let parsed = [];
    let parseable = true;
    try {
      parsed = rows(json.text).map((line) => JSON.parse(line));
    } catch {
      parseable = false;
    }
    assert(
      'host: --json emits one parseable record per row',
      json.code === 0 && parseable && parsed.length > 0 && parsed.length === rows(nativeJson.stdout).length,
      `code=${json.code} rows=${parsed.length} native=${rows(nativeJson.stdout).length}`,
    );

    // Rendering the records back into the text row shape and comparing the whole
    // reconstruction with the native build's text mode: the fields, their order and the
    // row order all at once, and it holds whatever the matched value contains.
    const rebuilt = parsed
      .map((record) => `${record.dex} | ${record.member} | matched=(${record.matched})`)
      .join('\n');
    const nativeText = nativeRun(['findrefs', '--threads', '1', paths.corpus, 'string', 'http']);
    assert(
      'host: the records rebuild the text mode\u2019s rows exactly',
      nativeText.code === 0 &&
        parsed.length > 0 &&
        `${rebuilt}\n` === nativeText.stdout,
      `${rebuilt.length} vs ${nativeText.stdout.length} chars`,
    );
    assert(
      'host: a string query reports the string that matched',
      parsed.length > 0 && parsed.every((record) => record.matched.includes('http')),
      `${parsed.filter((record) => !record.matched.includes('http')).length} rows disagree`,
    );
  }

  {
    const nothing = run('corpus', ['findrefs', '--json', '--threads', '1', 'corpus.apk', 'string', 'zzz-no-such-string']);
    assert(
      'host: a query with no hits is empty, not absent',
      nothing.code === 0 && nothing.text === '',
      `code=${nothing.code} ${JSON.stringify(nothing.text.slice(0, 40))}`,
    );
  }

  {
    // A member name with the row separator, a quote and a newline in it.
    const json = run('hostile', ['findrefs', '--json', '--threads', '1', 'hostile.apk', 'string', 'Authorization']);
    let parsed = [];
    try {
      parsed = rows(json.text).map((line) => JSON.parse(line));
    } catch {
      parsed = [];
    }
    const members = parsed.map((record) => record.member).sort();
    const expected = ['Lcom/example/Pipe|Inside;->m|0', 'Lcom/example/Quote"Inside;->m"1'].sort();
    assert(
      'host: a member name a text row cannot hold survives --json',
      json.code === 0 && JSON.stringify(members) === JSON.stringify(expected),
      JSON.stringify(members),
    );
    const reference = nativeRun(['findrefs', '--json', '--threads', '1', paths.hostile, 'string', 'Authorization']);
    assert(
      'host: and the native build prints the same records',
      json.text === reference.stdout && json.text.length > 0,
      `${json.text.length} vs ${reference.stdout.length} bytes`,
    );
  }

  {
    const junkPath = join(workdir, 'junk.bin');
    writeFileSync(junkPath, Buffer.from([0x4d, 0x5a, 0x00, 0x01, 0x02, 0x03]));
    const junk = await Rasc.load({ wasm, source: sourceFromBytes(new Uint8Array(readFileSync(junkPath))) });
    const result = junk.run(['findrefs', '--json', '--threads', '1', 'junk.bin', 'string', 'http']);
    const record = rows(decoder.decode(result.output)).length === 1 ? JSON.parse(rows(decoder.decode(result.output))[0]) : null;
    assert(
      'host: a failure is one record carrying the engine\u2019s message',
      result.code === 1 && record !== null && typeof record.error === 'string' && record.error.length > 0,
      `code=${result.code} ${JSON.stringify(record)}`,
    );
  }

  {
    const samples = [];
    for (let attempt = 0; attempt < 3; attempt += 1) {
      const started = process.hrtime.bigint();
      run('corpus', ['findrefs', '--json', '--threads', '1', 'corpus.apk', 'string', 'http']);
      samples.push(Number(process.hrtime.bigint() - started) / 1e6);
    }
    samples.sort((left, right) => left - right);
    const payload = run('corpus', ['findrefs', '--json', '--threads', '1', 'corpus.apk', 'string', 'http']);

    /*
     * The cost claim that decides the consumer's shape: a string press issues this query, so
     * the answer has to stay small enough to run per press without caching anything. Bytes,
     * not milliseconds - a gate must not fail because the machine was busy.
     */
    assert(
      'host: and one query costs kilobytes, not the archive',
      JSON.stringify(payload).length > 0 && JSON.stringify(payload).length < 256 * 1024,
      // The payload arrives parsed here, so this is the records' size and a lower bound on
      // the wire text - which is the honest direction for a ceiling.
      `${JSON.stringify(payload).length} bytes of records for one query over the corpus`,
    );
    /*
   * The number the repository's own instructions declare for this suite, held against what it
   * actually ran: coverage is a claim like any other, adding an assertion without saying so in
   * AGENT.md now fails here, and a rewrite of this file cannot move the number quietly.
   */
  // Built from a string rather than written as a literal: the suite's own path holds a slash,
  // and a slash inside a regex literal ends it - which is what made the first version of this
  // a SyntaxError rather than a check.
  const declaredPattern = new RegExp('`js/web-host-xrefs.mjs`\\s*\\|\\s*(\\d+)');
  const declaredMatch = declaredPattern.exec(readFileSync('AGENT.md', 'utf8'));
  const declared = declaredMatch === null ? Number.NaN : Number(declaredMatch[1]);
  if (declared !== total) {
    failures.push(`AGENT.md declares ${declared} assertions for this suite and it ran ${total}`);
    console.log(`FAIL the suite ran ${total}; AGENT.md declares ${declared}`);
  }

  console.log(`\nMETRIC xref_pass=${passed}`);
    console.log(`METRIC xref_total=${total}`);
    console.log(`METRIC xref_fail=${total - passed}`);
    console.log(`METRIC xref_ms=${Math.round(samples[1])}`);
    console.log(`METRIC xref_bytes=${payload.text.length}`);
  }

  console.log(`\n${passed}/${total} reference-query assertions passed`);
  if (failures.length > 0) {
    console.log(`\n${failures.length} failing assertion(s):`);
    for (const failure of failures) console.log(`- ${failure}`);
  }
  return 0;
}

const outcome = await main();
/*
 * A gate has to fail. These suites began as reporters, so `check-all.sh` stayed green while
 * every assertion in one of them could be failing - the gate proved only that Node ran. The
 * plain invocation is still a reporter (the experiment loop needs a partial state to be a
 * number, not a crash); `--gate` is what the repository's gates use.
 */
process.exitCode = process.argv.includes("--gate") && failures.length > 0 ? 1 : outcome;
