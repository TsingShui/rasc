#!/usr/bin/env node
/**
 * Reference counts per string, measured at the host boundary.
 *
 * The strings table has a column for how many places use each value, and a Java unit shows
 * a dash there because a count per row is a scan of the whole archive (22 ms measured) and
 * a page of five hundred rows cannot pay that five hundred times. One pass can count every
 * string at once, and this is the contract for that pass: `strings --json --xrefs` emits
 * `{"dex":…,"index":…,"count":…}` per *referenced* string.
 *
 * The rule it pins hardest, and the one an implementation is most likely to get subtly
 * wrong, is that **a count equals the number of rows `findrefs` reports for the same
 * value** - one per method that uses it, so a value used twice in one method counts once.
 * Nothing here is a literal: every count is checked against the engine's own answer.
 *
 * Two values a reference *query* cannot express, and which the corpus sample therefore
 * skips rather than pretending to check: the empty string (no query means "the empty
 * string") and any value holding NUL (it cannot be passed as an argument). The census
 * counts both; the comparison cannot.
 *
 *   node js/web-host-counts.mjs
 *   RASC_WASM=... RASC_SUITE_APK=... node js/web-host-counts.mjs
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Rasc, sourceFromBytes } from './rasc.mjs';
import { DEFAULT_WASM } from './node.mjs';
import { apkOf, fixtureDex, fixtureStrings } from './fixtures.mjs';

const wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM;
const native = process.env.RASC_BIN ?? 'target/release/rasc';
const wasm = readFileSync(wasmPath);
const decoder = new TextDecoder();

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

/** Four classes, each with one method whose body loads the same string. */
const CLASSES = 4;
const fixture = fixtureDex({ classCount: CLASSES, value: 'Authorization' });
const fixtureTable = fixtureStrings({ classCount: CLASSES });

const workdir = mkdtempSync(join(tmpdir(), 'rasc-counts-'));
const paths = {
  corpus: corpusPath,
  fixture: (() => {
    const path = join(workdir, 'fixture.apk');
    writeFileSync(path, apkOf(fixture));
    return path;
  })(),
};

const instances = new Map();
async function prepare() {
  for (const [id, bytes] of [['corpus', corpus], ['fixture', fixture]]) {
    if (bytes !== null) instances.set(id, await Rasc.load({ wasm, source: sourceFromBytes(bytes) }));
  }
}

