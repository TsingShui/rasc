#!/usr/bin/env node
/**
 * End-to-end check of the browser host in a real browser.
 *
 *   APK=/path/to/app.apk node js/browser-test.mjs
 *
 * Serves the wasm module, the worker and the APK over http://127.0.0.1, opens the page in
 * headless Chrome, and compares each command's output with the native binary by SHA-256
 * (WebCrypto has no MD5, and shipping 61 MiB of base64 back would be worse).
 *
 * Two source modes are exercised, because a browser has two of them:
 *
 *   file - the user's local File, read with Blob.slice + FileReaderSync in the worker
 *   url  - a remote archive, read with synchronous XHR Range requests in the worker
 *
 * Chrome is located with CHROME= (default: the macOS app bundle) and the test skips when it
 * is missing or when APK is not set.
 */
import { createHash } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createReadStream, existsSync, mkdtempSync, readFileSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join } from 'node:path';

const apk = process.env.APK;
if (!apk || !existsSync(apk)) {
  console.log('skip: set APK=/path/to/app.apk');
  process.exit(0);
}
const chrome =
  process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
if (!existsSync(chrome)) {
  console.log(`skip: no Chrome at ${chrome} (set CHROME=)`);
  process.exit(0);
}

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

const commands = [
  { label: 'manifest', argv: ['manifest', apk] },
  { label: 'classes', argv: ['classes', '--threads', '1', apk] },
  { label: 'findrefs field', argv: ['findrefs', '--threads', '1', apk, 'field', 'INSTANCE'] },
  { label: `getclass ${late}`, argv: ['getclass', '--threads', '1', apk, late] },
];
const modes = (process.env.MODES ?? 'file,url').split(',').filter(Boolean);

/** Native reference: exit code, output size and SHA-256. */
function nativeRun(argv) {
  try {
    const out = execFileSync(native, argv, { maxBuffer: 1 << 30 });
    return { code: 0, bytes: out.length, sha256: createHash('sha256').update(out).digest('hex') };
  } catch (error) {
    const out = error.stdout ?? Buffer.alloc(0);
    return {
      code: error.status ?? 1,
      bytes: out.length,
      sha256: createHash('sha256').update(out).digest('hex'),
    };
  }
}

  const page = `<!doctype html>
<meta charset="utf-8"><title>rasc wasm in a browser</title>
<script type="module">
  const wasm = await (await fetch('/rasc.wasm')).arrayBuffer();
  const commands = await (await fetch('/commands')).json();
  const modes = await (await fetch('/modes')).json();
  // Only the file mode needs the bytes in the page: fetching the archive is the harness's
  // own cost, not the module's, and leaving it unconditional would confound the measurement.
  const file = modes.includes('file')
    ? new File([await (await fetch('/app.apk')).blob()], 'app.apk')
    : null;

  const init = (worker, payload) =>
    new Promise((resolve, reject) => {
      worker.onmessage = ({ data }) => (data.error ? reject(new Error(data.error)) : resolve(data));
      worker.postMessage(payload);
    });

  const runOne = (worker, argv) =>
    new Promise((resolve, reject) => {
      const chunks = [];
      let length = 0;
      worker.onmessage = async ({ data }) => {
        if (data.error) return reject(new Error(data.error));
        if (data.chunk) { chunks.push(data.chunk); length += data.chunk.length; return; }
        const bytes = new Uint8Array(length);
        let at = 0;
        for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
        const digest = await crypto.subtle.digest('SHA-256', bytes);
        const sha256 = [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
        resolve({ code: data.code, bytes: length, sha256, reads: data.reads, backend: data.fetches, ms: data.ms, wasmMemoryBytes: data.wasmMemoryBytes, text: length < 400 ? new TextDecoder().decode(bytes) : '' });
      };
      worker.postMessage({ argv });
    });

  for (const mode of modes) {
    const worker = new Worker('/js/worker.mjs', { type: 'module' });
    await init(worker, mode === 'file' ? { wasm, file } : { wasm, url: '/app.apk' });
    const results = [];
    for (const command of commands) {
      results.push({ label: command.label, ...(await runOne(worker, command.argv)) });
    }
    worker.terminate();
    await fetch('/result', { method: 'POST', body: JSON.stringify({ mode, results }) });
  }
</script>`;

