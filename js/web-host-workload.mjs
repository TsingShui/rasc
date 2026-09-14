#!/usr/bin/env node
/**
 * What a browser host has to move to open a Java unit and search its strings.
 *
 * The host boundary suite (`web-host-test.mjs`) asks whether a host *can* read what
 * the engine prints. This asks what reading it *costs*: on a 126 MB APK the class
 * index is 15 MB of records and the string table is 69 MB, so a host that fetches the
 * table to populate a list has moved 85 MB before anyone has typed a search. That is
 * the workload this measures - the sequence a real host performs - and the metric is
 * its total payload, because payload is what crosses a Worker boundary and what the
 * page then holds.
 *
 * The sequence is what Repi's adapter does, in its order:
 *
 *   1. `classes --json`  - the column
 *   2. `entries --json`  - the readout
 *   3. the string count  - the navigator's number (today: the whole table, to count it)
 *   4. a searched page   - what someone sees after typing into the filter
 *
 * Steps 3 and 4 are the ones under test: a host should be able to ask how many strings
 * there are, and for the ones it wants, without receiving all of them. Until those
 * flags exist the script falls back to the full table for both, which is the baseline
 * this session starts from.
 *
 *   node js/web-host-workload.mjs
 *   RASC_WASM=... RASC_SUITE_APK=... node js/web-host-workload.mjs
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { Rasc, sourceFromBytes } from './rasc.mjs';
import { DEFAULT_WASM } from './node.mjs';

const wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM;
const wasm = readFileSync(wasmPath);
const corpus = findCorpus();
const decoder = new TextDecoder();

/** The largest cached corpus: the workload that hurts. */
function findCorpus() {
  if (process.env.RASC_SUITE_APK) return process.env.RASC_SUITE_APK;
  if (!existsSync('.cache/corpus')) return null;
  const files = readdirSync('.cache/corpus')
    .filter((name) => name.endsWith('.apk'))
    .map((name) => `.cache/corpus/${name}`)
    .sort((left, right) => statSync(right).size - statSync(left).size);
  return files[0] ?? null;
}

if (corpus === null) {
  console.error('web-host-workload: no corpus (set RASC_SUITE_APK or fetch one)');
  process.exit(1);
}

const bytes = new Uint8Array(readFileSync(corpus));
const rasc = await Rasc.load({ wasm, source: sourceFromBytes(bytes) });

/**
 * How many times each step runs; the reported time is the median.
 *
 * A single run of this sequence varies by up to 8% with the machine's mood, which is
 * more than some of the effects this benchmark is asked to resolve: a controlled
 * before/after measured an 11 ms change that one pair of single runs reported as 31 ms.
 * Bytes are exact and take the last run; only the clock is repeated.
 */
const REPEATS = Number(process.env.RASC_SUITE_REPEATS ?? 3);

/**
 * Runs one command and reports what it cost, streaming so nothing is held twice.
 *
 * `keep` also collects the payload, which is only for a step small enough to inspect -
 * the searched page - because holding it is exactly what the metric is about.
 */
function weigh(args, keep = false) {
  const samples = [];
  let payload = 0;
  let chunks = [];
  let last = null;
  for (let attempt = 0; attempt < REPEATS; attempt += 1) {
    payload = 0;
    chunks = [];
    const started = process.hrtime.bigint();
    last = rasc.run(args, {
      onOutput: (chunk) => {
        payload += chunk.length;
        if (keep) chunks.push(chunk);
      },
    });
    samples.push(Number(process.hrtime.bigint() - started) / 1e6);
  }
  samples.sort((left, right) => left - right);
  return { ...last, payload, ms: samples[Math.floor(samples.length / 2)], chunks, samples };
}

/** Does this build know a flag at all? A host asks before it relies on one. */
function supports(args) {
  const probe = rasc.run(args);
  return probe.code === 0;
}