function run(id, args) {
  const rasc = instances.get(id);
  if (rasc === undefined) throw new Error(`no fixture named ${id}`);
  const chunks = [];
  try {
    const result = rasc.run(args, { onOutput: (chunk) => chunks.push(chunk) });
    return { ...result, text: decoder.decode(concat(chunks)) };
  } catch (error) {
    return { code: null, text: '', trapped: true, error: String(error) };
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
const parse = (text) => {
  try {
    return rows(text).map((line) => JSON.parse(line));
  } catch {
    return [];
  }
};
/** How many rows the engine reports for one string: one per method that uses it. */
const referenceRows = (id, value) =>
  rows(run(id, ['findrefs', '--json', '--threads', '1', `${id}.apk`, 'string', value]).text).length;
/** A value a reference query can carry as an argument at all. */
const queryable = (value) => value !== '' && !value.includes('\0');

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

  // Anchors: the two answers a count has to agree with already exist.
  const table = run('fixture', ['strings', '--json', 'fixture.apk']);
  assert(
    'anchor: the string table lists the fixture',
    table.code === 0 && parse(table.text).length === fixtureTable.length,
    `${parse(table.text).length} of ${fixtureTable.length}`,
  );
  assert(
    'anchor: and the engine answers who uses one of its strings',
    referenceRows('fixture', 'Authorization') === CLASSES,
    `${referenceRows('fixture', 'Authorization')} rows for ${CLASSES} methods`,
  );

  {
    const counts = run('fixture', ['strings', '--json', '--xrefs', 'fixture.apk']);
    const records = parse(counts.text);
    assert(
      'host: --xrefs emits one parseable record per referenced string',
      counts.code === 0 && records.length > 0 && records.length <= fixtureTable.length,
      `code=${counts.code} rows=${records.length}`,
    );
    assert(
      'host: every record names a string that is in the table',
      records.length > 0 &&
        records.every(
          (record) =>
            typeof record.dex === 'string' &&
            Number.isInteger(record.index) &&
            typeof record.count === 'number' &&
            record.count >= 1 &&
            fixtureTable[record.index] !== undefined,
        ),
      `${records.length} records`,
    );

    const wrong = records.filter((record) => record.count !== referenceRows('fixture', fixtureTable[record.index]));
    assert(
      'host: a count is the number of methods findrefs reports',
      records.length > 0 && wrong.length === 0,
      `${wrong.length} disagree, expected ${CLASSES} for each`,
    );

    // A string nothing loads is absent, not zero: a zero in the column is a claim. The
    // non-empty payload is load-bearing - an empty one satisfies "it is not in there"
    // without the command having answered anything.
    const unreferenced = fixtureTable.findIndex((value) => value !== 'Authorization');
    assert(
      'host: a string nobody references is absent rather than zero',
      records.length > 0 &&
        unreferenced !== -1 &&
        !records.some((record) => record.index === unreferenced),
      `index ${unreferenced} = ${JSON.stringify(fixtureTable[unreferenced])}`,
    );
  }

  {
    const json = run('fixture', ['strings', '--json', '--xrefs', 'fixture.apk']);
    const reference = nativeRun(['strings', '--json', '--xrefs', paths.fixture]);
    assert(
      'host: and the native build prints the same records',
      json.code === 0 && json.text.length > 0 && json.text === reference.stdout,
      `${json.text.length} vs ${reference.stdout.length} bytes`,
    );
  }

  if (corpus !== null) {
    const counts = parse(run('corpus', ['strings', '--json', '--xrefs', 'corpus.apk']).text);
    // The whole table, because the oracle below needs a *global* fact about each candidate
    // value rather than a fact about a page of it.
    const table = parse(run('corpus', ['strings', '--json', 'corpus.apk']).text);
    const byId = new Map(table.map((record) => [`${record.dex}:${record.index}`, record.value]));
    /*
     * `findrefs string X` reports a row per method that references any string *containing*
     * X, so it answers for one string index only when no other string contains X - which is
     * true of the fixture and not of a real table's short values. The sample is therefore
     * restricted to values that are unique and are not a substring of any other value; the
     * census counts every string, the comparison can only speak for those.
     */
    const values = table.map((record) => record.value);
    const safe = (value) => values.filter((other) => other !== value && other.includes(value)).length === 0;
    /*
     * Candidates are taken *before* the uniqueness scan, not after: the scan is over the
     * whole table (496,435 values on the 126 MB corpus), so testing every record first made
     * this suite take eight minutes there - a benchmark nobody would run, and one that says
     * nothing about the engine. A handful of candidates costs a few million comparisons.
     */
    const sample = counts
      .map((record) => ({ record, value: byId.get(`${record.dex}:${record.index}`) ?? '' }))
      .filter((candidate) => queryable(candidate.value) && candidate.value.length > 8)
      .slice(0, 8)
      .filter((candidate) => safe(candidate.value))
      .slice(0, 3)
      .map((candidate) => candidate.record);
    const wrong = sample.filter(
      (record) => record.count !== referenceRows('corpus', byId.get(`${record.dex}:${record.index}`)),
    );
    /*
     * The design assumption, checked rather than remembered: a host fetches this when a table
     * opens, which is only reasonable while it is a small part of the table it annotates. If
     * it ever approached the table's size, the consumer shape would be wrong and this is
     * where that should be noticed rather than in a note.
     */
    assert(
      'host: and the census is a fraction of the table it annotates',
      table.length > 0 && counts.length * 4 < table.length,
      `${counts.length} counts against ${table.length} strings`,
    );

    assert(
      'host: a real archive\u2019s counts agree with findrefs too',
      sample.length > 0 && wrong.length === 0,
      `${sample.length} sampled, ${wrong.length} disagree`,
    );
  } else {
    assert('host: a real archive\u2019s counts agree with findrefs too', false, 'no corpus');
  }

  {
    const junk = await Rasc.load({ wasm, source: sourceFromBytes(new Uint8Array([0x4d, 0x5a, 0, 1, 2, 3])) });
    const result = junk.run(['strings', '--json', '--xrefs', 'junk.bin']);
    const records = parse(decoder.decode(result.output));
    assert(
      'host: a failure is one record carrying the engine\u2019s message',
      result.code === 1 && records.length === 1 && typeof records[0].error === 'string',
      `code=${result.code} ${JSON.stringify(records[0] ?? null)}`,
    );
  }

  /*
   * The number the repository's own instructions declare for this suite, held against what it
   * actually ran: coverage is a claim like any other, adding an assertion without saying so in
   * AGENT.md now fails here, and a rewrite of this file cannot move the number quietly.
   */
  // Built from a string rather than written as a literal: the suite's own path holds a slash,
  // and a slash inside a regex literal ends it - which is what made the first version of this
  // a SyntaxError rather than a check.
  const declaredPattern = new RegExp('`js/web-host-counts.mjs`\\s*\\|\\s*(\\d+)');
  const declaredMatch = declaredPattern.exec(readFileSync('AGENT.md', 'utf8'));
  const declared = declaredMatch === null ? Number.NaN : Number(declaredMatch[1]);
  if (declared !== total) {
    failures.push(`AGENT.md declares ${declared} assertions for this suite and it ran ${total}`);
    console.log(`FAIL the suite ran ${total}; AGENT.md declares ${declared}`);
  }

  console.log(`\nMETRIC counts_pass=${passed}`);
  console.log(`METRIC counts_total=${total}`);
  console.log(`METRIC counts_fail=${total - passed}`);
  console.log(`\n${passed}/${total} reference-count assertions passed`);
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
