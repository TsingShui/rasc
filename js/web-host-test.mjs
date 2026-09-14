#!/usr/bin/env node
/**
 * The web host boundary, measured.
 *
 * Repi's browser application is a JS host of this module, and a host is only as
 * convenient as the boundary it is handed. This suite is the check on that
 * boundary: every assertion is something a browser host should not have to do
 * itself, and each is answered against an independent oracle - the native binary,
 * the module's own text mode, the DEX header, the ZIP central directory, or a
 * second implementation of the rule in this file.
 *
 * It is deliberately separate from `test.mjs`: that one asks "do the two hosts
 * agree on the commands that exist", and this one asks "can a host use them at
 * all". The pass count is a session metric, so the assertions are frozen:
 * adding or loosening them after optimizing would be measuring the measurement.
 *
 * One exception is recorded here rather than quietly taken. The outline scenario's
 * first version asked the members to tile the whole document *and* to be exactly the
 * declarations a reader can see, which no answer satisfies: a class document starts
 * with its package line and ends with its closing brace, and neither is a
 * declaration. The tiling assertion is unchanged and still exact; the name assertion
 * now compares declarations only, and the two structural kinds are named so that it
 * can tell them apart. Without this the scenario was unsatisfiable, not hard.
 *
 *   node js/web-host-test.mjs
 *   RASC_WASM=... RASC_SUITE_APK=... node js/web-host-test.mjs
 *   node js/web-host-test.mjs --list
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Rasc, sourceFromBytes } from './rasc.mjs';
import { DEFAULT_WASM } from './node.mjs';
import {
  apkOf,
  centralDirectory,
  fixtureDex,
  fixtureStrings,
  junkFile,
  makeArchive,
  multiEntryApk,
} from './fixtures.mjs';

const wasmPath = process.env.RASC_WASM ?? DEFAULT_WASM;
const native = process.env.RASC_BIN ?? 'target/release/rasc';
const wasm = readFileSync(wasmPath);
const decoder = new TextDecoder();

/** The smallest cached corpus, which is the one the per-run assertions use. */
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

// ------------------------------------------------------------------ fixtures

const plain = fixtureDex({ classCount: 4 });
const plainApk = apkOf(plain);
const multi = multiEntryApk(8);
const junk = junkFile();
const hostileDescriptor = (index) =>
  [
    'Lcom/example/Pipe|Inside;',
    'Lcom/example/Quote"Inside;',
    'Lcom/example/Back\\slash;',
    'Lcom/example/New\nLine;',
    'Lcom/example/Emoji🙂Value;',
  ][index];
const hostileMethod = (index) => ['a|b', 'c"d', 'e\nf', 'g\\h', 'i🙂j'][index];
const hostile = fixtureDex({ classCount: 5, descriptor: hostileDescriptor, methodName: hostileMethod });
const unicodeValue = 'ключ 🔑 漢字\u00e9';
const large = fixtureDex({ classCount: 12000 });
const largeApk = apkOf(large);
const corpusPath = findCorpus();

/**
 * The fixtures are written out as well as held in memory.
 *
 * The host build ignores the archive argument - the host owns the bytes - but the
 * native binary does not, and the native binary is the oracle for half the
 * assertions. So the same bytes exist under a path, and the native runs are
 * pointed at the file while the host runs are pointed at the label.
 */
const workdir = mkdtempSync(join(tmpdir(), 'rasc-host-suite-'));
const fixturePath = (name, bytes) => {
  const path = join(workdir, name);
  writeFileSync(path, bytes);
  return path;
};
const paths = {
  plainApk: fixturePath('plain.apk', plainApk),
  plainDex: fixturePath('plain.dex', plain),
  largeApk: fixturePath('large.apk', largeApk),
};

const fixtures = new Map();
const corpusBytes = corpusPath === null ? null : new Uint8Array(readFileSync(corpusPath));

