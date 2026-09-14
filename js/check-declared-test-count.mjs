#!/usr/bin/env node
/**
 * The gate table's test count, held against what `cargo test` reports.
 *
 * `AGENT.md` describes every gate in one table, and the row beside `cargo test` states a
 * number - which is a claim about coverage like the suites' own assertion counts, and it
 * drifted for most of this session (83 + 2 while the command printed 100 + 2). The suites
 * check their numbers themselves; this one is checked here, because it is the only gate whose
 * number is a count of tests rather than of assertions.
 *
 *   node js/check-declared-test-count.mjs --gate
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const row = /\|\s*`cargo test`\s*\|\s*(\d+) \+ (\d+) unit\/integration tests\s*\|/.exec(
  readFileSync('AGENT.md', 'utf8'),
);

if (row === null) {
  console.error('declared test count: AGENT.md has no `cargo test` row to compare against');
  process.exit(1);
}

const output = execFileSync('cargo', ['test', '--quiet'], { encoding: 'utf8' });
const totals = [...output.matchAll(/test result: ok\. (\d+) passed/g)].map((match) => Number(match[1]));
// `cargo test` prints one result line per test binary: the crate's unit tests first, then one
// per file in `tests/`. The declared figure is "unit + integration", so the integration side is
// the sum of the rest - the first version assumed exactly one integration binary and failed the
// moment a second test file existed, which is the good kind of failure.
const [unit, ...integrationBins] = totals;
const integration = integrationBins.reduce((sum, n) => sum + n, 0);

if (totals.length < 2 || unit !== Number(row[1]) || integration !== Number(row[2])) {
  console.error(
    `declared test count: AGENT.md says ${row[1]} + ${row[2]}, cargo test reports ` +
      `${[unit, integration].join(' + ')} from ${totals.length} binaries`, 
  );
  process.exit(1);
}

console.log(`declared test count: ${unit} + ${integration}, which is what cargo test reports`);