// `reused` marks a step the host answered from bytes it already had: the payload was
// paid for once, and counting it again would make the metric about this script.
const steps = [];
steps.push(['classes', weigh(['classes', '--json', 'corpus.apk']), false]);
steps.push(['entries', weigh(['entries', '--json', 'corpus.apk']), false]);

const countArgs = ['strings', '--json', '--count', 'corpus.apk'];
const counted = supports(countArgs) ? weigh(countArgs) : null;
let wholeTable = null;
if (counted !== null) {
  steps.push(['count', counted, false]);
} else {
  // No count flag: the only way to know how many strings there are is to receive them,
  // and a host that has received them keeps them - so this one fetch stands in for both
  // the count and the page, which is what today's host does.
  wholeTable = weigh(['strings', '--json', 'corpus.apk']);
  steps.push(['count (whole table)', wholeTable, false]);
}

const needle = process.env.RASC_SUITE_NEEDLE ?? 'http';
const pageArgs = ['strings', '--json', '--filter', needle, '--limit', '200', 'corpus.apk'];
const hasFilter = supports(pageArgs);
const page = hasFilter ? weigh(pageArgs, true) : (wholeTable ?? weigh(['strings', '--json', 'corpus.apk'], true));
steps.push([
  hasFilter ? `page "${needle}"` : `page "${needle}" (whole table)`,
  page,
  !hasFilter && wholeTable !== null,
]);

const table = wholeTable ?? weigh(['strings', '--json', 'corpus.apk']);

/*
 * The census, which fills the strings table's counts column. It is measured here because the
 * shape of that design rests on it being a *fraction* of the table: if it ever approached the
 * table's size, a tab fetching it on open would be the wrong design and a per-page query the
 * right one. That claim was a one-off measurement until now.
 */
const census = weigh(['strings', '--json', '--xrefs', 'corpus.apk']);

let openBytes = 0;
let openMs = 0;
for (const [label, step, reused] of steps) {
  if (!reused) {
    openBytes += step.payload;
    openMs += step.ms;
  }
  console.log(
    `${label.padEnd(28)} ${step.ms.toFixed(0).padStart(6)} ms  ${(step.payload / 1e6).toFixed(2).padStart(8)} MB` +
      `  [${step.samples.map((sample) => sample.toFixed(0)).join(' ')}]` +
      (reused ? '  (from bytes already held)' : '') + (step.code === 0 ? '' : `  code=${step.code}`),
  );
}

/** The page's own sanity: a searched page holds the needle, or the flags lie. */
let pageIsHonest = true;
let pageRows = 0;
if (page.code === 0) {
  const payload = Buffer.concat(page.chunks.map((chunk) => Buffer.from(chunk))).toString('utf8');
  for (const line of payload.split('\n')) {
    if (line.length === 0) continue;
    pageRows += 1;
    try {
      const record = JSON.parse(line);
      if (typeof record.value === 'string' && !record.value.toLowerCase().includes(needle)) pageIsHonest = false;
    } catch {
      pageIsHonest = false;
    }
  }
}
if (pageRows === 0) pageIsHonest = false;

console.log(`\ncorpus: ${corpus}`);
console.log(`whole table: ${(table.payload / 1e6).toFixed(2)} MB in ${table.ms.toFixed(0)} ms`);
console.log(
  `census:      ${(census.payload / 1e6).toFixed(2)} MB in ${census.ms.toFixed(0)} ms ` +
    `(${((census.payload / table.payload) * 100).toFixed(1)}% of the table, which is what the design assumes)`,
);
console.log(`\nMETRIC open_bytes=${openBytes}`);
console.log(`METRIC open_ms=${Math.round(openMs)}`);
console.log(`METRIC strings_bytes=${table.payload}`);
console.log(`METRIC census_bytes=${census.payload}`);
console.log(`METRIC census_ms=${Math.round(census.ms)}`);
console.log(`METRIC strings_ms=${Math.round(table.ms)}`);
console.log(`METRIC page_rows=${pageRows}`);
console.log(`METRIC page_honest=${pageIsHonest ? 1 : 0}`);