/** Loads one instance per fixture once; a run never reloads the archive. */
async function prepare() {
  const empty = makeArchive([['AndroidManifest.xml', new TextEncoder().encode('<manifest />')]]);
  const unicode = fixtureDex({ classCount: 2, value: unicodeValue, extraStrings: ['🙂', '漢字'] });
  const entries = [
    ['plain', plain],
    ['plainApk', plainApk],
    ['largeApk', largeApk],
    ['multi', multi],
    ['hostile', hostile],
    ['junk', junk],
    ['empty', empty],
    ['unicode', unicode],
  ];
  if (corpusBytes !== null) entries.push(['corpus', corpusBytes]);
  for (const [id, bytes] of entries) {
    fixtures.set(id, await Rasc.load({ wasm, source: sourceFromBytes(bytes) }));
  }
}

async function freshInstance(bytes) {
  return Rasc.load({ wasm, source: sourceFromBytes(bytes) });
}

/**
 * Runs one command.
 *
 * A trap is a result, not an exception: the host has to survive one (the wasm
 * instance does not), and an assertion about a trap should be able to see it.
 */
function runOn(rasc, args, options = {}) {
  const chunks = [];
  const progress = [];
  try {
    const result = rasc.run(args, {
      onOutput: (chunk) => {
        chunks.push(chunk);
        options.onChunk?.(chunk);
      },
      onProgress: (done, count) => progress.push([done, count]),
    });
    return {
      ...result,
      chunks,
      progress,
      memoryBytes: rasc.wasmMemoryBytes,
      trapped: false,
      text: decoder.decode(concat(chunks)),
    };
  } catch (error) {
    return {
      code: null,
      output: new Uint8Array(0),
      reads: 0,
      bytes: 0,
      fetches: 0,
      memoryBytes: 0,
      chunks: [],
      progress,
      trapped: true,
      text: decoder.decode(concat(chunks)),
      error: String(error),
    };
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

/** Runs against a prepared fixture, or against a fresh instance when asked. */
function run(id, args, options = {}) {
  const rasc = fixtures.get(id);
  if (rasc === undefined) throw new Error(`no fixture named ${id}`);
  return runOn(rasc, args, options);
}

let freshRun = null;
async function prepareFresh() {
  const rasc = await freshInstance(plain);
  freshRun = (args) => runOn(rasc, args);
}

/** The native binary's answer for a file, for the assertions that compare hosts. */
function nativeRun(args) {
  try {
    const output = execFileSync(native, args, { maxBuffer: 1 << 30 });
    return { code: 0, stdout: output.toString('utf8'), stderr: '' };
  } catch (error) {
    return {
      code: error.status ?? 1,
      stdout: (error.stdout ?? Buffer.alloc(0)).toString('utf8'),
      stderr: (error.stderr ?? Buffer.alloc(0)).toString('utf8'),
    };
  }
}

const rows = (text) => text.split('\n').filter((line) => line.length > 0);
const records = (text) => rows(text).map((line) => JSON.parse(line));

// ----------------------------------------------------------------- scenarios

const scenarios = [];
const scenario = (name, expected, body) => scenarios.push({ name, expected, body });

scenario('bare-dex', 4, (assert) => {
  const wrapped = nativeRun(['classes', paths.plainApk]);
  const bare = run('plain', ['classes', 'plain.dex']);
  assert(
    'host: a bare DEX is indexed without a wrapper',
    bare.code === 0 && bare.text === wrapped.stdout,
    `code=${bare.code} ${bare.trapped ? bare.error : `${bare.text.length} bytes`}`,
  );

  const wrappedClass = nativeRun(['getclass', paths.plainApk, 'com.example.Fixture1']);
  const bareClass = run('plain', ['getclass', 'plain.dex', 'com.example.Fixture1']);
  assert(
    'host: getclass reads a class out of a bare DEX',
    bareClass.code === 0 && bareClass.text === wrappedClass.stdout,
    `code=${bareClass.code}`,
  );

  const wrappedRefs = nativeRun(['findrefs', paths.plainApk, 'string', 'Authorization']);
  const bareRefs = run('plain', ['findrefs', 'plain.dex', 'string', 'Authorization']);
  assert(
    'host: findrefs works on a bare DEX',
    bareRefs.code === 0 && bareRefs.text === wrappedRefs.stdout,
    `code=${bareRefs.code}`,
  );

  const junkRun = run('junk', ['classes', 'junk.bin']);
  assert(
    'host: a file that is neither DEX nor archive reports an error',
    junkRun.code === 1 && junkRun.text.startsWith('Error:'),
    `code=${junkRun.code} ${JSON.stringify(junkRun.text.slice(0, 40))}`,
  );
});

scenario('index-records', 5, (assert) => {
  const json = run('plain', ['classes', '--json', 'plain.apk']);
  let parsed = [];
  let allParsed = true;
  try {
    parsed = records(json.text);
  } catch {
    allParsed = false;
  }
  assert(
    'host: --json emits one parseable record per line',
    json.code === 0 && allParsed && parsed.length === 4,
    `code=${json.code} rows=${parsed.length}`,
  );

  const corpusText = corpusBytes === null ? null : run('corpus', ['classes', 'corpus.apk']);
  const corpusJson = corpusBytes === null ? null : run('corpus', ['classes', '--json', 'corpus.apk']);
  const corpusRecords = corpusJson === null ? [] : records(corpusJson.text);
  assert(
    'host: --json records the same rows as the text index',
    corpusText !== null &&
      corpusJson.code === 0 &&
      corpusRecords.length === rows(corpusText.text).length &&
      corpusRecords.length > 1000,
    corpusText === null ? 'no corpus' : `rows=${corpusRecords.length}`,
  );

  const textRows = corpusText === null ? [] : rows(corpusText.text).map((line) => line.split(' | '));
  const disagree = corpusRecords.findIndex(
    (record, index) =>
      textRows[index] === undefined ||
      textRows[index][1] !== record.descriptor ||
      textRows[index][2] !== record.name,
  );
  assert(
    'host: --json fields are the text index\u2019s columns',
    textRows.length > 0 && corpusRecords.length === textRows.length && disagree === -1,
    disagree === -1 ? `${corpusRecords.length} rows agree` : `row ${disagree}: ${JSON.stringify(corpusRecords[disagree])}`,
  );

  const hostileRecords = records(run('hostile', ['classes', '--json', 'hostile.apk']).text);
  const expected = Array.from({ length: 5 }, (_, index) => hostileDescriptor(index)).sort();
  const seen = hostileRecords.map((record) => record.descriptor).sort();
  assert(
    'host: a name a text row cannot hold survives --json',
    JSON.stringify(seen) === JSON.stringify(expected),
    `${seen.length} names`,
  );

  const empty = makeArchive([['AndroidManifest.xml', new TextEncoder().encode('<manifest />')]]);
  const emptyText = run('empty', ['classes', 'empty.apk']);
  const emptyJson = run('empty', ['classes', '--json', 'empty.apk']);
  assert(
    'host: an archive with no DEX is an empty list, not an error',
    emptyText.code === 0 && emptyText.text === '' && emptyJson.code === 0 && emptyJson.text === '',
    `code=${emptyText.code}`,
  );
});

scenario('error-records', 4, (assert) => {
  const text = run('plain', ['getclass', 'plain.apk', 'com.example.Missing']);
  const json = run('plain', ['getclass', '--json', 'plain.apk', 'com.example.Missing']);
  const parsed = rows(json.text).length === 1 ? JSON.parse(rows(json.text)[0]) : null;
  assert(
    'host: --json reports a failure as a record',
    json.code === 1 &&
      parsed !== null &&
      typeof parsed.error === 'string' &&
      parsed.error === text.text.trimEnd().replace(/^Error: /, ''),
    `code=${json.code} ${JSON.stringify(parsed)}`,
  );
  assert('host: a failure still exits non-zero under --json', json.code === 1, `code=${json.code}`);

  const bad = freshRun(['classes', '--nonsense', 'plain.apk']);
  assert(
    'host: an unknown flag is answered, not a trap',
    !bad.trapped && bad.code !== 0 && bad.text.length > 0,
    bad.trapped ? bad.error : `code=${bad.code} ${JSON.stringify(bad.text.slice(0, 60))}`,
  );

  const after = run('plain', ['classes', 'plain.apk']);
  assert(
    'host: and the instance still works afterwards',
    after.code === 0 && after.text === nativeRun(['classes', paths.plainApk]).stdout,
    `code=${after.code}`,
  );
});

scenario('class-outline', 5, (assert) => {
  const target = 'com.example.Fixture1';
  const source = run('plain', ['getclass', 'plain.apk', target]);
  const outlined = run('plain', ['getclass', '--outline', 'plain.apk', target]);
  const newline = outlined.text.indexOf('\n');
  const record = newline === -1 ? null : JSON.parse(outlined.text.slice(0, newline));
  const document = newline === -1 ? '' : outlined.text.slice(newline + 1);

  assert(
    'host: --outline leaves the document byte-identical',
    outlined.code === 0 && record !== null && document === source.text,
    `code=${outlined.code} ${record === null ? 'no record' : `${document.length} bytes`}`,
  );

  const members = Array.isArray(record?.members) ? record.members : [];
  assert(
    'host: every member has a name, a kind and a line range',
    members.length > 0 &&
      members.every(
        (member) =>
          typeof member.name === 'string' &&
          member.name.length > 0 &&
          typeof member.kind === 'string' &&
          Number.isInteger(member.start) &&
          Number.isInteger(member.end) &&
          member.start <= member.end,
      ),
    `${members.length} members`,
  );

  const lines = document.split('\n');
  while (lines.length > 0 && lines[lines.length - 1].trim() === '') lines.pop();
  assert(
    'host: the ranges partition the document exactly',
    members.length > 0 &&
      members[0].start === 1 &&
      members[members.length - 1].end === lines.length &&
      members.every((member, index) => index === 0 || member.start === members[index - 1].end + 1),
    `${members.length} members cover ${members[members.length - 1]?.end} of ${lines.length} lines`,
  );

  // A declaration, as opposed to the two structural parts of the document that no
  // declaration owns. The suite fixes those two words because it has to tell them
  // apart to compare names at all; every other kind is the engine's business.
  const declarations = members.filter((member) => member.kind !== 'header' && member.kind !== 'footer');
  const scanned = scanMembers(document).map((member) => member.name);
  assert(
    'host: the names are the ones a reader can see',
    members.length > 0 &&
      declarations.length > 0 &&
      JSON.stringify(declarations.map((member) => member.name)) === JSON.stringify(scanned),
    `engine=${declarations.map((member) => member.name).join(',')} scan=${scanned.join(',')}`,
  );

  const sample = corpusBytes === null ? null : outlineSample();
  assert(
    'host: the outline agrees with a scan on real classes',
    sample !== null && sample.checked >= 12 && sample.agree === sample.checked,
    sample === null ? 'no corpus' : `${sample.agree}/${sample.checked} classes agree`,
  );
});

scenario('progress', 3, (assert) => {
  const result = run('multi', ['classes', 'multi.apk']);
  assert(
    'host: progress is reported during a class index',
    result.progress.length >= 2,
    `${result.progress.length} callbacks`,
  );

  const monotonic = result.progress.every(
    (entry, index) => index === 0 || entry[0] >= result.progress[index - 1][0],
  );
  const last = result.progress[result.progress.length - 1] ?? [0, 0];
  assert(
    'host: progress is monotonic and reaches its total',
    monotonic && result.progress.length > 0 && last[0] === last[1] && last[0] > 0,
    `last=${last[0]}/${last[1]}`,
  );
  assert(
    'host: the total is the number of DEX entries walked',
    result.progress.length > 0 && result.progress.every((entry) => entry[1] === 8),
    `totals=${[...new Set(result.progress.map((entry) => entry[1]))].join(',')}`,
  );
});

scenario('entries', 4, (assert) => {
  const json = run('multi', ['entries', '--json', 'multi.apk']);
  const listed = json.code === 0 ? records(json.text) : [];
  const directory = centralDirectory(multi);

  assert(
    'host: entries lists every entry in directory order',
    json.code === 0 &&
      listed.length === directory.length &&
      JSON.stringify(listed.map((entry) => entry.name)) === JSON.stringify(directory.map((entry) => entry.name)),
    `code=${json.code} rows=${listed.length}`,
  );
  assert(
    'host: entries reports each entry\u2019s sizes',
    listed.length === directory.length &&
      listed.every(
        (entry, index) =>
          entry.uncompressed === directory[index].uncompressed &&
          entry.compressed === directory[index].compressed,
      ),
    `compared ${listed.length}`,
  );
  assert(
    'host: entries reports the compression method',
    listed.length === directory.length && listed.every((entry, index) => entry.method === directory[index].method),
    `methods=${[...new Set(listed.map((entry) => entry.method))].join(',')}`,
  );

  const bare = run('plain', ['entries', '--json', 'plain.dex']);
  const bareListed = bare.code === 0 ? records(bare.text) : [];
  assert(
    'host: a bare DEX reports itself as one entry',
    bare.code === 0 &&
      bareListed.length === 1 &&
      bareListed[0].name === 'classes.dex' &&
      bareListed[0].uncompressed === plain.length,
    `code=${bare.code} ${JSON.stringify(bareListed[0] ?? null)}`,
  );
});

scenario('strings', 4, (assert) => {
  const expected = fixtureStrings({ classCount: 4 });
  const json = run('plain', ['strings', '--json', 'plain.apk']);
  const listed = json.code === 0 ? records(json.text) : [];
  const headerCount = new DataView(plain.buffer, plain.byteOffset, plain.byteLength).getUint32(0x38, true);

  assert(
    'host: strings lists the whole string table',
    json.code === 0 && listed.length === headerCount,
    `code=${json.code} rows=${listed.length} header=${headerCount}`,
  );
  assert(
    'host: strings are in index order',
    listed.length === expected.length && listed.every((entry, index) => entry.value === expected[index]),
    `first=${JSON.stringify(listed[0]?.value ?? null)}`,
  );

  const hostileJson = run('hostile', ['strings', '--json', 'hostile.apk']);
  const hostileValues = hostileJson.code === 0 ? records(hostileJson.text).map((entry) => entry.value) : [];
  const hostileExpected = fixtureStrings({
    classCount: 5,
    descriptor: hostileDescriptor,
    methodName: hostileMethod,
  });
  assert(
    'host: a value a text row cannot hold survives --json',
    JSON.stringify(hostileValues) === JSON.stringify(hostileExpected),
    `${hostileValues.length}/${hostileExpected.length} values`,
  );

  const unicodeJson = run('unicode', ['strings', '--json', 'unicode.apk']);
  const unicodeValues = unicodeJson.code === 0 ? records(unicodeJson.text).map((entry) => entry.value) : [];
  assert(
    'host: MUTF-8 outside the BMP survives --json',
    unicodeJson.code === 0 && unicodeValues.includes(unicodeValue) && unicodeValues.includes('🙂'),
    `values=${JSON.stringify(unicodeValues.slice(0, 6))}`,
  );
});

scenario('streaming', 2, (assert) => {
  // 12,000 classes: past the renderer's 8,192-row chunk, so the payload is written
  // in more than one go and the host sees it as it is produced.
  const result = run('largeApk', ['classes', 'large.apk']);
  assert(
    'host: output arrives in chunks before the run ends',
    result.chunks.length >= 2,
    `${result.chunks.length} chunks`,
  );
  const reference = nativeRun(['classes', paths.largeApk]);
  assert(
    'host: the chunks are the payload',
    result.code === 0 &&
      result.text === reference.stdout &&
      result.output.length === 0,
    `chunks=${result.chunks.length} bytes=${result.chunks.reduce((sum, chunk) => sum + chunk.length, 0)} output=${result.output.length}`,
  );
});

scenario('reuse', 2, (assert) => {
  const classes = run('plainApk', ['classes', 'plain.apk']);
  const klass = run('plainApk', ['getclass', 'plain.apk', 'com.example.Fixture0']);
  const manifest = run('plainApk', ['manifest', 'plain.apk']);
  assert(
    'host: one instance answers several commands',
    classes.code === 0 && klass.code === 0 && manifest.code === 0 && manifest.text.includes('<manifest'),
    `codes=${classes.code},${klass.code},${manifest.code}`,
  );

  const missing = run('plainApk', ['getclass', 'plain.apk', 'com.example.Missing']);
  const again = run('plainApk', ['classes', 'plain.apk']);
  assert(
    'host: a command after a failure still works',
    missing.code === 1 && again.code === 0 && again.output.length === classes.output.length,
    `codes=${missing.code},${again.code}`,
  );
});

scenario('memory', 2, (assert) => {
  const small = run('plain', ['classes', 'plain.apk']);
  const bigger = run('largeApk', ['classes', 'large.apk']);
  assert('host: linear memory is reported with every run', small.memoryBytes > 0, `${small.memoryBytes} bytes`);
  assert(
    'host: memory grows with the workload',
    bigger.memoryBytes > small.memoryBytes,
    `${small.memoryBytes} -> ${bigger.memoryBytes}`,
  );
});

scenario('native-parity', 3, (assert) => {
  const compare = (args, nativeArgs) => {
    const host = run('plainApk', args);
    const reference = nativeRun(nativeArgs);
    return host.code === reference.code && host.text === reference.stdout;
  };
  assert(
    'host: classes matches the native binary byte for byte',
    compare(['classes', 'plain.apk'], ['classes', paths.plainApk]),
  );
  assert(
    'host: manifest matches the native binary byte for byte',
    compare(['manifest', 'plain.apk'], ['manifest', paths.plainApk]),
  );
  assert(
    'host: findrefs matches the native binary byte for byte',
    compare(
      ['findrefs', '--threads', '1', 'plain.apk', 'string', 'Authorization'],
      ['findrefs', '--threads', '1', paths.plainApk, 'string', 'Authorization'],
    ),
  );
});

scenario('cli-surface', 3, (assert) => {
  const help = freshRun(['--help']);
  const nativeHelp = nativeRun(['--help']);
  assert(
    'host: --help is the native help, byte for byte',
    !help.trapped && help.code === nativeHelp.code && help.text === nativeHelp.stdout,
    help.trapped ? help.error : `code=${help.code} ${help.text.length}/${nativeHelp.stdout.length} bytes`,
  );

  const version = freshRun(['--version']);
  const nativeVersion = nativeRun(['--version']);
  assert(
    'host: --version is the native version, byte for byte',
    !version.trapped && version.code === nativeVersion.code && version.text === nativeVersion.stdout,
    version.trapped ? version.error : `code=${version.code} ${JSON.stringify(version.text.trim())}`,
  );

  const unknown = freshRun(['nonsense']);
  const nativeUnknown = nativeRun(['nonsense']);
  assert(
    'host: an unknown command is answered like the native binary',
    !unknown.trapped && unknown.code === nativeUnknown.code && unknown.text === nativeUnknown.stderr,
    unknown.trapped
      ? unknown.error
      : `code=${unknown.code} vs ${nativeUnknown.code} ${JSON.stringify(unknown.text.slice(0, 40))}`,
  );
});

// ------------------------------------------------------- independent readings

/**
 * The member rule, written here as the outline's oracle.
 *
 * It is deliberately the simple reading: a depth-1 line that opens a brace starts
 * a member, a depth-1 line that ends in `;` and names a parameter list is one on
 * its own, and braces inside strings, characters and comments do not count.
 */
function scanMembers(source) {
  const lines = source.split('\n');
  const members = [];
  let depth = 0;
  let state = 'code';
  let open = null;

  for (const line of lines) {
    const before = depth;
    let opens = false;
    for (let index = 0; index < line.length; index += 1) {
      const character = line[index];
      const next = line[index + 1];
      if (state === 'code') {
        if (character === '/' && next === '/') {
          state = 'line';
          index += 1;
        } else if (character === '/' && next === '*') {
          state = 'block';
          index += 1;
        } else if (character === '"') state = 'string';
        else if (character === "'") state = 'char';
        else if (character === '{') {
          depth += 1;
          opens = true;
        } else if (character === '}') depth -= 1;
      } else if (state === 'string') {
        if (character === '\\') index += 1;
        else if (character === '"') state = 'code';
      } else if (state === 'char') {
        if (character === '\\') index += 1;
        else if (character === "'") state = 'code';
      } else if (state === 'block') {
        if (character === '*' && next === '/') {
          state = 'code';
          index += 1;
        }
      }
    }
    if (state === 'line') state = 'code';

    const declaration =
      before === 1 && depth === before && line.trimEnd().endsWith(';') && line.includes('(');
    if (open === null && before === 1 && (opens || declaration)) {
      open = name(line);
      if (!opens || depth === before) {
        members.push(open);
        open = null;
      }
      continue;
    }
    if (open !== null && depth === 1) {
      members.push(open);
      open = null;
    }
  }
  if (open !== null) members.push(open);
  return members.map((member) => ({ name: member }));

  function name(text) {
    const cut = text.indexOf('(') === -1 ? text.indexOf('{') : text.indexOf('(');
    const identifiers = [...text.slice(0, cut).matchAll(/[A-Za-z_$][\w$]*/g)];
    return identifiers.length === 0 ? text.trim() : identifiers[identifiers.length - 1][0];
  }
}

/** Outlines a sample of real classes and compares each with the scan. */
function outlineSample() {
  const index = run('corpus', ['classes', 'corpus.apk']).text;
  const names = rows(index).map((line) => line.split(' | ')[2]);
  const stride = Math.max(1, Math.floor(names.length / 15));
  let checked = 0;
  let agree = 0;
  for (let at = 0; at < names.length && checked < 15; at += stride) {
    const outlined = run('corpus', ['getclass', '--outline', 'corpus.apk', names[at]]);
    if (outlined.code !== 0) continue;
    const newline = outlined.text.indexOf('\n');
    if (newline === -1) continue;
    let record = null;
    try {
      record = JSON.parse(outlined.text.slice(0, newline));
    } catch {
      continue;
    }
    const scanned = scanMembers(outlined.text.slice(newline + 1)).map((member) => member.name);
    const declared = record.members
      .filter((member) => member.kind !== 'header' && member.kind !== 'footer')
      .map((member) => member.name);
    checked += 1;
    if (JSON.stringify(declared) === JSON.stringify(scanned)) agree += 1;
  }
  return { checked, agree };
}

// -------------------------------------------------------------------- report

let passed = 0;
let total = 0;
const failures = [];

async function main() {
  console.log(`wasm:   ${wasmPath}  (${Math.round(statSync(wasmPath).size / 1024)} KB)`);
  console.log(`native: ${native}${existsSync(native) ? '' : '  (missing)'}`);
  console.log(`corpus: ${corpusPath ?? '(none: corpus assertions will fail)'}\n`);

  if (process.argv.includes('--list')) {
    for (const { name, expected } of scenarios) console.log(`${name.padEnd(16)} ${expected} assertions`);
    return 0;
  }

  await prepare();
  await prepareFresh();

  const timed = corpusBytes === null ? null : run('corpus', ['classes', 'corpus.apk']);
  const indexMs = corpusBytes === null ? 0 : measureIndex();

  for (const { name, expected, body } of scenarios) {
    let reached = 0;
    const assert = (label, condition, detail = '') => {
      reached += 1;
      total += 1;
      if (condition) {
        passed += 1;
        console.log(`ok   ${label}${detail ? `  ${detail}` : ''}`);
      } else {
        failures.push(`${label}${detail ? `  ${detail}` : ''}`);
        console.log(`FAIL ${label}${detail ? `  ${detail}` : ''}`);
      }
    };

    console.log(`\n--- ${name}`);
    try {
      body(assert);
    } catch (error) {
      console.log(`FAIL ${name}: threw  ${String(error).split('\n')[0]}`);
      failures.push(`${name}: threw ${String(error).split('\n')[0]}`);
    }
    while (reached < expected) {
      reached += 1;
      total += 1;
      failures.push(`${name}: assertion ${reached} was not reached`);
      console.log(`FAIL ${name}: assertion ${reached} was not reached`);
    }
  }

  /*
   * The number the repository's own instructions declare for this suite, held against what it
   * actually ran: coverage is a claim like any other, adding an assertion without saying so in
   * AGENT.md now fails here, and a rewrite of this file cannot move the number quietly.
   */
  // Built from a string rather than written as a literal: the suite's own path holds a slash,
  // and a slash inside a regex literal ends it - which is what made the first version of this
  // a SyntaxError rather than a check.
  const declaredPattern = new RegExp('`js/web-host-test.mjs`\\s*\\|\\s*(\\d+)');
  const declaredMatch = declaredPattern.exec(readFileSync('AGENT.md', 'utf8'));
  const declared = declaredMatch === null ? Number.NaN : Number(declaredMatch[1]);
  if (declared !== total) {
    failures.push(`AGENT.md declares ${declared} assertions for this suite and it ran ${total}`);
    console.log(`FAIL the suite ran ${total}; AGENT.md declares ${declared}`);
  }

  console.log(`\nMETRIC web_host_pass=${passed}`);
  console.log(`METRIC web_host_total=${total}`);
  console.log(`METRIC web_host_fail=${total - passed}`);
  console.log(`METRIC wasm_kb=${Math.round(statSync(wasmPath).size / 1024)}`);
  console.log(`METRIC index_ms=${indexMs}`);
  const indexBytes = timed === null ? 0 : timed.chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  console.log(`METRIC index_bytes=${indexBytes}`);
  console.log(`\n${passed}/${total} host-boundary assertions passed`);
  if (failures.length > 0) {
    console.log(`\n${failures.length} failing assertion(s):`);
    for (const failure of failures) console.log(`- ${failure}`);
  }
  return 0;
}

/** Median of three corpus indexes, minus the first run's cold caches. */
function measureIndex() {
  const samples = [];
  for (let index = 0; index < 3; index += 1) {
    const started = process.hrtime.bigint();
    run('corpus', ['classes', 'corpus.apk']);
    samples.push(Number(process.hrtime.bigint() - started) / 1e6);
  }
  samples.sort((left, right) => left - right);
  return Math.round(samples[1]);
}

const outcome = await main();
/*
 * A gate has to fail. These suites began as reporters, so `check-all.sh` stayed green while
 * every assertion in one of them could be failing - the gate proved only that Node ran. The
 * plain invocation is still a reporter (the experiment loop needs a partial state to be a
 * number, not a crash); `--gate` is what the repository's gates use.
 */
process.exitCode = process.argv.includes("--gate") && failures.length > 0 ? 1 : outcome;