const server = createServer((request, response) => {
  const url = new URL(request.url, 'http://127.0.0.1');
  const send = (status, type, body) => {
    response.writeHead(status, { 'content-type': type });
    response.end(body);
  };

  if (request.method === 'POST' && url.pathname === '/result') {
    let body = '';
    request.on('data', (part) => (body += part));
    request.on('end', () => {
      send(200, 'text/plain', 'ok');
      const parsed = JSON.parse(body);
      received.set(parsed.mode, parsed.results);
      if (received.size === modes.length) finish();
    });
    return;
  }

  switch (url.pathname) {
    case '/':
      return send(200, 'text/html', page);
    case '/commands':
      return send(200, 'application/json', JSON.stringify(commands));
    case '/modes':
      return send(200, 'application/json', JSON.stringify(modes));
    case '/rasc.wasm':
      response.writeHead(200, { 'content-type': 'application/wasm' });
      return createReadStream(wasmPath).pipe(response);
    case '/app.apk': {
      const size = statSync(apk).size;
      const range = request.headers.range;
      const match = range && /bytes=(\d+)-(\d*)/.exec(range);
      if (match) {
        const start = Number(match[1]);
        const end = match[2] ? Math.min(Number(match[2]), size - 1) : size - 1;
        response.writeHead(206, {
          'content-type': 'application/vnd.android.package-archive',
          'accept-ranges': 'bytes',
          'content-range': `bytes ${start}-${end}/${size}`,
          'content-length': String(end - start + 1),
        });
        if (request.method === 'HEAD') return response.end();
        return createReadStream(apk, { start, end }).pipe(response);
      }
      response.writeHead(200, {
        'content-type': 'application/vnd.android.package-archive',
        'accept-ranges': 'bytes',
        'content-length': String(size),
      });
      if (request.method === 'HEAD') return response.end();
      return createReadStream(apk).pipe(response);
    }
    default:
      if (url.pathname.startsWith('/js/') && !url.pathname.includes('..')) {
        const path = join(repo, url.pathname);
        if (existsSync(path)) {
          const types = { '.mjs': 'text/javascript', '.js': 'text/javascript' };
          return send(
            200,
            types[extname(path)] ?? 'application/octet-stream',
            readFileSync(path),
          );
        }
      }
      return send(404, 'text/plain', 'not found');
  }
});

const received = new Map();
let child;
let checks = 0;
let failures = 0;
let profilePath = '';
let peakRssKib = 0;

/** Sum of RSS over every Chrome process started with this profile, in MiB. */
function sampleRss() {
  try {
    const output = execFileSync('ps', ['-Ao', 'rss=,command='], { maxBuffer: 1 << 26 }).toString();
    let total = 0;
    for (const line of output.split('\n')) {
      if (!line.includes(profilePath)) continue;
      const kib = Number(line.trim().split(/\s+/)[0]);
      if (Number.isFinite(kib)) total += kib;
    }
    if (total > peakRssKib) peakRssKib = total;
  } catch {
    // Sampling is best effort; a failed ps must not fail the run.
  }
}

function peakRssMiB() {
  return (peakRssKib / 1024).toFixed(0);
}

function finish() {
  for (const mode of modes) {
    const entries = received.get(mode) ?? [];
    const byLabel = new Map(entries.map((entry) => [entry.label, entry]));
    for (const { label, argv } of commands) {
      checks += 1;
      const reference = nativeRun(argv);
      const got = byLabel.get(label);
      const ok =
        got &&
        got.code === reference.code &&
        got.bytes === reference.bytes &&
        got.sha256 === reference.sha256;
      if (!ok) failures += 1;
      console.log(
        `${ok ? 'ok  ' : 'DIFF'} [${mode}] ${label.padEnd(28)}` +
          ` browser=${got?.code}/${got?.sha256?.slice(0, 8)} native=${reference.code}/${reference.sha256.slice(0, 8)}` +
          ` bytes=${got?.bytes} backend=${got?.backend} ms=${got?.ms?.toFixed(0)}` +
          ` wasm_mem=${got ? (got.wasmMemoryBytes / 1048576).toFixed(0) : '?'} MiB` +
          `${ok || !got?.text ? '' : ` text=${JSON.stringify(got.text.trim().slice(0, 160))}`}`,
      );
    }
  }
  // The browser's own memory, sampled from the process tree while the page works: the wasm
  // linear memory reported above is precise but only covers the instance, not the renderer.
  console.log(`chrome peak rss (whole tree): ${peakRssMiB()} MiB`);
  console.log(failures === 0 ? `PASS (${checks} checks)` : `FAIL (${failures} of ${checks})`);
  shutdown(failures === 0 ? 0 : 1);
}

function shutdown(code) {
  child?.kill('SIGKILL');
  server.close(() => process.exit(code));
  setTimeout(() => process.exit(code), 2000).unref();
}

server.listen(0, '127.0.0.1', () => {
  const port = server.address().port;
  const profile = mkdtempSync(join(tmpdir(), 'rasc-chrome-'));
  profilePath = profile;
  const sampler = setInterval(sampleRss, 100);
  sampler.unref();
  child = spawn(
    chrome,
    [
      '--headless=new',
      '--disable-gpu',
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-extensions',
      `--user-data-dir=${profile}`,
      `http://127.0.0.1:${port}/`,
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  const log = [];
  child.stderr.on('data', (part) => log.push(part));
  child.on('exit', (code) => {
    if (checks === 0) {
      console.error(`chrome exited early (${code})\n${Buffer.concat(log).toString().slice(0, 800)}`);
      shutdown(1);
    }
  });
  console.log(`chrome on port ${port}\nwasm ${wasmPath}\nmodes ${modes.join(', ')}`);
  setTimeout(() => {
    if (checks === 0) {
      console.error('timed out waiting for the browser result');
      shutdown(1);
    }
  }, 300000).unref();
});
