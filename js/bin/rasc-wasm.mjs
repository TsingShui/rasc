#!/usr/bin/env node
/**
 * The wasm build as a command-line tool.
 *
 *   node js/bin/rasc-wasm.mjs manifest app.apk
 *   node js/bin/rasc-wasm.mjs findrefs --threads 1 app.apk field INSTANCE
 *
 * Same arguments, same stdout, same exit code as the native binary: the module needs a range
 * source, so the archive argument is found among the arguments (the first one that names an
 * existing file) and served with `fs.readSync`. Nothing else about the command line changes,
 * and `--help`/`--version` - plus `skill`, which needs no archive - are answered by the module
 * itself.
 *
 * The wasm build has no stderr of its own, so the wrapper keeps the payload contract intact:
 * a successful payload is streamed to stdout (buffering only a small tail, so a 61 MiB
 * `classes` run does not sit in memory), and a failed run's message goes to stderr with
 * stdout left empty - which is what the native CLI does.
 *
 * Set RASC_WASM to point at another module build.
 */
import { closeSync, existsSync, openSync, statSync, unlinkSync, writeSync } from 'node:fs';
import { loadFromFile } from '../node.mjs';

// A consumer that closes stdout early (the usual `| head -1`) must end quietly. The native
// CLI restores the default SIGPIPE disposition for exactly this; Node cannot die from
// SIGPIPE, so the EPIPE is handled here and the process ends with status 0, which is the
// other value the CLI contract accepts.
for (const stream of [process.stdout, process.stderr]) {
  stream.on('error', (error) => {
    if (error.code === 'EPIPE') process.exit(0);
    throw error;
  });
}

const argv = process.argv.slice(2);

/**
 * Takes `-o`/`--output` out of the arguments.
 *
 * The module has no filesystem, so writing the file is the host's job: the payload still goes
 * to stdout and is teed into the file. Only a form with a value is stripped, so a malformed
 * `-o` is still reported by the module's own argument parser.
 */
function takeOutputPath(args) {
  const rest = [];
  let path;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if ((arg === '-o' || arg === '--output') && index + 1 < args.length) {
      path = args[index + 1];
      index += 1;
      continue;
    }
    if (arg.startsWith('--output=')) {
      path = arg.slice('--output='.length);
      continue;
    }
    if (arg.startsWith('-o=')) {
      path = arg.slice('-o='.length);
      continue;
    }
    rest.push(arg);
  }
  return { args: rest, path };
}

const { args, path: outputPath } = takeOutputPath(argv);

// `--help` and `--version` need no archive, and neither does `skill`: it writes the bundled
// skill (or prints it with `--print`), so the "first argument that names an existing file"
// rule below cannot apply to it.
const ARCHIVE_FREE = new Set(['skill']);
const subcommand = args.find((arg) => !arg.startsWith('-'));
const archiveFree = ARCHIVE_FREE.has(subcommand ?? '');
const apk = archiveFree
  ? undefined
  : args.find((arg) => !arg.startsWith('-') && existsSync(arg) && statSync(arg).isFile());
if (
  !archiveFree &&
  !apk &&
  !args.includes('--help') &&
  !args.includes('-h') &&
  !args.includes('--version')
) {
  process.stderr.write('rasc-wasm: no archive argument found\n');
  process.exitCode = 2;
} else {

let rasc;
try {
  rasc = await loadFromFile({
    apk: apk ?? '/dev/null',
    wasmPath: process.env.RASC_WASM,
    // `--debug` diagnostics: the module has no stderr, so they come back through the host.
    onDiagnostic: (bytes) => process.stderr.write(bytes),
  });
} catch (error) {
  process.stderr.write(`rasc-wasm: ${error}\n`);
  process.exit(2);
}

const outputFile = outputPath === undefined ? null : openSync(outputPath, 'w');

/** Payload chunks not yet written, held only until they are clearly not an error message. */
const pending = [];
let pendingBytes = 0;
const flushToStdout = () => {
  for (const chunk of pending) process.stdout.write(chunk);
  pending.length = 0;
  pendingBytes = 0;
};

const { code, reads, fetches } = rasc.run(args, {
  onOutput: (bytes) => {
    if (outputFile !== null) writeSync(outputFile, bytes);
    pending.push(bytes);
    pendingBytes += bytes.length;
    // A payload this large cannot be an error message, so stream it rather than hold it.
    if (pendingBytes > 1 << 16) flushToStdout();
  },
});
rasc.close();

if (process.env.RASC_WASM_DEBUG) {
  process.stderr.write(`[rasc-wasm] reads=${reads} backend=${fetches}\n`);
}

// `process.exitCode` rather than `process.exit`: stdout writes are asynchronous once they
// exceed the pipe buffer, and exiting would truncate a large payload.
if (code === 0) {
  flushToStdout();
  if (outputFile !== null) closeSync(outputFile);
  process.exitCode = 0;
} else {
  // The native CLI writes nothing on error, so the file this wrapper created goes away again.
  if (outputFile !== null) {
    closeSync(outputFile);
    unlinkSync(outputPath);
  }
  const message = Buffer.concat(pending).toString('utf8');
  if (message.length > 0) {
    process.stderr.write(message.endsWith('\n') ? message : `${message}\n`);
  }
  process.exitCode = code;
  }
}
